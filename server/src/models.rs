use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub full_name: String,
    pub role: UserRole,
    pub is_active: bool,
    pub mfa_enabled: bool,
    pub mfa_secret: Option<String>,
    pub last_login: Option<DateTime<Utc>>,
    pub failed_login_attempts: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    Operator,
    Viewer,
    Technician,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: Uuid,
    pub name: String,
    pub hostname: String,
    pub platform: String,
    pub architecture: String,
    pub version: String,
    pub public_key: String,
    pub last_seen: Option<DateTime<Utc>>,
    pub is_online: bool,
    pub owner_id: Uuid,
    pub group_id: Option<Uuid>,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub user_id: Uuid,
    pub session_type: SessionType,
    pub status: SessionStatus,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub ip_address: String,
    pub user_agent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "session_type", rename_all = "lowercase")]
pub enum SessionType {
    View,
    Control,
    FileTransfer,
    Terminal,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "session_status", rename_all = "lowercase")]
pub enum SessionStatus {
    Active,
    Connecting,
    Ended,
    Failed,
}
