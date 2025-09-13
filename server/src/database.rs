use sqlx::{PgPool, Row};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::models::{User, Agent, Session, SessionAuditLog, Organization};
use std::collections::HashMap;
use anyhow::Result;

pub struct DatabaseService {
    pool: PgPool,
}

impl DatabaseService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // User operations
    pub async fn create_user(&self, user: &User) -> Result<Uuid> {
        let id = sqlx::query_scalar!(
            r#"
            INSERT INTO users (username, email, password_hash, full_name, role, is_active)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#,
            user.username,
            user.email,
            user.password_hash,
            user.full_name,
            user.role,
            user.is_active
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(id)
    }

    pub async fn get_user_by_username(&self, username: &str) -> Result<Option<User>> {
        let user = sqlx::query_as!(
            User,
            "SELECT * FROM users WHERE username = $1",
            username
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn update_user_last_login(&self, user_id: Uuid) -> Result<()> {
        sqlx::query!(
            "UPDATE users SET last_login = NOW() WHERE id = $1",
            user_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // Organization operations
    pub async fn create_organization(&self, org: &Organization) -> Result<Uuid> {
        let id = sqlx::query_scalar!(
            r#"
            INSERT INTO organizations (name, slug, settings)
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
            org.name,
            org.slug,
            org.settings as _
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(id)
    }

    pub async fn get_organization_by_slug(&self, slug: &str) -> Result<Option<Organization>> {
        let org = sqlx::query_as!(
            Organization,
            "SELECT * FROM organizations WHERE slug = $1",
            slug
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(org)
    }

    // Agent operations
    pub async fn create_agent(&self, agent: &Agent) -> Result<Uuid> {
        let id = sqlx::query_scalar!(
            r#"
            INSERT INTO agents (organization_id, name, hostname, platform, architecture, 
                              os_version, agent_version, public_key, status, connection_info, 
                              capabilities, settings)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING id
            "#,
            agent.organization_id,
            agent.name,
            agent.hostname,
            agent.platform,
            agent.architecture,
            agent.os_version,
            agent.agent_version,
            agent.public_key,
            agent.status,
            agent.connection_info as _,
            agent.capabilities as _,
            agent.settings as _
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(id)
    }

    pub async fn get_agent_by_id(&self, id: Uuid) -> Result<Option<Agent>> {
        let agent = sqlx::query_as!(
            Agent,
            "SELECT * FROM agents WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(agent)
    }

    pub async fn update_agent_status(&self, id: Uuid, status: &str, last_seen: DateTime<Utc>) -> Result<()> {
        sqlx::query!(
            "UPDATE agents SET status = $1, last_seen = $2 WHERE id = $3",
            status,
            last_seen,
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_connected_agents(&self, organization_id: Option<Uuid>) -> Result<Vec<Agent>> {
        let agents = if let Some(org_id) = organization_id {
            sqlx::query_as!(
                Agent,
                "SELECT * FROM agents WHERE organization_id = $1 AND status = 'online' ORDER BY name",
                org_id
            )
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as!(
                Agent,
                "SELECT * FROM agents WHERE status = 'online' ORDER BY name"
            )
            .fetch_all(&self.pool)
            .await?
        };

        Ok(agents)
    }

    // Session operations
    pub async fn create_session(&self, session: &Session) -> Result<Uuid> {
        let id = sqlx::query_scalar!(
            r#"
            INSERT INTO sessions (agent_id, user_id, organization_id, session_type, status,
                                started_at, bytes_transferred, frames_captured, settings, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id
            "#,
            session.agent_id,
            session.user_id,
            session.organization_id,
            session.session_type,
            session.status,
            session.started_at,
            session.bytes_transferred,
            session.frames_captured,
            session.settings as _,
            session.metadata as _
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(id)
    }

    pub async fn end_session(&self, session_id: Uuid, bytes_transferred: i64, frames_captured: i32) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE sessions 
            SET status = 'ended', 
                ended_at = NOW(), 
                duration_seconds = EXTRACT(EPOCH FROM (NOW() - started_at))::INTEGER,
                bytes_transferred = $1,
                frames_captured = $2
            WHERE id = $3
            "#,
            bytes_transferred,
            frames_captured,
            session_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_agent_sessions(&self, agent_id: Uuid) -> Result<Vec<Session>> {
        let sessions = sqlx::query_as!(
            Session,
            "SELECT * FROM sessions WHERE agent_id = $1 ORDER BY started_at DESC",
            agent_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(sessions)
    }

    pub async fn get_user_sessions(&self, user_id: Uuid) -> Result<Vec<Session>> {
        let sessions = sqlx::query_as!(
            Session,
            "SELECT * FROM sessions WHERE user_id = $1 ORDER BY started_at DESC",
            user_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(sessions)
    }

    // Audit operations
    pub async fn log_session_event(&self, audit_log: &SessionAuditLog) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO session_audit_log (session_id, event_type, event_data, user_id, agent_id)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            audit_log.session_id,
            audit_log.event_type,
            audit_log.event_data as _,
            audit_log.user_id,
            audit_log.agent_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // Statistics
    pub async fn get_statistics(&self, organization_id: Option<Uuid>) -> Result<serde_json::Value> {
        let base_query = if organization_id.is_some() {
            "WHERE organization_id = $1"
        } else {
            "WHERE 1=1"
        };

        let agents_online = if let Some(org_id) = organization_id {
            sqlx::query_scalar!(
                "SELECT COUNT(*) FROM agents WHERE organization_id = $1 AND status = 'online'",
                org_id
            )
            .fetch_one(&self.pool)
            .await?
            .unwrap_or(0)
        } else {
            sqlx::query_scalar!(
                "SELECT COUNT(*) FROM agents WHERE status = 'online'"
            )
            .fetch_one(&self.pool)
            .await?
            .unwrap_or(0)
        };

        let total_agents = if let Some(org_id) = organization_id {
            sqlx::query_scalar!(
                "SELECT COUNT(*) FROM agents WHERE organization_id = $1",
                org_id
            )
            .fetch_one(&self.pool)
            .await?
            .unwrap_or(0)
        } else {
            sqlx::query_scalar!(
                "SELECT COUNT(*) FROM agents"
            )
            .fetch_one(&self.pool)
            .await?
            .unwrap_or(0)
        };

        let active_sessions = if let Some(org_id) = organization_id {
            sqlx::query_scalar!(
                "SELECT COUNT(*) FROM sessions WHERE organization_id = $1 AND status = 'active'",
                org_id
            )
            .fetch_one(&self.pool)
            .await?
            .unwrap_or(0)
        } else {
            sqlx::query_scalar!(
                "SELECT COUNT(*) FROM sessions WHERE status = 'active'"
            )
            .fetch_one(&self.pool)
            .await?
            .unwrap_or(0)
        };

        Ok(serde_json::json!({
            "agents_online": agents_online,
            "total_agents": total_agents,
            "active_sessions": active_sessions,
            "timestamp": Utc::now()
        }))
    }

    // Database health check
    pub async fn health_check(&self) -> Result<bool> {
        let result = sqlx::query_scalar!("SELECT 1")
            .fetch_one(&self.pool)
            .await?;

        Ok(result == Some(1))
    }

    // Run migrations
    pub async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await?;

        Ok(())
    }
}