use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::toolbox::ToolboxManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub device_name: String,
    pub operating_system: String,
    pub ip_address: String,
    pub user_name: Option<String>,
    pub connected_time: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub enum SessionTab {
    Start,      // Main remote desktop view
    General,    // Session information
    Timeline,   // Session history/events
    Messages,   // Chat messages
    Commands,   // Real-time command execution
    Notes,      // Session notes
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: Uuid,
    pub sender: String,
    pub message: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub is_technician: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionNote {
    pub id: Uuid,
    pub content: String,
    pub author: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub is_private: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub id: Uuid,
    pub event_type: String,
    pub description: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub details: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandExecution {
    pub id: Uuid,
    pub command: String,
    pub output: String,
    pub exit_code: Option<i32>,
    pub execution_time: chrono::DateTime<chrono::Utc>,
    pub duration_ms: u64,
    pub timeout_seconds: u32,
    pub max_length: usize,
    pub shell: Option<String>,
}

pub struct SessionWindow {
    pub session_info: SessionInfo,
    pub current_tab: SessionTab,
    pub toolbox: Arc<Mutex<ToolboxManager>>,
    
    // Tab data
    pub messages: Arc<RwLock<Vec<ChatMessage>>>,
    pub notes: Arc<RwLock<Vec<SessionNote>>>,
    pub timeline: Arc<RwLock<Vec<TimelineEvent>>>,
    pub command_history: Arc<RwLock<Vec<CommandExecution>>>,
    
    // Session state
    pub is_backstage_mode: bool,
    pub input_suspended: bool,
    pub screen_blanked: bool,
    pub is_recording: bool,
    
    // Connection
    pub server_url: String,
    pub auth_token: String,
}

impl SessionWindow {
    pub async fn new(
        session_id: String,
        server_url: String,
        auth_token: String,
        toolbox: ToolboxManager,
    ) -> Result<Self> {
        let session_info = SessionInfo {
            session_id: session_id.clone(),
            device_name: "Unknown".to_string(),
            operating_system: "Unknown".to_string(),
            ip_address: "0.0.0.0".to_string(),
            user_name: None,
            connected_time: chrono::Utc::now(),
            last_activity: chrono::Utc::now(),
        };
        
        Ok(Self {
            session_info,
            current_tab: SessionTab::Start,
            toolbox: Arc::new(Mutex::new(toolbox)),
            messages: Arc::new(RwLock::new(Vec::new())),
            notes: Arc::new(RwLock::new(Vec::new())),
            timeline: Arc::new(RwLock::new(Vec::new())),
            command_history: Arc::new(RwLock::new(Vec::new())),
            is_backstage_mode: false,
            input_suspended: false,
            screen_blanked: false,
            is_recording: false,
            server_url,
            auth_token,
        })
    }
    
    pub async fn switch_tab(&mut self, tab: SessionTab) {
        info!("Switching to tab: {:?}", tab);
        self.current_tab = tab;
    }
    
    pub async fn enable_backstage_mode(&mut self) -> Result<()> {
        info!("Enabling backstage mode for session {}", self.session_info.session_id);
        self.is_backstage_mode = true;
        self.add_timeline_event("backstage_enabled", "Backstage mode enabled").await;
        Ok(())
    }
    
    pub async fn disable_backstage_mode(&mut self) -> Result<()> {
        info!("Disabling backstage mode for session {}", self.session_info.session_id);
        self.is_backstage_mode = false;
        self.add_timeline_event("backstage_disabled", "Backstage mode disabled").await;
        Ok(())
    }
    
    pub async fn suspend_input(&mut self) -> Result<()> {
        info!("Suspending remote input");
        self.input_suspended = true;
        self.add_timeline_event("input_suspended", "Remote input suspended").await;
        Ok(())
    }
    
    pub async fn resume_input(&mut self) -> Result<()> {
        info!("Resuming remote input");
        self.input_suspended = false;
        self.add_timeline_event("input_resumed", "Remote input resumed").await;
        Ok(())
    }
    
    pub async fn blank_screen(&mut self) -> Result<()> {
        info!("Blanking remote screen");
        self.screen_blanked = true;
        self.add_timeline_event("screen_blanked", "Remote screen blanked").await;
        Ok(())
    }
    
    pub async fn unblank_screen(&mut self) -> Result<()> {
        info!("Unblanking remote screen");
        self.screen_blanked = false;
        self.add_timeline_event("screen_unblanked", "Remote screen restored").await;
        Ok(())
    }
    
    pub async fn send_message(&self, message: String, is_technician: bool) -> Result<()> {
        let chat_message = ChatMessage {
            id: Uuid::new_v4(),
            sender: if is_technician { "Technician".to_string() } else { self.session_info.user_name.clone().unwrap_or("User".to_string()) },
            message,
            timestamp: chrono::Utc::now(),
            is_technician,
        };
        
        self.messages.write().await.push(chat_message);
        self.add_timeline_event("message_sent", "Chat message sent").await;
        Ok(())
    }
    
    pub async fn add_note(&self, content: String, author: String, is_private: bool) -> Result<()> {
        let note = SessionNote {
            id: Uuid::new_v4(),
            content,
            author,
            timestamp: chrono::Utc::now(),
            is_private,
        };
        
        self.notes.write().await.push(note);
        self.add_timeline_event("note_added", "Session note added").await;
        Ok(())
    }
    
    pub async fn execute_command(
        &self,
        command: String,
        timeout_seconds: Option<u32>,
        max_length: Option<usize>,
        shell: Option<String>,
    ) -> Result<CommandExecution> {
        let start_time = std::time::Instant::now();
        let execution_time = chrono::Utc::now();
        
        // Parse command modifiers
        let timeout = timeout_seconds.unwrap_or(10); // Default 10 seconds like ScreenConnect
        let max_len = max_length.unwrap_or(8192);     // Default 8KB
        
        info!("Executing command: {} (timeout: {}s, max_length: {})", command, timeout, max_len);
        
        // TODO: Implement actual command execution via WebSocket to remote machine
        let output = format!("Command executed: {}\n(Implementation pending)", command);
        let exit_code = Some(0);
        
        let duration = start_time.elapsed();
        
        let execution = CommandExecution {
            id: Uuid::new_v4(),
            command: command.clone(),
            output,
            exit_code,
            execution_time,
            duration_ms: duration.as_millis() as u64,
            timeout_seconds: timeout,
            max_length: max_len,
            shell,
        };
        
        self.command_history.write().await.push(execution.clone());
        
        let event_details = HashMap::from([
            ("command".to_string(), command),
            ("duration_ms".to_string(), execution.duration_ms.to_string()),
        ]);
        
        self.add_timeline_event_with_details("command_executed", "Command executed", event_details).await;
        
        Ok(execution)
    }
    
    pub async fn launch_tool(&self, tool_name: String, args: Vec<String>) -> Result<()> {
        info!("Launching tool: {} with args: {:?}", tool_name, args);
        
        let toolbox = self.toolbox.lock().await;
        let tools = toolbox.list_tools();
        
        if let Some(tool) = tools.iter().find(|t| t.name == tool_name) {
            // TODO: Launch tool on remote machine or locally depending on tool type
            info!("Tool found: {} - {}", tool.name, tool.description);
            
            let event_details = HashMap::from([
                ("tool_name".to_string(), tool_name.clone()),
                ("args".to_string(), args.join(" ")),
            ]);
            
            self.add_timeline_event_with_details("tool_launched", &format!("Tool launched: {}", tool_name), event_details).await;
        } else {
            warn!("Tool not found: {}", tool_name);
        }
        
        Ok(())
    }
    
    async fn add_timeline_event(&self, event_type: &str, description: &str) {
        self.add_timeline_event_with_details(event_type, description, HashMap::new()).await;
    }
    
    async fn add_timeline_event_with_details(&self, event_type: &str, description: &str, details: HashMap<String, String>) {
        let event = TimelineEvent {
            id: Uuid::new_v4(),
            event_type: event_type.to_string(),
            description: description.to_string(),
            timestamp: chrono::Utc::now(),
            details,
        };
        
        self.timeline.write().await.push(event);
    }
    
    pub async fn get_session_summary(&self) -> HashMap<String, String> {
        let messages_count = self.messages.read().await.len();
        let notes_count = self.notes.read().await.len();
        let commands_count = self.command_history.read().await.len();
        let events_count = self.timeline.read().await.len();
        
        HashMap::from([
            ("session_id".to_string(), self.session_info.session_id.clone()),
            ("device_name".to_string(), self.session_info.device_name.clone()),
            ("operating_system".to_string(), self.session_info.operating_system.clone()),
            ("messages_count".to_string(), messages_count.to_string()),
            ("notes_count".to_string(), notes_count.to_string()),
            ("commands_count".to_string(), commands_count.to_string()),
            ("events_count".to_string(), events_count.to_string()),
            ("backstage_mode".to_string(), self.is_backstage_mode.to_string()),
            ("input_suspended".to_string(), self.input_suspended.to_string()),
            ("screen_blanked".to_string(), self.screen_blanked.to_string()),
            ("recording".to_string(), self.is_recording.to_string()),
        ])
    }
}