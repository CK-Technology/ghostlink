use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, warn, trace};
use url::Url;

use crate::agent::heartbeat::{HeartbeatManager, HeartbeatMessage};
use crate::config::ClientConfig;

// pub mod auth;
// pub mod reconnect;
pub mod p2p;\npub mod hybrid;\npub mod monitor_protocol;\n\npub use p2p::{P2PManager, P2PConnectionInfo, NATType};\npub use hybrid::{HybridConnectionManager, ConnectionType, ConnectionSettings};

pub type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// WebSocket connection to AtlasConnect server
pub struct RelayConnection {
    config: ClientConfig,
    ws_stream: Arc<RwLock<Option<WsStream>>>,
    heartbeat_manager: Arc<RwLock<HeartbeatManager>>,
    message_tx: mpsc::Sender<RelayMessage>,
    message_rx: mpsc::Receiver<RelayMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RelayMessage {
    // Agent registration
    AgentRegister {
        agent_id: String,
        hostname: String,
        os_info: serde_json::Value,
        capabilities: Vec<String>,
    },
    
    // Authentication
    Authenticate {
        token: String,
    },
    
    // Heartbeat
    Heartbeat {
        data: HeartbeatMessage,
    },
    
    // Session management
    SessionRequest {
        session_id: String,
        session_type: String,
        requester: String,
    },
    
    SessionResponse {
        session_id: String,
        accepted: bool,
        reason: Option<String>,
    },
    
    SessionEnd {
        session_id: String,
    },
    
    // Screen capture
    ScreenFrame {
        session_id: String,
        frame_data: Vec<u8>,
        width: u32,
        height: u32,
        format: String,
    },
    
    // Input control
    InputEvent {
        session_id: String,
        event_type: String,
        data: serde_json::Value,
    },
    
    // File transfer
    FileTransfer {
        session_id: String,
        file_data: Vec<u8>,
        filename: String,
        total_size: u64,
        chunk_index: u32,
        total_chunks: u32,
    },
    
    // Monitor control
    MonitorControl {
        session_id: String,
        data: serde_json::Value,
    },
    
    
    // P2P connection exchange (RustDesk-style)
    P2PHandshake {
        session_id: String,
        connection_info: P2PConnectionInfo,
    },
    
    P2PResponse {
        session_id: String,
        accepted: bool,
        connection_info: Option<P2PConnectionInfo>,
    },
    
    // Clipboard sync (RustDesk feature)
    ClipboardSync {
        session_id: String,
        content: String,
        content_type: String,
    },
    // Control messages
    Ping,
    Pong,
    
    // Error handling
    Error {
        code: u32,
        message: String,
    },
}

impl RelayConnection {
    pub async fn new(config: &ClientConfig) -> Result<Self> {
        let (message_tx, message_rx) = mpsc::channel(100);
        
        let heartbeat_manager = Arc::new(RwLock::new(HeartbeatManager::new(
            config.heartbeat_interval,
        )));
        
        let mut connection = Self {
            config: config.clone(),
            ws_stream: Arc::new(RwLock::new(None)),
            heartbeat_manager,
            message_tx,
            message_rx,
        };
        
        connection.connect().await?;
        connection.start_message_handler().await?;
        
        Ok(connection)
    }

    /// Establish WebSocket connection to server
    async fn connect(&self) -> Result<()> {
        let url = Url::parse(&self.config.server_url)
            .context("Invalid server URL")?;
        
        info!("Connecting to WebSocket: {}", url);
        
        let (ws_stream, response) = connect_async(&url).await
            .context("Failed to connect to WebSocket")?;
        
        info!("WebSocket connected, response: {}", response.status());
        
        let mut stream_guard = self.ws_stream.write().await;
        *stream_guard = Some(ws_stream);
        
        // Send initial registration
        self.register_agent().await?;
        
        Ok(())
    }

