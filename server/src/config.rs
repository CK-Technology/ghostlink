use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub jwt_secret: String,
    pub session_timeout: u64,
    pub max_concurrent_sessions: u32,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        Ok(AppConfig {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "8443".to_string())
                .parse()
                .unwrap_or(8443),
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:./data/atlasconnect.db".to_string()),
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or_else(|_| "your-secret-key-here".to_string()),
            session_timeout: env::var("SESSION_TIMEOUT")
                .unwrap_or_else(|_| "3600".to_string())
                .parse()
                .unwrap_or(3600),
            max_concurrent_sessions: env::var("MAX_CONCURRENT_SESSIONS")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .unwrap_or(100),
        })
    }
}
