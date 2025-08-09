use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub agent_id: String,
    pub hostname: String,
    pub server_url: String,
    pub reconnect_interval: u64,
    pub heartbeat_interval: u64,
    pub max_concurrent_sessions: u32,
    pub log_level: String,
}

impl ClientConfig {
    pub fn new(server_url: String, device_name: Option<String>) -> Result<Self> {
        // Generate or load device ID
        let agent_id = Self::get_or_create_device_id()?;
        
        // Determine device name
        let hostname = device_name.unwrap_or_else(|| {
            env::var("COMPUTERNAME")
                .or_else(|_| env::var("HOSTNAME"))
                .unwrap_or_else(|_| "Unknown Device".to_string())
        });
        
        Ok(ClientConfig {
            agent_id,
            hostname,
            server_url,
            reconnect_interval: 30, // seconds
            heartbeat_interval: 30, // seconds
            max_concurrent_sessions: 5,
            log_level: "info".to_string(),
        })
    }
    
    pub fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let config: Self = toml::from_str(&content)?;
            Ok(config)
        } else {
            // Create default config
            let config = Self::new(
                "wss://relay.cktechx.com".to_string(),
                None,
            )?;
            config.save(path)?;
            Ok(config)
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
    
    fn get_or_create_device_id() -> Result<String> {
        // In a real implementation, this would:
        // 1. Check for existing device ID in registry/config file
        // 2. Generate new one if not found
        // 3. Store it persistently
        
        // For now, generate a new UUID each time
        // TODO: Implement persistent storage
        Ok(Uuid::new_v4().to_string())
    }
}
