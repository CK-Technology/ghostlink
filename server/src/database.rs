use sqlx::{PgPool, Row};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::models::{Agent, Session, User, SessionAuditLog, Organization};
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
        let row = sqlx::query(
            r#"
            INSERT INTO users (username, email, password_hash, full_name, role, is_active)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#
        )
        .bind(&user.username)
        .bind(&user.email)
        .bind(&user.password_hash)
        .bind(&user.full_name)
        .bind(&user.role)
        .bind(user.is_active)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get("id"))
    }

    pub async fn get_user_by_username(&self, username: &str) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE username = $1"
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn get_user_by_id(&self, user_id: Uuid) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE id = $1"
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn update_user_last_login(&self, user_id: Uuid) -> Result<()> {
        sqlx::query(
            "UPDATE users SET last_login = NOW() WHERE id = $1"
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // Organization operations
    pub async fn create_organization(&self, org: &Organization) -> Result<Uuid> {
        let row = sqlx::query(
            r#"
            INSERT INTO organizations (name, slug, settings)
            VALUES ($1, $2, $3)
            RETURNING id
            "#
        )
        .bind(&org.name)
        .bind(&org.slug)
        .bind(&org.settings)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get("id"))
    }

    pub async fn get_organization_by_slug(&self, slug: &str) -> Result<Option<Organization>> {
        let org = sqlx::query_as::<_, Organization>(
            "SELECT * FROM organizations WHERE slug = $1"
        )
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?;

        Ok(org)
    }

    // Agent operations
    pub async fn create_agent(&self, agent: &Agent) -> Result<Uuid> {
        let row = sqlx::query(
            r#"
            INSERT INTO agents (organization_id, name, hostname, platform, architecture,
                              os_version, agent_version, public_key, status, connection_info,
                              capabilities, settings)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING id
            "#
        )
        .bind(&agent.organization_id)
        .bind(&agent.name)
        .bind(&agent.hostname)
        .bind(&agent.platform)
        .bind(&agent.architecture)
        .bind(&agent.os_version)
        .bind(&agent.agent_version)
        .bind(&agent.public_key)
        .bind(&agent.status)
        .bind(&agent.connection_info)
        .bind(&agent.capabilities)
        .bind(&agent.settings)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get("id"))
    }

    pub async fn get_agent_by_id(&self, id: Uuid) -> Result<Option<Agent>> {
        let agent = sqlx::query_as::<_, Agent>(
            "SELECT * FROM agents WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(agent)
    }

    pub async fn update_agent_status(&self, id: Uuid, status: &str, last_seen: DateTime<Utc>) -> Result<()> {
        sqlx::query(
            "UPDATE agents SET status = $1, last_seen = $2 WHERE id = $3"
        )
        .bind(status)
        .bind(last_seen)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_connected_agents(&self, organization_id: Option<Uuid>) -> Result<Vec<Agent>> {
        let agents = if let Some(org_id) = organization_id {
            sqlx::query_as::<_, Agent>(
                "SELECT * FROM agents WHERE organization_id = $1 AND status = 'online' ORDER BY name"
            )
            .bind(org_id)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, Agent>(
                "SELECT * FROM agents WHERE status = 'online' ORDER BY name"
            )
            .fetch_all(&self.pool)
            .await?
        };

        Ok(agents)
    }

    // Session operations
    pub async fn create_session(&self, session: &Session) -> Result<Uuid> {
        let row = sqlx::query(
            r#"
            INSERT INTO sessions (agent_id, user_id, organization_id, session_type, status,
                                started_at, bytes_transferred, frames_captured, settings, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id
            "#
        )
        .bind(&session.agent_id)
        .bind(&session.user_id)
        .bind(&session.organization_id)
        .bind(&session.session_type)
        .bind(&session.status)
        .bind(&session.started_at)
        .bind(&session.bytes_transferred)
        .bind(&session.frames_captured)
        .bind(&session.settings)
        .bind(&session.metadata)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get("id"))
    }

    pub async fn end_session(&self, session_id: Uuid, bytes_transferred: i64, frames_captured: i32) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE sessions
            SET status = 'ended',
                ended_at = NOW(),
                duration_seconds = EXTRACT(EPOCH FROM (NOW() - started_at))::INTEGER,
                bytes_transferred = $1,
                frames_captured = $2
            WHERE id = $3
            "#
        )
        .bind(bytes_transferred)
        .bind(frames_captured)
        .bind(session_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_agent_sessions(&self, agent_id: Uuid) -> Result<Vec<Session>> {
        let sessions = sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions WHERE agent_id = $1 ORDER BY started_at DESC"
        )
        .bind(agent_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(sessions)
    }

    pub async fn get_user_sessions(&self, user_id: Uuid) -> Result<Vec<Session>> {
        let sessions = sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions WHERE user_id = $1 ORDER BY started_at DESC"
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(sessions)
    }

    // Audit operations
    pub async fn log_session_event(&self, audit_log: &SessionAuditLog) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO session_audit_log (session_id, event_type, event_data, user_id, agent_id)
            VALUES ($1, $2, $3, $4, $5)
            "#
        )
        .bind(&audit_log.session_id)
        .bind(&audit_log.event_type)
        .bind(&audit_log.event_data)
        .bind(&audit_log.user_id)
        .bind(&audit_log.agent_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // Statistics
    pub async fn get_statistics(&self, organization_id: Option<Uuid>) -> Result<serde_json::Value> {
        let agents_online: i64 = if let Some(org_id) = organization_id {
            let row = sqlx::query(
                "SELECT COUNT(*) as count FROM agents WHERE organization_id = $1 AND status = 'online'"
            )
            .bind(org_id)
            .fetch_one(&self.pool)
            .await?;
            row.get("count")
        } else {
            let row = sqlx::query(
                "SELECT COUNT(*) as count FROM agents WHERE status = 'online'"
            )
            .fetch_one(&self.pool)
            .await?;
            row.get("count")
        };

        let total_agents: i64 = if let Some(org_id) = organization_id {
            let row = sqlx::query(
                "SELECT COUNT(*) as count FROM agents WHERE organization_id = $1"
            )
            .bind(org_id)
            .fetch_one(&self.pool)
            .await?;
            row.get("count")
        } else {
            let row = sqlx::query(
                "SELECT COUNT(*) as count FROM agents"
            )
            .fetch_one(&self.pool)
            .await?;
            row.get("count")
        };

        let active_sessions: i64 = if let Some(org_id) = organization_id {
            let row = sqlx::query(
                "SELECT COUNT(*) as count FROM sessions WHERE organization_id = $1 AND status = 'active'"
            )
            .bind(org_id)
            .fetch_one(&self.pool)
            .await?;
            row.get("count")
        } else {
            let row = sqlx::query(
                "SELECT COUNT(*) as count FROM sessions WHERE status = 'active'"
            )
            .fetch_one(&self.pool)
            .await?;
            row.get("count")
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
        let row = sqlx::query("SELECT 1 as val")
            .fetch_one(&self.pool)
            .await?;
        let result: i32 = row.get("val");

        Ok(result == 1)
    }
}
