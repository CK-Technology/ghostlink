use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub server: HttpServerConfig,
    pub database: DatabaseConfig,
    pub security: SecurityConfig,
    pub relay: RelayConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpServerConfig {
    pub host: String,
    pub port: u16,
    pub static_files_dir: Option<PathBuf>,
    pub max_connections: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub jwt_secret: String,
    pub session_timeout_minutes: u64,
    pub max_failed_attempts: u32,
    pub enable_mfa: bool,
    pub tls: Option<TlsConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
    pub require_client_cert: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayConfig {
    pub max_relay_connections: usize,
    pub connection_timeout_seconds: u64,
    pub heartbeat_interval_seconds: u64,
    pub max_bandwidth_mbps: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub json_format: bool,
    pub audit_log_enabled: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            server: HttpServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                static_files_dir: Some(PathBuf::from("./web")),
                max_connections: 10000,
            },
            database: DatabaseConfig {
                url: "postgresql://atlasconnect:password@localhost/atlasconnect".to_string(),
                max_connections: 20,
                min_connections: 5,
            },
            security: SecurityConfig {
                jwt_secret: "change-me-in-production".to_string(),
                session_timeout_minutes: 480, // 8 hours
                max_failed_attempts: 5,
                enable_mfa: true,
                tls: None,
            },
            relay: RelayConfig {
                max_relay_connections: 1000,
                connection_timeout_seconds: 300,
                heartbeat_interval_seconds: 30,
                max_bandwidth_mbps: None,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                json_format: false,
                audit_log_enabled: true,
            },
        }
    }
}

impl ServerConfig {
    pub fn load() -> anyhow::Result<Self> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name("config.toml").required(false))
            .add_source(config::Environment::with_prefix("ATLAS"))
            .build()?;

        let config = settings.try_deserialize()?;
        Ok(config)
    }
}
