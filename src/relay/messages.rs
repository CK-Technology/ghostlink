use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// WebSocket message types for relay communication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RelayMessage {
    // Authentication
    Authenticate { token: String },
    AuthResult { success: bool, message: String, user_id: Option<Uuid> },
    
    // Agent registration and heartbeat
    AgentRegister { agent_info: AgentInfo },
    AgentHeartbeat { agent_id: Uuid, status: AgentStatus },
    
    // Session management
    SessionRequest { 
        session_id: Uuid,
        agent_id: Uuid,
        session_type: crate::models::SessionType,
        user_id: Uuid,
    },
    SessionAccept { session_id: Uuid, connection_info: ConnectionInfo },
    SessionReject { session_id: Uuid, reason: String },
    SessionEnd { session_id: Uuid },
    
    // Screen sharing
    ScreenFrame { 
        session_id: Uuid,
        frame_data: Vec<u8>,
        width: u32,
        height: u32,
        format: ImageFormat,
        timestamp: DateTime<Utc>,
    },
    ScreenControl {
        session_id: Uuid,
        event: ControlEvent,
    },
    
    // File transfer
    FileTransferStart {
        session_id: Uuid,
        file_id: Uuid,
        filename: String,
        file_size: u64,
        checksum: String,
    },
    FileChunk {
        session_id: Uuid,
        file_id: Uuid,
        chunk_id: u32,
        data: Vec<u8>,
    },
    FileTransferComplete {
        session_id: Uuid,
        file_id: Uuid,
        success: bool,
    },
    
    // Chat
    ChatMessage {
        session_id: Uuid,
        user_id: Uuid,
        message: String,
        timestamp: DateTime<Utc>,
    },
    
    // Error handling
    Error { code: u32, message: String },
    
    // Keep alive
    Ping { timestamp: DateTime<Utc> },
    Pong { timestamp: DateTime<Utc> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: Uuid,
    pub name: String,
    pub hostname: String,
    pub platform: String,
    pub architecture: String,
    pub version: String,
    pub public_key: String,
    pub capabilities: Vec<AgentCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentCapability {
    ScreenShare,
    FileTransfer,
    Shell,
    Chat,
    MultiMonitor,
    Audio,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatus {
    pub cpu_usage: f32,
    pub memory_usage: f32,
    pub disk_usage: f32,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub active_sessions: u32,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub relay_endpoint: String,
    pub session_key: String,
    pub encryption_method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImageFormat {
    Jpeg,
    Png,
    WebP,
    Raw,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlEvent {
    MouseMove { x: i32, y: i32 },
    MouseClick { x: i32, y: i32, button: MouseButton },
    MouseScroll { x: i32, y: i32, delta_x: i32, delta_y: i32 },
    KeyPress { key: String, modifiers: Vec<KeyModifier> },
    KeyRelease { key: String, modifiers: Vec<KeyModifier> },
    ClipboardUpdate { content: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyModifier {
    Ctrl,
    Alt,
    Shift,
    Meta,
    CapsLock,
    NumLock,
}
