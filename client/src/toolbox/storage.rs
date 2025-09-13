use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::{Tool, ToolboxConfig};

#[derive(Debug, Serialize, Deserialize)]
struct ToolboxStorage {
    version: String,
    tools: Vec<Tool>,
    last_sync: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct LocalStorage {
    config: ToolboxConfig,
    storage_path: PathBuf,
}

impl LocalStorage {
    pub fn new(config: ToolboxConfig) -> Self {
        let storage_path = config.local_tools_path.join("toolbox.json");
        Self {
            config,
            storage_path,
        }
    }
    
    pub async fn save_tools(&self, tools: &[Tool]) -> Result<()> {
        // Ensure directory exists
        if let Some(parent) = self.storage_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        let storage = ToolboxStorage {
            version: "1.0".to_string(),
            tools: tools.to_vec(),
            last_sync: Some(chrono::Utc::now()),
        };
        
        let content = serde_json::to_string_pretty(&storage)?;
        fs::write(&self.storage_path, content).await?;
        
        debug!("Saved {} tools to storage", tools.len());
        Ok(())
    }
    
    pub async fn load_tools(&self) -> Result<Vec<Tool>> {
        if !self.storage_path.exists() {
            debug!("No local storage found, starting with empty toolbox");
            return Ok(Vec::new());
        }
        
        let content = fs::read_to_string(&self.storage_path).await?;
        let storage: ToolboxStorage = serde_json::from_str(&content)?;
        
        info!("Loaded {} tools from local storage", storage.tools.len());
        Ok(storage.tools)
    }
    
    pub async fn install_tool(&self, tool: &Tool, source_path: &Path) -> Result<()> {
        let tool_dir = self.config.local_tools_path.join(tool.id.to_string());
        
        // Create tool directory
        fs::create_dir_all(&tool_dir).await?;
        
        // Copy tool files
        if source_path.is_file() {
            // Single file tool
            let file_name = source_path.file_name()
                .ok_or_else(|| anyhow::anyhow!("Invalid source file path"))?;
            let dest_path = tool_dir.join(file_name);
            fs::copy(source_path, dest_path).await?;
        } else if source_path.is_dir() {
            // Directory tool
            copy_dir_all_sync(source_path, &tool_dir)?;
        } else {
            return Err(anyhow::anyhow!("Source path does not exist: {}", source_path.display()));
        }
        
        // Set executable permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let command_path = tool_dir.join(&tool.command);
            if command_path.exists() {
                let mut perms = fs::metadata(&command_path).await?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&command_path, perms).await?;
            }
        }
        
        info!("Installed tool '{}' to {}", tool.name, tool_dir.display());
        Ok(())
    }
    
    pub async fn uninstall_tool(&self, tool_id: &Uuid) -> Result<()> {
        let tool_dir = self.config.local_tools_path.join(tool_id.to_string());
        
        if tool_dir.exists() {
            fs::remove_dir_all(&tool_dir).await?;
            info!("Uninstalled tool {}", tool_id);
        } else {
            warn!("Tool directory not found: {}", tool_dir.display());
        }
        
        Ok(())
    }
    
    pub fn get_tool_path(&self, tool_id: &Uuid) -> PathBuf {
        self.config.local_tools_path.join(tool_id.to_string())
    }
    
    pub async fn verify_tool_integrity(&self, tool: &Tool) -> Result<bool> {
        let tool_dir = self.get_tool_path(&tool.id);
        let command_path = tool_dir.join(&tool.command);
        
        // Check if command exists
        if !command_path.exists() {
            warn!("Tool command not found: {}", command_path.display());
            return Ok(false);
        }
        
        // TODO: Verify checksum if available
        if tool.checksum != "manual" {
            debug!("Checksum verification not yet implemented for tool: {}", tool.name);
        }
        
        Ok(true)
    }
}

fn copy_dir_all_sync(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        
        if file_type.is_dir() {
            copy_dir_all_sync(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    
    Ok(())
}
