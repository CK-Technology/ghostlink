use axum::{
    extract::{Path, Query, State, Multipart},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::fs;
use uuid::Uuid;
use tracing::{info, warn, error, debug};

use crate::AppState;

/// ScreenConnect-style toolbox manager for custom tools and scripts
pub struct ToolboxManager {
    /// Available tools indexed by category
    tools: Arc<RwLock<HashMap<String, Vec<Tool>>>>,
    /// Tool storage directory
    storage_path: PathBuf,
    /// Tool execution history
    execution_history: Arc<RwLock<Vec<ToolExecution>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub category: String,
    pub tool_type: ToolType,
    pub file_path: String,
    pub icon: Option<String>,
    pub version: String,
    pub author: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub permissions: ToolPermissions,
    pub parameters: Vec<ToolParameter>,
    pub supported_platforms: Vec<Platform>,
    pub file_size: u64,
    pub checksum: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolType {
    Executable,     // .exe, binary files
    PowerShell,     // .ps1 scripts
    Batch,          // .bat, .cmd files
    Python,         // .py scripts
    JavaScript,     // .js scripts
    Portable,       // Portable applications
    Archive,        // .zip containing tools
    Registry,       // .reg files
    MSI,           // .msi installers
    Custom,        // Custom tool types
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPermissions {
    pub requires_admin: bool,
    pub requires_elevation: bool,
    pub network_access: bool,
    pub file_system_access: bool,
    pub registry_access: bool,
    pub allowed_users: Vec<String>,
    pub allowed_groups: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    pub name: String,
    pub parameter_type: ParameterType,
    pub description: String,
    pub required: bool,
    pub default_value: Option<String>,
    pub validation_regex: Option<String>,
    pub options: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParameterType {
    String,
    Integer,
    Boolean,
    File,
    Directory,
    Choice,
    Password,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Platform {
    Windows,
    Linux,
    MacOS,
    CrossPlatform,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecution {
    pub id: Uuid,
    pub tool_id: Uuid,
    pub session_id: Uuid,
    pub user_id: String,
    pub device_id: String,
    pub parameters: HashMap<String, String>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub status: ExecutionStatus,
    pub exit_code: Option<i32>,
    pub output: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
    TimedOut,
}

/// Built-in tool categories
pub const TOOL_CATEGORIES: &[&str] = &[
    "System Information",
    "Network Tools", 
    "Security Tools",
    "Disk & File Tools",
    "Registry Tools",
    "Process Management",
    "Performance Monitoring",
    "Troubleshooting",
    "NirSoft Tools",
    "Sysinternals",
    "Custom Scripts",
    "Third Party Tools",
];

impl ToolboxManager {
    pub fn new(storage_path: PathBuf) -> Self {
        Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
            storage_path,
            execution_history: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Initialize toolbox with built-in tools
    pub async fn initialize(&self) -> Result<(), String> {
        info!("Initializing toolbox manager at {:?}", self.storage_path);
        
        // Create storage directory
        if let Err(e) = fs::create_dir_all(&self.storage_path).await {
            return Err(format!("Failed to create toolbox directory: {}", e));
        }
        
        // Load built-in tools
        self.load_builtin_tools().await?;
        
        // Load custom tools from disk
        self.load_custom_tools().await?;
        
        info!("Toolbox manager initialized successfully");
        Ok(())
    }
    
    /// Load built-in tools (NirSoft, Sysinternals, etc.)
    async fn load_builtin_tools(&self) -> Result<(), String> {
        let mut tools = self.tools.write().await;
        
        // NirSoft Tools
        let nirsoft_tools = vec![
            self.create_nirsoft_tool("WirelessKeyView", "Recover wireless network keys", "wirelesskeyview.exe"),
            self.create_nirsoft_tool("ProduKey", "Recover Windows/Office product keys", "produkey.exe"),
            self.create_nirsoft_tool("BlueScreenView", "Analyze Windows crash dumps", "bluescreenview.exe"),
            self.create_nirsoft_tool("CurrPorts", "Display currently opened TCP/UDP ports", "cports.exe"),
            self.create_nirsoft_tool("ProcessActivityView", "Monitor file/registry access by processes", "processactivityview.exe"),
            self.create_nirsoft_tool("NetworkTrafficView", "Monitor network adapter traffic", "networktrafficview.exe"),
        ];
        
        tools.insert("NirSoft Tools".to_string(), nirsoft_tools);
        
        // Sysinternals Tools  
        let sysinternals_tools = vec![
            self.create_sysinternals_tool("Process Explorer", "Advanced process viewer", "procexp64.exe"),
            self.create_sysinternals_tool("Process Monitor", "Real-time file system monitor", "procmon64.exe"),
            self.create_sysinternals_tool("Autoruns", "Startup program manager", "autoruns64.exe"),
            self.create_sysinternals_tool("TCPView", "Network connection viewer", "tcpview64.exe"),
            self.create_sysinternals_tool("PSExec", "Remote command execution", "psexec64.exe"),
            self.create_sysinternals_tool("Handle", "Display open handles", "handle64.exe"),
        ];
        
        tools.insert("Sysinternals".to_string(), sysinternals_tools);
        
        // System Tools
        let system_tools = vec![
            self.create_system_tool("System Information", "Display system information", "msinfo32.exe", vec![]),
            self.create_system_tool("Device Manager", "Manage hardware devices", "devmgmt.msc", vec![]),
            self.create_system_tool("Event Viewer", "View Windows event logs", "eventvwr.msc", vec![]),
            self.create_system_tool("Services", "Manage Windows services", "services.msc", vec![]),
            self.create_system_tool("Task Manager", "Process and performance manager", "taskmgr.exe", vec![]),
            self.create_system_tool("Registry Editor", "Edit Windows registry", "regedt32.exe", vec![]),
        ];
        
        tools.insert("System Information".to_string(), system_tools);
        
        // Network Tools
        let network_tools = vec![
            self.create_network_tool("Ping", "Test network connectivity", "ping.exe", vec![
                ToolParameter {
                    name: "target".to_string(),
                    parameter_type: ParameterType::String,
                    description: "Target host or IP address".to_string(),
                    required: true,
                    default_value: None,
                    validation_regex: None,
                    options: None,
                }
            ]),
            self.create_network_tool("Traceroute", "Trace network path", "tracert.exe", vec![
                ToolParameter {
                    name: "target".to_string(),
                    parameter_type: ParameterType::String,
                    description: "Target host or IP address".to_string(),
                    required: true,
                    default_value: None,
                    validation_regex: None,
                    options: None,
                }
            ]),
            self.create_network_tool("NSLookup", "DNS lookup tool", "nslookup.exe", vec![
                ToolParameter {
                    name: "hostname".to_string(),
                    parameter_type: ParameterType::String,
                    description: "Hostname to resolve".to_string(),
                    required: true,
                    default_value: None,
                    validation_regex: None,
                    options: None,
                }
            ]),
        ];
        
        tools.insert("Network Tools".to_string(), network_tools);
        
        info!("Loaded {} built-in tool categories", tools.len());
        Ok(())
    }
    
    /// Load custom tools from storage directory
    async fn load_custom_tools(&self) -> Result<(), String> {
        let custom_dir = self.storage_path.join("custom");
        
        if !custom_dir.exists() {
            fs::create_dir_all(&custom_dir).await
                .map_err(|e| format!("Failed to create custom tools directory: {}", e))?;
            return Ok(());
        }
        
        // TODO: Scan custom directory and load tool definitions
        debug!("Custom tools directory: {:?}", custom_dir);
        
        Ok(())
    }
    
    /// Create NirSoft tool definition
    fn create_nirsoft_tool(&self, name: &str, description: &str, filename: &str) -> Tool {
        Tool {
            id: Uuid::new_v4(),
            name: name.to_string(),
            description: description.to_string(),
            category: "NirSoft Tools".to_string(),
            tool_type: ToolType::Executable,
            file_path: format!("tools/nirsoft/{}", filename),
            icon: Some(format!("icons/nirsoft/{}.ico", filename.replace(".exe", ""))),
            version: "Latest".to_string(),
            author: "NirSoft".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            permissions: ToolPermissions {
                requires_admin: false,
                requires_elevation: false,
                network_access: true,
                file_system_access: true,
                registry_access: true,
                allowed_users: vec![],
                allowed_groups: vec![],
            },
            parameters: vec![],
            supported_platforms: vec![Platform::Windows],
            file_size: 0,
            checksum: String::new(),
            tags: vec!["nirsoft".to_string(), "utility".to_string()],
        }
    }
    
    /// Create Sysinternals tool definition
    fn create_sysinternals_tool(&self, name: &str, description: &str, filename: &str) -> Tool {
        Tool {
            id: Uuid::new_v4(),
            name: name.to_string(),
            description: description.to_string(),
            category: "Sysinternals".to_string(),
            tool_type: ToolType::Executable,
            file_path: format!("tools/sysinternals/{}", filename),
            icon: Some(format!("icons/sysinternals/{}.ico", filename.replace(".exe", "").replace("64", ""))),
            version: "Latest".to_string(),
            author: "Microsoft Sysinternals".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            permissions: ToolPermissions {
                requires_admin: true,
                requires_elevation: true,
                network_access: true,
                file_system_access: true,
                registry_access: true,
                allowed_users: vec![],
                allowed_groups: vec![],
            },
            parameters: vec![],
            supported_platforms: vec![Platform::Windows],
            file_size: 0,
            checksum: String::new(),
            tags: vec!["sysinternals".to_string(), "microsoft".to_string(), "system".to_string()],
        }
    }
    
    /// Create system tool definition
    fn create_system_tool(&self, name: &str, description: &str, filename: &str, parameters: Vec<ToolParameter>) -> Tool {
        Tool {
            id: Uuid::new_v4(),
            name: name.to_string(),
            description: description.to_string(),
            category: "System Information".to_string(),
            tool_type: ToolType::Executable,
            file_path: filename.to_string(),
            icon: Some(format!("icons/system/{}.ico", filename.replace(".exe", "").replace(".msc", ""))),
            version: "System".to_string(),
            author: "Microsoft".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            permissions: ToolPermissions {
                requires_admin: filename.contains("regedt32") || filename.contains("services"),
                requires_elevation: filename.contains("regedt32") || filename.contains("services"),
                network_access: false,
                file_system_access: true,
                registry_access: filename.contains("regedt32"),
                allowed_users: vec![],
                allowed_groups: vec![],
            },
            parameters,
            supported_platforms: vec![Platform::Windows],
            file_size: 0,
            checksum: String::new(),
            tags: vec!["system".to_string(), "builtin".to_string()],
        }
    }
    
    /// Create network tool definition
    fn create_network_tool(&self, name: &str, description: &str, filename: &str, parameters: Vec<ToolParameter>) -> Tool {
        Tool {
            id: Uuid::new_v4(),
            name: name.to_string(),
            description: description.to_string(),
            category: "Network Tools".to_string(),
            tool_type: ToolType::Executable,
            file_path: filename.to_string(),
            icon: Some(format!("icons/network/{}.ico", filename.replace(".exe", ""))),
            version: "System".to_string(),
            author: "Microsoft".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            permissions: ToolPermissions {
                requires_admin: false,
                requires_elevation: false,
                network_access: true,
                file_system_access: false,
                registry_access: false,
                allowed_users: vec![],
                allowed_groups: vec![],
            },
            parameters,
            supported_platforms: vec![Platform::Windows, Platform::Linux, Platform::MacOS],
            file_size: 0,
            checksum: String::new(),
            tags: vec!["network".to_string(), "diagnostic".to_string()],
        }
    }
    
    /// Get all available tools
    pub async fn get_all_tools(&self) -> HashMap<String, Vec<Tool>> {
        self.tools.read().await.clone()
    }
    
    /// Get tools by category
    pub async fn get_tools_by_category(&self, category: &str) -> Vec<Tool> {
        let tools = self.tools.read().await;
        tools.get(category).cloned().unwrap_or_default()
    }
    
    /// Add custom tool
    pub async fn add_custom_tool(&self, tool: Tool, file_data: Vec<u8>) -> Result<Tool, String> {
        // Save tool file to storage
        let file_path = self.storage_path.join("custom").join(&tool.file_path);
        
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await
                .map_err(|e| format!("Failed to create tool directory: {}", e))?;
        }
        
        fs::write(&file_path, file_data).await
            .map_err(|e| format!("Failed to save tool file: {}", e))?;
        
        // Add to tools collection
        let mut tools = self.tools.write().await;
        let category_tools = tools.entry("Custom Scripts".to_string()).or_insert_with(Vec::new);
        category_tools.push(tool.clone());
        
        info!("Added custom tool: {} to category: {}", tool.name, tool.category);
        Ok(tool)
    }
    
    /// Execute tool on target device
    pub async fn execute_tool(
        &self,
        tool_id: Uuid,
        session_id: Uuid,
        user_id: String,
        device_id: String,
        parameters: HashMap<String, String>,
    ) -> Result<ToolExecution, String> {
        // Find tool
        let tools = self.tools.read().await;
        let mut found_tool: Option<Tool> = None;
        
        for category_tools in tools.values() {
            if let Some(tool) = category_tools.iter().find(|t| t.id == tool_id) {
                found_tool = Some(tool.clone());
                break;
            }
        }
        
        let tool = found_tool.ok_or_else(|| format!("Tool {} not found", tool_id))?;
        
        // Create execution record
        let execution = ToolExecution {
            id: Uuid::new_v4(),
            tool_id,
            session_id,
            user_id,
            device_id,
            parameters,
            started_at: chrono::Utc::now(),
            completed_at: None,
            status: ExecutionStatus::Queued,
            exit_code: None,
            output: None,
            error: None,
        };
        
        // Store execution record
        {
            let mut history = self.execution_history.write().await;
            history.push(execution.clone());
        }
        
        info!("Queued tool execution: {} for tool: {}", execution.id, tool.name);
        
        // TODO: Send execution command to target device via WebSocket/relay
        // This would typically be handled by the session manager
        
        Ok(execution)
    }
    
    /// Get execution history
    pub async fn get_execution_history(&self, limit: Option<usize>) -> Vec<ToolExecution> {
        let history = self.execution_history.read().await;
        let mut executions = history.clone();
        
        // Sort by start time (newest first)
        executions.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        
        if let Some(limit) = limit {
            executions.truncate(limit);
        }
        
        executions
    }
}

/// API Handlers

/// Get all available tools
pub async fn api_get_tools(
    State(app_state): State<AppState>,
) -> impl IntoResponse {
    let tools = app_state.device_manager.toolbox_manager.get_all_tools().await;
    Json(tools)
}

/// Get tools by category
pub async fn api_get_tools_by_category(
    State(app_state): State<AppState>,
    Path(category): Path<String>,
) -> impl IntoResponse {
    let tools = app_state.device_manager.toolbox_manager.get_tools_by_category(&category).await;
    Json(tools)
}

/// Execute tool
pub async fn api_execute_tool(
    State(app_state): State<AppState>,
    Path(tool_id): Path<Uuid>,
    Json(request): Json<ToolExecutionRequest>,
) -> Response {
    match app_state.device_manager.toolbox_manager.execute_tool(
        tool_id,
        request.session_id,
        request.user_id,
        request.device_id,
        request.parameters,
    ).await {
        Ok(execution) => Json(execution).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e
            }))
        ).into_response(),
    }
}

/// Upload custom tool
pub async fn api_upload_tool(
    State(app_state): State<AppState>,
    mut multipart: Multipart,
) -> Response {
    // TODO: Handle multipart form data for tool upload
    // This would extract the tool definition JSON and binary file
    
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({
            "error": "Tool upload not yet implemented"
        }))
    ).into_response()
}

/// Get execution history
pub async fn api_get_execution_history(
    State(app_state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let limit = params.get("limit")
        .and_then(|s| s.parse::<usize>().ok());
    
    let history = app_state.device_manager.toolbox_manager.get_execution_history(limit).await;
    Json(history)
}

#[derive(Debug, Deserialize)]
pub struct ToolExecutionRequest {
    pub session_id: Uuid,
    pub user_id: String,
    pub device_id: String,
    pub parameters: HashMap<String, String>,
}

/// Get available tools
pub async fn api_get_available_tools(
    State(app_state): State<AppState>,
) -> impl IntoResponse {
    let tools = app_state.device_manager.toolbox_manager.get_all_tools().await;
    Json(tools)
}

/// Upload custom tool (stub)
pub async fn api_upload_custom_tool(
    State(_app_state): State<AppState>,
    mut _multipart: Multipart,
) -> impl IntoResponse {
    // TODO: Implement file upload for custom tools
    Json(serde_json::json!({
        "status": "error",
        "message": "Upload custom tool not yet implemented"
    }))
}