    /// Register this agent with the server
    async fn register_agent(&self) -> Result<()> {
        let system_info = self.get_system_info();
        
        let register_msg = RelayMessage::AgentRegister {
            agent_id: self.config.agent_id.clone(),
            hostname: self.config.hostname.clone(),
            os_info: system_info,
            capabilities: vec![
                "screen_capture".to_string(),
                "input_control".to_string(),
                "file_transfer".to_string(),
                "multi_monitor".to_string(),
                "monitor_selection".to_string(),
                "high_fps_capture".to_string(),
                "session_recording".to_string(),
            ],
        };
        
        self.send_message(register_msg).await?;
        info!("Agent registration sent");
        
        Ok(())
    }

    /// Start background task to handle incoming messages
    async fn start_message_handler(&self) -> Result<()> {
        let ws_stream = Arc::clone(&self.ws_stream);
        let heartbeat_manager = Arc::clone(&self.heartbeat_manager);
        
        tokio::spawn(async move {
            loop {
                let mut stream_guard = ws_stream.write().await;
                
                if let Some(ref mut stream) = *stream_guard {
                    match stream.next().await {
                        Some(Ok(Message::Text(text))) => {
                            if let Err(e) = Self::handle_text_message(&text).await {
                                error!("Error handling text message: {}", e);
                            }
                        }
                        Some(Ok(Message::Binary(data))) => {
                            if let Err(e) = Self::handle_binary_message(&data).await {
                                error!("Error handling binary message: {}", e);
                            }
                        }
                        Some(Ok(Message::Frame(_))) => {
                            // Handle frame messages if needed
                            debug!("Received frame message (not yet implemented)");
                        }
                        Some(Ok(Message::Ping(data))) => {
                            if let Err(e) = stream.send(Message::Pong(data)).await {
                                error!("Failed to send pong: {}", e);
                            }
                        }
                        Some(Ok(Message::Pong(_))) => {
                            let mut hb_guard = heartbeat_manager.write().await;
                            hb_guard.record_success();
                        }
                        Some(Ok(Message::Close(_))) => {
                            warn!("WebSocket connection closed by server");
                            break;
                        }
                        Some(Err(e)) => {
                            error!("WebSocket error: {}", e);
                            break;
                        }
                        None => {
                            warn!("WebSocket stream ended");
                            break;
                        }
                    }
                } else {
                    break;
                }
            }
        });
        
        Ok(())
    }

    /// Handle incoming text messages
    async fn handle_text_message(text: &str) -> Result<()> {
        debug!("Received text message: {}", text);
        
        let message: RelayMessage = serde_json::from_str(text)
            .context("Failed to parse relay message")?;
        
        match message {
            RelayMessage::SessionRequest { session_id, session_type, requester } => {
                info!("Received session request: {} ({}) from {}", session_id, session_type, requester);
                // TODO: Handle session request
            }
            RelayMessage::SessionEnd { session_id } => {
                info!("Session ended: {}", session_id);
                // TODO: Clean up session
            }
            RelayMessage::InputEvent { session_id, event_type, data } => {
                debug!("Input event for session {}: {} {:?}", session_id, event_type, data);
                
                // Forward to input service if available
                // TODO: Get input service from session manager and forward event
                if let Ok(json_str) = serde_json::to_string(&data) {
                    trace!("Input event JSON: {}", json_str);
                    // The actual input processing will be handled by InputService
                }
            }
            RelayMessage::MonitorControl { session_id, data } => {
                debug!("Monitor control message for session {}: {:?}", session_id, data);
                
                // Parse monitor control message
                if let Ok(monitor_msg) = serde_json::from_value::<crate::connection::monitor_protocol::MonitorControlMessage>(data) {
                    info!("Processing monitor control: {:?}", monitor_msg.message_type());
                    // TODO: Forward to monitor manager for processing
                } else {
                    warn!("Failed to parse monitor control message");
                }
            }
            RelayMessage::Ping => {
                // Ping handled automatically by WebSocket protocol
            }
            RelayMessage::Error { code, message } => {
                error!("Server error {}: {}", code, message);
            }
            _ => {
                debug!("Unhandled message type: {:?}", message);
            }
        }
        
        Ok(())
    }

