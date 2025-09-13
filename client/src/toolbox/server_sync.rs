use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::time::{Duration, interval};
use tracing::{debug, info, warn, error};
use uuid::Uuid;

use super::{Tool, ToolboxConfig};

#[derive(Debug, Serialize, Deserialize)]
struct ToolSyncRequest {
    organization_id: Option<Uuid>,
    user_id: Option<Uuid>,
    last_sync: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolSyncResponse {
    tools: Vec<ServerTool>,
    deleted_tools: Vec<Uuid>,
    last_sync: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ServerTool {
    #[serde(flatten)]
    tool: Tool,
    download_url: String,
    file_size: u64,
    last_modified: chrono::DateTime<chrono::Utc>,
}

pub struct ServerSync {
    client: Client,
    config: ToolboxConfig,
    server_url: String,
    last_sync: Option<chrono::DateTime<chrono::Utc>>,
}

impl ServerSync {
    pub fn new(config: ToolboxConfig, server_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
            
        Self {
            client,
            config,
            server_url,
            last_sync: None,
        }
    }
    
    pub async fn sync_tools(&mut self) -> Result<(Vec<Tool>, Vec<Uuid>)> {
        let sync_url = format!("{}/api/toolbox/sync", self.server_url);
        
        let request = ToolSyncRequest {
            organization_id: self.config.organization_id,
            user_id: self.config.user_id,
            last_sync: self.last_sync,
        };
        
        let response = self.client
            .post(&sync_url)
            .json(&request)
            .send()
            .await?;
            
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Server sync failed: {}", response.status()));
        }
        
        let sync_response: ToolSyncResponse = response.json().await?;
        
        // Update last sync time
        self.last_sync = Some(sync_response.last_sync);
        
        // Convert server tools to regular tools
        let tools: Vec<Tool> = sync_response.tools
            .into_iter()
            .map(|server_tool| server_tool.tool)
            .collect();
            
        info!("Synced {} tools from server, {} deleted", tools.len(), sync_response.deleted_tools.len());
        
        Ok((tools, sync_response.deleted_tools))
    }
    
    pub async fn download_tool(&self, tool: &Tool, download_url: &str) -> Result<Vec<u8>> {
        debug!("Downloading tool: {} from {}", tool.name, download_url);
        
        let response = self.client
            .get(download_url)
            .send()
            .await?;
            
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Tool download failed: {}", response.status()));
        }
        
        let data = response.bytes().await?;
        info!("Downloaded tool '{}': {} bytes", tool.name, data.len());
        
        Ok(data.to_vec())
    }
    
    pub async fn upload_tool(&self, tool: &Tool, file_data: Vec<u8>) -> Result<()> {
        let upload_url = format!("{}/api/toolbox/upload", self.server_url);
        
        let form = reqwest::multipart::Form::new()
            .text("tool_data", serde_json::to_string(tool)?)
            .part("file", reqwest::multipart::Part::bytes(file_data)
                .file_name(format!("{}.zip", tool.name)));
        
        let response = self.client
            .post(&upload_url)
            .multipart(form)
            .send()
            .await?;
            
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Tool upload failed: {}", response.status()));
        }
        
        info!("Uploaded tool: {}", tool.name);
        Ok(())
    }
    
    pub async fn delete_tool_from_server(&self, tool_id: &Uuid) -> Result<()> {
        let delete_url = format!("{}/api/toolbox/tools/{}", self.server_url, tool_id);
        
        let response = self.client
            .delete(&delete_url)
            .send()
            .await?;
            
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Tool deletion failed: {}", response.status()));
        }
        
        info!("Deleted tool from server: {}", tool_id);
        Ok(())
    }
    
    pub async fn start_background_sync(&mut self, mut shutdown_rx: tokio::sync::oneshot::Receiver<()>) {
        let mut sync_interval = interval(Duration::from_secs(300)); // 5 minutes
        
        info!("Starting background tool synchronization");
        
        loop {
            tokio::select! {
                _ = sync_interval.tick() => {
                    if let Err(e) = self.sync_tools().await {
                        error!("Background sync failed: {}", e);
                    }
                }
                _ = &mut shutdown_rx => {
                    info!("Stopping background tool synchronization");
                    break;
                }
            }
        }
    }
    
    pub async fn check_tool_updates(&self, local_tools: &[Tool]) -> Result<Vec<Tool>> {
        let check_url = format!("{}/api/toolbox/check-updates", self.server_url);
        
        let tool_versions: HashMap<Uuid, String> = local_tools
            .iter()
            .filter(|tool| tool.server_managed)
            .map(|tool| (tool.id, tool.version.clone()))
            .collect();
        
        let response = self.client
            .post(&check_url)
            .json(&tool_versions)
            .send()
            .await?;
            
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Update check failed: {}", response.status()));
        }
        
        let updated_tools: Vec<Tool> = response.json().await?;
        
        if !updated_tools.is_empty() {
            info!("Found {} tool updates available", updated_tools.len());
        }
        
        Ok(updated_tools)
    }
}