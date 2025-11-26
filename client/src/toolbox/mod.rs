#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tracing::{debug, info};
use uuid::Uuid;

pub mod storage;
pub mod server_sync;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub command: String,
    pub icon_path: Option<String>,
    pub category: ToolCategory,
    pub version: String,
    pub checksum: String,
    pub is_portable: bool,
    pub requires_admin: bool,
    pub auto_update: bool,
    pub server_managed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolCategory {
    System,
    Network,
    Security,
    Monitoring,
    Development,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolboxConfig {
    pub local_tools_path: PathBuf,
    pub server_sync_enabled: bool,
    pub auto_update_enabled: bool,
    pub organization_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
}

pub struct ToolboxManager {
    config: ToolboxConfig,
    local_tools: HashMap<Uuid, Tool>,
    server_tools: HashMap<Uuid, Tool>,
}

impl ToolboxManager {
    pub async fn new(config: ToolboxConfig) -> Result<Self> {
        let mut manager = Self {
            config,
            local_tools: HashMap::new(),
            server_tools: HashMap::new(),
        };
        
        manager.initialize().await?;
        Ok(manager)
    }
    
    async fn initialize(&mut self) -> Result<()> {
        // Ensure local tools directory exists
        if !self.config.local_tools_path.exists() {
            fs::create_dir_all(&self.config.local_tools_path).await?;
            info!("Created local tools directory: {}", self.config.local_tools_path.display());
        }
        
        // Load local tools
        self.load_local_tools().await?;
        
        // Sync with server if enabled
        if self.config.server_sync_enabled {
            self.sync_server_tools().await?;
        }
        
        Ok(())
    }
    
    async fn load_local_tools(&mut self) -> Result<()> {
        let tools_config_path = self.config.local_tools_path.join("tools.json");
        
        if tools_config_path.exists() {
            let content = fs::read_to_string(&tools_config_path).await?;
            let tools: Vec<Tool> = serde_json::from_str(&content)?;
            
            for tool in tools {
                self.local_tools.insert(tool.id, tool);
            }
            
            info!("Loaded {} local tools", self.local_tools.len());
        }
        
        Ok(())
    }
    
    async fn sync_server_tools(&mut self) -> Result<()> {
        // TODO: Implement server synchronization
        debug!("Server tool synchronization not yet implemented");
        Ok(())
    }
    
    pub async fn add_tool(&mut self, tool: Tool) -> Result<()> {
        // Download and verify tool if it's server-managed
        if tool.server_managed {
            self.download_tool(&tool).await?;
        }
        
        // Add to appropriate collection
        if tool.server_managed {
            self.server_tools.insert(tool.id, tool);
        } else {
            self.local_tools.insert(tool.id, tool);
            self.save_local_tools().await?;
        }
        
        Ok(())
    }
    
    pub fn remove_tool(&mut self, tool_id: &Uuid) -> Result<()> {
        if self.local_tools.remove(tool_id).is_some() {
            // Remove local tool files
            let tool_dir = self.config.local_tools_path.join(tool_id.to_string());
            if tool_dir.exists() {
                std::fs::remove_dir_all(tool_dir)?;
            }
        } else if self.server_tools.remove(tool_id).is_some() {
            // Remove server-managed tool files
            let tool_dir = self.config.local_tools_path.join("server").join(tool_id.to_string());
            if tool_dir.exists() {
                std::fs::remove_dir_all(tool_dir)?;
            }
        }
        
        Ok(())
    }
    
    pub fn get_tool(&self, tool_id: &Uuid) -> Option<&Tool> {
        self.local_tools.get(tool_id)
            .or_else(|| self.server_tools.get(tool_id))
    }
    
    pub fn list_tools(&self) -> Vec<&Tool> {
        let mut tools: Vec<&Tool> = self.local_tools.values().collect();
        tools.extend(self.server_tools.values());
        tools.sort_by(|a, b| a.name.cmp(&b.name));
        tools
    }
    
    pub fn list_tools_by_category(&self, category: &ToolCategory) -> Vec<&Tool> {
        self.list_tools()
            .into_iter()
            .filter(|tool| std::mem::discriminant(&tool.category) == std::mem::discriminant(category))
            .collect()
    }
    
    async fn download_tool(&self, tool: &Tool) -> Result<()> {
        // TODO: Implement tool download from server
        debug!("Tool download not yet implemented for: {}", tool.name);
        Ok(())
    }
    
    async fn save_local_tools(&self) -> Result<()> {
        let tools: Vec<&Tool> = self.local_tools.values().collect();
        let tools_config_path = self.config.local_tools_path.join("tools.json");
        let content = serde_json::to_string_pretty(&tools)?;
        fs::write(&tools_config_path, content).await?;
        Ok(())
    }
    
    pub async fn execute_tool(&self, tool_id: &Uuid, args: Vec<String>) -> Result<String> {
        let tool = self.get_tool(tool_id)
            .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", tool_id))?;
        
        info!("Executing tool: {} with args: {:?}", tool.name, args);
        
        // Build command
        let mut command = tokio::process::Command::new(&tool.command);
        command.args(&args);
        
        // Set working directory to tool's directory
        let tool_dir = if tool.server_managed {
            self.config.local_tools_path.join("server").join(tool.id.to_string())
        } else {
            self.config.local_tools_path.join(tool.id.to_string())
        };
        
        if tool_dir.exists() {
            command.current_dir(&tool_dir);
        }
        
        // Execute command
        let output = command.output().await?;
        
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let error = String::from_utf8_lossy(&output.stderr).to_string();
            Err(anyhow::anyhow!("Tool execution failed: {}", error))
        }
    }
}

impl Default for ToolboxConfig {
    fn default() -> Self {
        let documents_dir = dirs::document_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")));
        
        Self {
            local_tools_path: documents_dir.join("GhostLink").join("Tools"),
            server_sync_enabled: true,
            auto_update_enabled: true,
            organization_id: None,
            user_id: None,
        }
    }
}