    /// Handle incoming binary messages (frame data from server)
    async fn handle_binary_message(data: &[u8]) -> Result<()> {
        use crate::capture::frame_protocol::FrameMessage;
        
        debug!("Received binary message: {} bytes", data.len());
        
        // Try to parse as frame message
        match FrameMessage::deserialize_binary(data) {
            Ok(frame_msg) => {
                let info = frame_msg.get_info();
                debug!("Received frame {}: {}x{} {} bytes codec={:?}", 
                    info.sequence, info.width, info.height, 
                    info.data_size, info.codec);
                
                // TODO: Handle received frame (for future client-to-client scenarios)
                // This would be used if this client is receiving frames from another client
            }
            Err(e) => {
                debug!("Binary message is not a frame: {}", e);
                // Could be other binary data like file transfers
            }
        }
        
        Ok(())
    }

    /// Send a message to the server
    pub async fn send_message(&self, message: RelayMessage) -> Result<()> {
        let json = serde_json::to_string(&message)
            .context("Failed to serialize message")?;
        
        let mut stream_guard = self.ws_stream.write().await;
        
        if let Some(ref mut stream) = *stream_guard {
            stream.send(Message::Text(json)).await
                .context("Failed to send message")?;
            debug!("Sent message: {:?}", message);
        } else {
            return Err(anyhow::anyhow!("WebSocket not connected"));
        }
        
        Ok(())
    }
    
    /// Send binary frame data directly (more efficient for video frames)
    pub async fn send_binary_frame(&self, frame_data: Vec<u8>) -> Result<()> {
        let mut stream_guard = self.ws_stream.write().await;
        
        if let Some(ref mut stream) = *stream_guard {
            stream.send(Message::Binary(frame_data)).await
                .context("Failed to send binary frame")?;
            trace!("Sent binary frame: {} bytes", frame_data.len());
        } else {
            return Err(anyhow::anyhow!("WebSocket not connected"));
        }
        
        Ok(())
    }

    /// Send heartbeat to server
    pub async fn send_heartbeat(&self) -> Result<()> {
        let heartbeat_data = HeartbeatMessage::new(
            self.config.agent_id.clone(),
            0, // TODO: Get actual active session count
        );
        
        let heartbeat_msg = RelayMessage::Heartbeat {
            data: heartbeat_data,
        };
        
        match self.send_message(heartbeat_msg).await {
            Ok(()) => {
                let mut hb_guard = self.heartbeat_manager.write().await;
                hb_guard.record_success();
                Ok(())
            }
            Err(e) => {
                let mut hb_guard = self.heartbeat_manager.write().await;
                hb_guard.record_failure();
                Err(e)
            }
        }
    }

    /// Get system information for registration
    fn get_system_info(&self) -> serde_json::Value {
        use sysinfo::System;
        
        let mut sys = System::new_all();
        sys.refresh_all();
        
        serde_json::json!({
            "os": format!("{} {}", System::name().unwrap_or_default(), System::os_version().unwrap_or_default()),
            "kernel": System::kernel_version().unwrap_or_default(),
            "cpu": {
                "brand": sys.cpus().first().map(|cpu| cpu.brand()).unwrap_or("Unknown"),
                "cores": sys.cpus().len(),
            },
            "memory": {
                "total": sys.total_memory(),
                "available": sys.available_memory(),
            },
            "uptime": System::uptime(),
            "agent_version": env!("CARGO_PKG_VERSION"),
        })
    }

    /// Disconnect from server
    pub async fn disconnect(&self) -> Result<()> {
        info!("Disconnecting from server");
        
        let mut stream_guard = self.ws_stream.write().await;
        
        if let Some(mut stream) = stream_guard.take() {
            let _ = stream.close(None).await;
            info!("WebSocket connection closed");
        }
        
        Ok(())
    }

    /// Check connection health
    pub async fn is_healthy(&self) -> bool {
        let stream_guard = self.ws_stream.read().await;
        let hb_guard = self.heartbeat_manager.read().await;
        
        stream_guard.is_some() && !hb_guard.is_connection_dead()
    }
}
