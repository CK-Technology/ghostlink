use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use sqlx::{FromRow, Type};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: Option<String>,
    pub password_hash: Option<String>,
    pub full_name: Option<String>,
    pub role: String,
    pub is_active: bool,
    pub last_login: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub settings: sqlx::types::Json<HashMap<String, serde_json::Value>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    Operator,
    User,
    Viewer,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Agent {
    pub id: Uuid,
    pub organization_id: Option<Uuid>,
    pub name: String,
    pub hostname: Option<String>,
    pub platform: String,
    pub architecture: Option<String>,
    pub os_version: Option<String>,
    pub agent_version: Option<String>,
    pub public_key: Option<String>,
    pub last_seen: Option<DateTime<Utc>>,
    pub status: String,
    pub connection_info: sqlx::types::Json<HashMap<String, serde_json::Value>>,
    pub capabilities: sqlx::types::Json<HashMap<String, serde_json::Value>>,
    pub settings: sqlx::types::Json<HashMap<String, serde_json::Value>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Session {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub user_id: Uuid,
    pub organization_id: Option<Uuid>,
    pub session_type: String,
    pub status: String,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_seconds: Option<i32>,
    pub bytes_transferred: i64,
    pub frames_captured: i32,
    pub settings: sqlx::types::Json<HashMap<String, serde_json::Value>>,
    pub metadata: sqlx::types::Json<HashMap<String, serde_json::Value>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SessionAuditLog {
    pub id: Uuid,
    pub session_id: Uuid,
    pub event_type: String,
    pub event_data: sqlx::types::Json<HashMap<String, serde_json::Value>>,
    pub timestamp: DateTime<Utc>,
    pub user_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "lowercase")]
pub enum SessionType {
    Console,
    Backstage,
    Adhoc,
    FileTransfer,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "lowercase")]
pub enum SessionStatus {
    Pending,
    Active,
    Ended,
    Failed,
}
