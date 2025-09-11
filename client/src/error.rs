use thiserror::Error;

/// Main error type for GhostLink client
#[derive(Error, Debug)]
pub enum GhostLinkError {
    #[error("Capture error: {0}")]
    Capture(#[from] CaptureError),
    
    #[error("Connection error: {0}")]
    Connection(#[from] ConnectionError),
    
    #[error("Session error: {0}")]
    Session(#[from] SessionError),
    
    #[error("Input error: {0}")]
    Input(#[from] InputError),
    
    #[error("Service error: {0}")]
    Service(#[from] ServiceError),
    
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    
    #[error("Encoding error: {0}")]
    Encode(String),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("X11 error: {0}")]
    X11(#[from] x11rb::errors::ReplyOrIdError),
    
    #[error("X11 connection error: {0}")]
    X11Connection(#[from] x11rb::errors::ConnectError),
    
    #[error("X11 reply error: {0}")]
    X11Reply(#[from] x11rb::errors::ReplyError),
    
    #[error("X11 generic error: {0}")]
    X11Generic(#[from] x11rb::errors::ConnectionError),
    
    #[error("{0}")]
    Other(String),
}

/// Screen capture specific errors
#[derive(Error, Debug)]
pub enum CaptureError {
    #[error("Platform not supported: {platform}")]
    UnsupportedPlatform { platform: String },
    
    #[error("Capturer not initialized")]
    NotInitialized,
    
    #[error("Display {display_id} not found")]
    DisplayNotFound { display_id: u32 },
    
    #[error("Invalid display ID: {id}")]
    InvalidDisplay { id: u32 },
    
    #[error("Failed to capture frame: {reason}")]
    FrameCaptureFailed { reason: String },
    
    #[error("Encoder not available: {encoder_type}")]
    EncoderUnavailable { encoder_type: String },
    
    #[error("Encoding failed: {reason}")]
    EncodingFailed { reason: String },
    
    #[error("Invalid resolution: {width}x{height}")]
    InvalidResolution { width: u32, height: u32 },
    
    #[error("Connection failed: {reason}")]
    ConnectionFailed { reason: String },
    
    #[error("Initialization failed: {reason}")]
    InitializationFailed { reason: String },
    
    #[error("Capture failed: {reason}")]
    CaptureFailed { reason: String },
    
    #[error("System dependency missing: {dependency}")]
    MissingDependency { dependency: String },
}

/// Connection related errors
#[derive(Error, Debug)]
pub enum ConnectionError {
    #[error("Not connected to server")]
    NotConnected,
    
    #[error("Connection failed: {reason}")]
    ConnectionFailed { reason: String },
    
    #[error("Authentication failed: {reason}")]
    AuthenticationFailed { reason: String },
    
    #[error("Invalid server URL: {url}")]
    InvalidServerUrl { url: String },
    
    #[error("Message send failed: {reason}")]
    SendFailed { reason: String },
    
    #[error("Heartbeat timeout")]
    HeartbeatTimeout,
    
    #[error("Protocol version mismatch: client={client}, server={server}")]
    ProtocolMismatch { client: String, server: String },
}

/// Session management errors
#[derive(Error, Debug)]
pub enum SessionError {
    #[error("Session {session_id} not found")]
    NotFound { session_id: String },
    
    #[error("Session limit exceeded: max={max_sessions}")]
    LimitExceeded { max_sessions: usize },
    
    #[error("Session type not supported: {session_type}")]
    UnsupportedType { session_type: String },
    
    #[error("Permission denied for session operation")]
    PermissionDenied,
    
    #[error("Screen capture not available for session")]
    CaptureNotAvailable,
    
    #[error("Session already active: {session_id}")]
    AlreadyActive { session_id: String },
}

/// Input control errors
#[derive(Error, Debug)]
pub enum InputError {
    #[error("Input system not initialized")]
    NotInitialized,
    
    #[error("Platform input method not available: {method}")]
    MethodUnavailable { method: String },
    
    #[error("Invalid input coordinates: ({x}, {y})")]
    InvalidCoordinates { x: i32, y: i32 },
    
    #[error("Key mapping failed for key: {key}")]
    KeyMappingFailed { key: String },
    
    #[error("Input blocked by system")]
    InputBlocked,
    
    #[error("Required tool missing: {tool}")]
    ToolMissing { tool: String },
}

/// Service management errors
#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("Service not installed")]
    NotInstalled,
    
    #[error("Platform not supported for service operations")]
    PlatformNotSupported,
    
    #[error("Service operation failed: {operation}")]
    OperationFailed { operation: String },
    
    #[error("Insufficient privileges for service operation")]
    InsufficientPrivileges,
    
    #[error("Service configuration invalid: {reason}")]
    ConfigurationInvalid { reason: String },
}

/// Configuration errors
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Configuration file not found: {path}")]
    FileNotFound { path: String },
    
    #[error("Invalid configuration format: {reason}")]
    InvalidFormat { reason: String },
    
    #[error("Missing required field: {field}")]
    MissingField { field: String },
    
    #[error("Invalid value for {field}: {value}")]
    InvalidValue { field: String, value: String },
}

impl From<anyhow::Error> for GhostLinkError {
    fn from(error: anyhow::Error) -> Self {
        GhostLinkError::Other(error.to_string())
    }
}

impl From<String> for GhostLinkError {
    fn from(error: String) -> Self {
        GhostLinkError::Other(error)
    }
}

impl From<&str> for GhostLinkError {
    fn from(error: &str) -> Self {
        GhostLinkError::Other(error.to_string())
    }
}

/// Type alias for Results using GhostLinkError
pub type Result<T> = std::result::Result<T, GhostLinkError>;