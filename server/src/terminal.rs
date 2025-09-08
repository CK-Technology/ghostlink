use axum::{
    extract::{Path, Query, State, WebSocketUpgrade, ws::{WebSocket, Message}},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command as AsyncCommand};
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;
use tracing::{info, warn, error, debug};

use crate::AppState;

/// ScreenConnect-style terminal manager for web-based command execution
pub struct TerminalManager {
    /// Active terminal sessions
    sessions: Arc<RwLock<HashMap<Uuid, TerminalSession>>>,
    /// Terminal configuration
    config: Arc<RwLock<TerminalConfig>>,
    /// Command history for all sessions
    command_history: Arc<RwLock<Vec<CommandHistoryEntry>>>,
}

#[derive(Debug, Clone)]
pub struct TerminalSession {
    pub session_id: Uuid,
    pub user_id: String,
    pub client_session_id: Uuid,
    pub shell_type: ShellType,
    pub current_directory: String,
    pub environment: HashMap<String, String>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
    pub process_handle: Option<u32>,
    pub is_elevated: bool,
    pub output_buffer: Vec<String>,
    pub input_buffer: String,
    pub status: TerminalStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShellType {
    Cmd,           // Windows Command Prompt
    PowerShell,    // Windows PowerShell
    PowerShellCore, // PowerShell Core (cross-platform)
    Bash,          // Unix/Linux Bash
    Sh,            // POSIX shell
    Zsh,           // Z shell
    Fish,          // Fish shell
    Custom(String), // Custom shell
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TerminalStatus {
    Starting,
    Active,
    Suspended,
    Terminated,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalConfig {
    pub default_shell: ShellType,
    pub max_output_buffer_lines: usize,
    pub command_timeout_seconds: u64,
    pub enable_file_transfer: bool,
    pub enable_script_upload: bool,
    pub restricted_commands: Vec<String>,
    pub audit_all_commands: bool,
    pub session_timeout_minutes: u64,
    pub color_scheme: ColorScheme,
    pub font_settings: FontSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorScheme {
    pub background: String,
    pub foreground: String,
    pub cursor: String,
    pub selection: String,
    pub black: String,
    pub red: String,
    pub green: String,
    pub yellow: String,
    pub blue: String,
    pub magenta: String,
    pub cyan: String,
    pub white: String,
    pub bright_black: String,
    pub bright_red: String,
    pub bright_green: String,
    pub bright_yellow: String,
    pub bright_blue: String,
    pub bright_magenta: String,
    pub bright_cyan: String,
    pub bright_white: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontSettings {
    pub family: String,
    pub size: u32,
    pub weight: String,
    pub line_height: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandHistoryEntry {
    pub id: Uuid,
    pub session_id: Uuid,
    pub user_id: String,
    pub command: String,
    pub working_directory: String,
    pub executed_at: chrono::DateTime<chrono::Utc>,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub output_lines: usize,
    pub error_output: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalMessage {
    pub message_type: TerminalMessageType,
    pub session_id: Uuid,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TerminalMessageType {
    /// Input from user to terminal
    Input,
    /// Output from terminal to user
    Output,
    /// Terminal control messages
    Control,
    /// File transfer messages
    FileTransfer,
    /// Session status updates
    Status,
    /// Error messages
    Error,
}

#[derive(Debug, Deserialize)]
pub struct CreateTerminalRequest {
    pub user_id: String,
    pub shell_type: Option<ShellType>,
    pub working_directory: Option<String>,
    pub environment_vars: Option<HashMap<String, String>>,
    pub elevated: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct TerminalSessionInfo {
    pub session_id: Uuid,
    pub shell_type: ShellType,
    pub current_directory: String,
    pub is_elevated: bool,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
    pub status: TerminalStatus,
}

impl TerminalManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(Self::default_config())),
            command_history: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Initialize terminal manager
    pub async fn initialize(&self) -> Result<(), String> {
        info!("Initializing terminal manager");
        
        // Load configuration
        // For now, use defaults
        
        info!("Terminal manager initialized successfully");
        Ok(())
    }
    
    /// Default terminal configuration
    fn default_config() -> TerminalConfig {
        TerminalConfig {
            default_shell: if cfg!(windows) {
                ShellType::PowerShell
            } else {
                ShellType::Bash
            },
            max_output_buffer_lines: 10000,
            command_timeout_seconds: 300,
            enable_file_transfer: true,
            enable_script_upload: true,
            restricted_commands: vec![
                "rm -rf /".to_string(),
                "format".to_string(),
                "del /f /s /q C:\\*".to_string(),
            ],
            audit_all_commands: true,
            session_timeout_minutes: 60,
            color_scheme: ColorScheme {
                background: "#1e1e1e".to_string(),
                foreground: "#cccccc".to_string(),
                cursor: "#ffffff".to_string(),
                selection: "#264f78".to_string(),
                black: "#000000".to_string(),
                red: "#cd3131".to_string(),
                green: "#0dbc79".to_string(),
                yellow: "#e5e510".to_string(),
                blue: "#2472c8".to_string(),
                magenta: "#bc3fbc".to_string(),
                cyan: "#11a8cd".to_string(),
                white: "#e5e5e5".to_string(),
                bright_black: "#666666".to_string(),
                bright_red: "#f14c4c".to_string(),
                bright_green: "#23d18b".to_string(),
                bright_yellow: "#f5f543".to_string(),
                bright_blue: "#3b8eea".to_string(),
                bright_magenta: "#d670d6".to_string(),
                bright_cyan: "#29b8db".to_string(),
                bright_white: "#ffffff".to_string(),
            },
            font_settings: FontSettings {
                family: "Consolas, 'Courier New', monospace".to_string(),
                size: 14,
                weight: "normal".to_string(),
                line_height: 1.2,
            },
        }
    }
    
    /// Create new terminal session
    pub async fn create_session(
        &self,
        client_session_id: Uuid,
        request: CreateTerminalRequest,
    ) -> Result<TerminalSession, String> {
        let config = self.config.read().await;
        
        let shell_type = request.shell_type.unwrap_or(config.default_shell.clone());
        let working_directory = request.working_directory
            .unwrap_or_else(|| self.get_default_working_directory());
        
        let mut environment = std::env::vars().collect::<HashMap<_, _>>();
        if let Some(env_vars) = request.environment_vars {
            environment.extend(env_vars);
        }
        
        let terminal_session = TerminalSession {
            session_id: Uuid::new_v4(),
            user_id: request.user_id.clone(),
            client_session_id,
            shell_type: shell_type.clone(),
            current_directory: working_directory.clone(),
            environment,
            started_at: chrono::Utc::now(),
            last_activity: chrono::Utc::now(),
            process_handle: None,
            is_elevated: request.elevated.unwrap_or(false),
            output_buffer: Vec::new(),
            input_buffer: String::new(),
            status: TerminalStatus::Starting,
        };
        
        // Store session
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(terminal_session.session_id, terminal_session.clone());
        }
        
        info!("Created terminal session {} for user {} with shell {:?}", 
              terminal_session.session_id, request.user_id, shell_type);
        
        Ok(terminal_session)
    }
    
    /// Get default working directory
    fn get_default_working_directory(&self) -> String {
        std::env::current_dir()
            .unwrap_or_else(|_| {
                if cfg!(windows) {
                    std::path::PathBuf::from("C:\\")
                } else {
                    std::path::PathBuf::from("/")
                }
            })
            .to_string_lossy()
            .to_string()
    }
    
    /// Execute command in terminal session
    pub async fn execute_command(
        &self,
        session_id: Uuid,
        command: String,
    ) -> Result<String, String> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(&session_id)
            .ok_or_else(|| format!("Terminal session {} not found", session_id))?;
        
        let config = self.config.read().await;
        
        // Check for restricted commands
        for restricted in &config.restricted_commands {
            if command.to_lowercase().contains(&restricted.to_lowercase()) {
                return Err(format!("Command contains restricted pattern: {}", restricted));
            }
        }
        
        let start_time = std::time::Instant::now();
        let executed_at = chrono::Utc::now();
        
        // Execute command based on shell type
        let result = match session.shell_type {
            ShellType::Cmd => self.execute_cmd_command(&command, &session.current_directory).await,
            ShellType::PowerShell | ShellType::PowerShellCore => {
                self.execute_powershell_command(&command, &session.current_directory).await
            }
            ShellType::Bash => self.execute_bash_command(&command, &session.current_directory).await,
            ShellType::Sh => self.execute_sh_command(&command, &session.current_directory).await,
            ShellType::Zsh => self.execute_zsh_command(&command, &session.current_directory).await,
            ShellType::Fish => self.execute_fish_command(&command, &session.current_directory).await,
            ShellType::Custom(ref shell) => {
                self.execute_custom_command(shell, &command, &session.current_directory).await
            }
        };
        
        let duration_ms = start_time.elapsed().as_millis() as u64;
        session.last_activity = chrono::Utc::now();
        
        let (exit_code, output, error) = match result {
            Ok((code, stdout, stderr)) => (code, stdout, stderr),
            Err(e) => (Some(1), String::new(), e),
        };
        
        // Add to output buffer
        if !output.is_empty() {
            for line in output.lines() {
                session.output_buffer.push(line.to_string());
            }
        }
        
        if !error.is_empty() {
            for line in error.lines() {
                session.output_buffer.push(format!("ERROR: {}", line));
            }
        }
        
        // Trim output buffer if too long
        if session.output_buffer.len() > config.max_output_buffer_lines {
            let excess = session.output_buffer.len() - config.max_output_buffer_lines;
            session.output_buffer.drain(0..excess);
        }
        
        // Record command in history
        let history_entry = CommandHistoryEntry {
            id: Uuid::new_v4(),
            session_id,
            user_id: session.user_id.clone(),
            command: command.clone(),
            working_directory: session.current_directory.clone(),
            executed_at,
            exit_code,
            duration_ms,
            output_lines: output.lines().count() + error.lines().count(),
            error_output: !error.is_empty(),
        };
        
        drop(sessions);
        drop(config);
        
        {
            let mut history = self.command_history.write().await;
            history.push(history_entry);
        }
        
        info!("Executed command in terminal session {}: {}", session_id, command);
        
        Ok(format!("{}{}", output, if error.is_empty() { String::new() } else { format!("\n{}", error) }))
    }
    
    /// Execute Windows CMD command
    async fn execute_cmd_command(
        &self,
        command: &str,
        working_dir: &str,
    ) -> Result<(Option<i32>, String, String), String> {
        let output = Command::new("cmd")
            .args(&["/C", command])
            .current_dir(working_dir)
            .output()
            .map_err(|e| format!("Failed to execute CMD command: {}", e))?;
        
        Ok((
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
    
    /// Execute PowerShell command
    async fn execute_powershell_command(
        &self,
        command: &str,
        working_dir: &str,
    ) -> Result<(Option<i32>, String, String), String> {
        let ps_command = format!("Set-Location '{}'; {}", working_dir, command);
        
        let output = Command::new("powershell")
            .args(&["-Command", &ps_command])
            .output()
            .map_err(|e| format!("Failed to execute PowerShell command: {}", e))?;
        
        Ok((
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
    
    /// Execute Bash command
    async fn execute_bash_command(
        &self,
        command: &str,
        working_dir: &str,
    ) -> Result<(Option<i32>, String, String), String> {
        let bash_command = format!("cd '{}' && {}", working_dir, command);
        
        let output = Command::new("bash")
            .args(&["-c", &bash_command])
            .output()
            .map_err(|e| format!("Failed to execute Bash command: {}", e))?;
        
        Ok((
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
    
    /// Execute sh command
    async fn execute_sh_command(
        &self,
        command: &str,
        working_dir: &str,
    ) -> Result<(Option<i32>, String, String), String> {
        let sh_command = format!("cd '{}' && {}", working_dir, command);
        
        let output = Command::new("sh")
            .args(&["-c", &sh_command])
            .output()
            .map_err(|e| format!("Failed to execute sh command: {}", e))?;
        
        Ok((
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
    
    /// Execute zsh command
    async fn execute_zsh_command(
        &self,
        command: &str,
        working_dir: &str,
    ) -> Result<(Option<i32>, String, String), String> {
        let zsh_command = format!("cd '{}' && {}", working_dir, command);
        
        let output = Command::new("zsh")
            .args(&["-c", &zsh_command])
            .output()
            .map_err(|e| format!("Failed to execute zsh command: {}", e))?;
        
        Ok((
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
    
    /// Execute fish command
    async fn execute_fish_command(
        &self,
        command: &str,
        working_dir: &str,
    ) -> Result<(Option<i32>, String, String), String> {
        let fish_command = format!("cd '{}'; {}", working_dir, command);
        
        let output = Command::new("fish")
            .args(&["-c", &fish_command])
            .output()
            .map_err(|e| format!("Failed to execute fish command: {}", e))?;
        
        Ok((
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
    
    /// Execute custom shell command
    async fn execute_custom_command(
        &self,
        shell: &str,
        command: &str,
        working_dir: &str,
    ) -> Result<(Option<i32>, String, String), String> {
        let shell_command = format!("cd '{}' && {}", working_dir, command);
        
        let output = Command::new(shell)
            .args(&["-c", &shell_command])
            .current_dir(working_dir)
            .output()
            .map_err(|e| format!("Failed to execute custom shell command: {}", e))?;
        
        Ok((
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
    
    /// Get terminal session info
    pub async fn get_session_info(&self, session_id: Uuid) -> Option<TerminalSessionInfo> {
        let sessions = self.sessions.read().await;
        sessions.get(&session_id).map(|session| TerminalSessionInfo {
            session_id: session.session_id,
            shell_type: session.shell_type.clone(),
            current_directory: session.current_directory.clone(),
            is_elevated: session.is_elevated,
            started_at: session.started_at,
            last_activity: session.last_activity,
            status: session.status.clone(),
        })
    }
    
    /// Get terminal output buffer
    pub async fn get_output_buffer(&self, session_id: Uuid, lines: Option<usize>) -> Vec<String> {
        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(&session_id) {
            let buffer = &session.output_buffer;
            if let Some(lines) = lines {
                let start = buffer.len().saturating_sub(lines);
                buffer[start..].to_vec()
            } else {
                buffer.clone()
            }
        } else {
            Vec::new()
        }
    }
    
    /// Get command history
    pub async fn get_command_history(
        &self,
        session_id: Option<Uuid>,
        user_id: Option<String>,
        limit: Option<usize>,
    ) -> Vec<CommandHistoryEntry> {
        let history = self.command_history.read().await;
        let mut filtered: Vec<_> = history.iter()
            .filter(|entry| {
                if let Some(ref sid) = session_id {
                    if entry.session_id != *sid {
                        return false;
                    }
                }
                if let Some(ref uid) = user_id {
                    if entry.user_id != *uid {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();
        
        filtered.sort_by(|a, b| b.executed_at.cmp(&a.executed_at));
        
        if let Some(limit) = limit {
            filtered.truncate(limit);
        }
        
        filtered
    }
    
    /// Close terminal session
    pub async fn close_session(&self, session_id: Uuid) -> Result<(), String> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(mut session) = sessions.remove(&session_id) {
            session.status = TerminalStatus::Terminated;
            info!("Closed terminal session {}", session_id);
            Ok(())
        } else {
            Err(format!("Terminal session {} not found", session_id))
        }
    }
    
    /// Get terminal configuration
    pub async fn get_config(&self) -> TerminalConfig {
        self.config.read().await.clone()
    }
}

/// WebSocket handler for real-time terminal interaction
pub async fn handle_terminal_websocket(
    mut socket: WebSocket,
    session_id: Uuid,
    terminal_manager: Arc<TerminalManager>,
) {
    info!("Starting WebSocket terminal session {}", session_id);
    
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<TerminalMessage>();
    
    // Spawn task to handle outgoing messages
    let tx_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            let msg_text = serde_json::to_string(&message).unwrap_or_default();
            if sender.send(Message::Text(msg_text)).await.is_err() {
                break;
            }
        }
    });
    
    // Handle incoming messages
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                match serde_json::from_str::<TerminalMessage>(&text) {
                    Ok(terminal_msg) => {
                        match terminal_msg.message_type {
                            TerminalMessageType::Input => {
                                if let Some(command) = terminal_msg.data.get("command").and_then(|c| c.as_str()) {
                                    match terminal_manager.execute_command(session_id, command.to_string()).await {
                                        Ok(output) => {
                                            let response = TerminalMessage {
                                                message_type: TerminalMessageType::Output,
                                                session_id,
                                                timestamp: chrono::Utc::now(),
                                                data: serde_json::json!({
                                                    "output": output
                                                }),
                                            };
                                            let _ = tx.send(response);
                                        }
                                        Err(error) => {
                                            let response = TerminalMessage {
                                                message_type: TerminalMessageType::Error,
                                                session_id,
                                                timestamp: chrono::Utc::now(),
                                                data: serde_json::json!({
                                                    "error": error
                                                }),
                                            };
                                            let _ = tx.send(response);
                                        }
                                    }
                                }
                            }
                            _ => {
                                debug!("Received terminal message type: {:?}", terminal_msg.message_type);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse terminal message: {}", e);
                    }
                }
            }
            Ok(Message::Close(_)) => {
                info!("Terminal WebSocket session {} closed", session_id);
                break;
            }
            Err(e) => {
                error!("Terminal WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }
    
    tx_task.abort();
}

/// API Handlers
/// Create terminal session
pub async fn api_create_terminal_session(
    State(app_state): State<AppState>,
    Path(client_session_id): Path<Uuid>,
    Json(request): Json<CreateTerminalRequest>,
) -> Response {
    match app_state.device_manager.terminal_manager.create_session(client_session_id, request).await {
        Ok(session) => Json(TerminalSessionInfo {
            session_id: session.session_id,
            shell_type: session.shell_type,
            current_directory: session.current_directory,
            is_elevated: session.is_elevated,
            started_at: session.started_at,
            last_activity: session.last_activity,
            status: session.status,
        }).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e
            }))
        ).into_response(),
    }
}

/// Get terminal session info
pub async fn api_get_terminal_session(
    State(app_state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Response {
    match app_state.device_manager.terminal_manager.get_session_info(session_id).await {
        Some(info) => Json(info).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Terminal session not found"
            }))
        ).into_response(),
    }
}

/// Get terminal output buffer
pub async fn api_get_terminal_output(
    State(app_state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let lines = params.get("lines")
        .and_then(|l| l.parse::<usize>().ok());
    
    let output = app_state.device_manager.terminal_manager.get_output_buffer(session_id, lines).await;
    Json(serde_json::json!({
        "output": output
    })).into_response()
}

/// Get command history
pub async fn api_get_command_history(
    State(app_state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let session_id = params.get("session_id")
        .and_then(|id| Uuid::parse_str(id).ok());
    let user_id = params.get("user_id").map(|id| id.to_string());
    let limit = params.get("limit")
        .and_then(|l| l.parse::<usize>().ok());
    
    let history = app_state.device_manager.terminal_manager.get_command_history(session_id, user_id, limit).await;
    Json(history).into_response()
}

/// Terminal WebSocket handler
pub async fn websocket_terminal_handler(
    ws: WebSocketUpgrade,
    State(app_state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Response {
    ws.on_upgrade(move |socket| {
        handle_terminal_websocket(socket, session_id, app_state.device_manager.terminal_manager)
    })
}

/// Get terminal configuration
pub async fn api_get_terminal_config(
    State(app_state): State<AppState>,
) -> impl IntoResponse {
    let config = app_state.device_manager.terminal_manager.get_config().await;
    Json(config)
}