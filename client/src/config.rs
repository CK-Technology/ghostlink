#![allow(dead_code)]

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_new_config() {
        let config = ClientConfig::new(
            "wss://test.example.com".to_string(),
            Some("Test Device".to_string()),
        ).unwrap();
        
        assert_eq!(config.server_url, "wss://test.example.com");
        assert_eq!(config.hostname, "Test Device");
        assert_eq!(config.reconnect_interval, 30);
        assert_eq!(config.heartbeat_interval, 30);
        assert_eq!(config.max_concurrent_sessions, 5);
        assert_eq!(config.log_level, "info");
        assert!(!config.agent_id.is_empty());
    }

    #[test]
    fn test_new_config_default_hostname() {
        let config = ClientConfig::new(
            "wss://test.example.com".to_string(),
            None,
        ).unwrap();
        
        assert_eq!(config.server_url, "wss://test.example.com");
        assert!(!config.hostname.is_empty());
    }

    #[test]
    fn test_config_save_and_load() {
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path();
        
        // Create and save config
        let original_config = ClientConfig::new(
            "wss://save-test.example.com".to_string(),
            Some("Save Test Device".to_string()),
        ).unwrap();
        
        original_config.save(temp_path).unwrap();
        
        // Load config
        let loaded_config = ClientConfig::load(temp_path).unwrap();
        
        assert_eq!(original_config.server_url, loaded_config.server_url);
        assert_eq!(original_config.hostname, loaded_config.hostname);
        assert_eq!(original_config.agent_id, loaded_config.agent_id);
    }
}
