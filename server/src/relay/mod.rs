//! GhostLink Relay Module
//!
//! Handles WebSocket connections between agents (remote devices) and technicians (operators).
//! Supports both direct P2P connections and relayed connections through the server.

use anyhow::Result;
use axum::extract::ws::{Message, WebSocket};
use chrono::{DateTime, Utc};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::device_manager::DeviceManager;

pub mod connection_broker;
pub mod load_balancer;
pub mod rendezvous;

// ============================================================================
// Relay Message Types
// ============================================================================

/// Messages sent between agents and technicians through the relay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayMessage {
    pub id: Uuid,
    pub message_type: RelayMessageType,
    pub payload: Vec<u8>,
    pub timestamp: DateTime<Utc>,
    pub priority: MessagePriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelayMessageType {
    // Screen sharing
    ScreenFrame,
    ScreenConfig,

    // Input forwarding
    MouseEvent,
    KeyboardEvent,
    TouchEvent,

    // Clipboard
    ClipboardData,
    ClipboardRequest,

    // File transfer
    FileChunk,
    FileMetadata,
    FileTransferControl,

    // Control
    SessionStart,
    SessionEnd,
    ConnectionInfo,

    // Health & Monitoring
    Heartbeat,
    LatencyProbe,
    QualityReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessagePriority {
    Critical = 0, // Control commands, errors
    High = 1,     // Input events, tool execution
    Normal = 2,   // Video frames, clipboard
    Low = 3,      // Heartbeats, background data
}

// ============================================================================
// Relay Node & Session Routing
// ============================================================================

/// A relay node in the distributed relay network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayNode {
    pub id: Uuid,
    pub address: SocketAddr,
    pub region: String,
    pub capacity: u32,
    pub current_load: u32,
    pub health_score: f32,
    pub last_heartbeat: DateTime<Utc>,
    pub features: Vec<String>,
}

/// Session routing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRoute {
    pub session_id: String,
    pub agent_id: String,
    pub technician_id: String,
    pub relay_node: Option<Uuid>,
    pub connection_type: ConnectionType,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionType {
    Direct,      // P2P connection
    RelayedTcp,  // Through relay via TCP
    RelayedUdp,  // Through relay via UDP (faster, less reliable)
    Hybrid,      // Try UDP first, fall back to TCP
}

// ============================================================================
// WebSocket Handlers (used by main.rs)
// ============================================================================

/// Handle WebSocket connections for devices (agents)
pub async fn handle_websocket(
    socket: WebSocket,
    agent_id: String,
    session_type: String,
    device_manager: Arc<DeviceManager>,
) {
    info!(
        "Agent WebSocket connected: {} (type: {})",
        agent_id, session_type
    );

    // Parse agent UUID
    let agent_uuid = match Uuid::parse_str(&agent_id) {
        Ok(uuid) => uuid,
        Err(e) => {
            error!("Invalid agent ID {}: {}", agent_id, e);
            return;
        }
    };

    // Split socket into sender and receiver
    let (mut sender, mut receiver) = socket.split();

    // Create channel for sending messages to this socket
    let (_tx, mut rx) = mpsc::unbounded_channel::<Message>();

    // Spawn task to forward messages from channel to socket sender
    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages from agent
    let device_manager_clone = device_manager.clone();
    let agent_id_clone = agent_id.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(result) = receiver.next().await {
            match result {
                Ok(msg) => {
                    if let Err(e) =
                        handle_agent_message(&device_manager_clone, &agent_id_clone, msg).await
                    {
                        warn!("Error handling agent message: {}", e);
                    }
                }
                Err(e) => {
                    error!("WebSocket error from agent {}: {}", agent_id_clone, e);
                    break;
                }
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    // Cleanup
    device_manager.disconnect_device(agent_uuid).await;
    info!("Agent WebSocket disconnected: {}", agent_id);
}

/// Handle WebSocket connections for sessions (technicians viewing/controlling agents)
pub async fn handle_session_websocket(
    socket: WebSocket,
    session_id: String,
    session_type: String,
    device_manager: Arc<DeviceManager>,
) {
    info!(
        "Session WebSocket connected: {} (type: {})",
        session_id, session_type
    );

    // Parse session UUID
    let session_uuid = match Uuid::parse_str(&session_id) {
        Ok(uuid) => uuid,
        Err(e) => {
            error!("Invalid session ID {}: {}", session_id, e);
            return;
        }
    };

    // Split socket into sender and receiver
    let (mut sender, mut receiver) = socket.split();

    // Create channel for sending messages to this socket
    let (_tx, mut rx) = mpsc::unbounded_channel::<Message>();

    // Spawn task to forward messages from channel to socket sender
    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages from technician
    let device_manager_clone = device_manager.clone();
    let session_id_clone = session_id.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(result) = receiver.next().await {
            match result {
                Ok(msg) => {
                    if let Err(e) =
                        handle_session_message(&device_manager_clone, &session_id_clone, msg).await
                    {
                        warn!("Error handling session message: {}", e);
                    }
                }
                Err(e) => {
                    error!("WebSocket error from session {}: {}", session_id_clone, e);
                    break;
                }
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    // Cleanup
    let _ = device_manager.end_session(session_uuid).await;
    info!("Session WebSocket disconnected: {}", session_id);
}

// ============================================================================
// Message Handlers
// ============================================================================

/// Handle messages received from agents
async fn handle_agent_message(
    device_manager: &Arc<DeviceManager>,
    agent_id: &str,
    message: Message,
) -> Result<()> {
    match message {
        Message::Binary(data) => {
            // Binary data is typically screen frames
            if let Ok(agent_uuid) = Uuid::parse_str(agent_id) {
                device_manager
                    .broadcast_screen_frame(agent_uuid, data)
                    .await;
            }
        }
        Message::Text(text) => {
            // Text messages are typically control commands
            debug!("Agent {} sent text: {}", agent_id, text);

            // Parse as JSON command
            if let Ok(cmd) = serde_json::from_str::<serde_json::Value>(&text) {
                handle_agent_command(device_manager, agent_id, cmd).await?;
            }
        }
        Message::Ping(_data) => {
            // Update heartbeat
            if let Ok(agent_uuid) = Uuid::parse_str(agent_id) {
                let _ = device_manager.update_device_heartbeat(agent_uuid).await;
            }
            debug!("Ping from agent {}", agent_id);
        }
        Message::Pong(_) => {
            debug!("Pong from agent {}", agent_id);
        }
        Message::Close(_) => {
            info!("Agent {} requested close", agent_id);
        }
    }
    Ok(())
}

/// Handle messages received from sessions (technicians)
async fn handle_session_message(
    device_manager: &Arc<DeviceManager>,
    session_id: &str,
    message: Message,
) -> Result<()> {
    match message {
        Message::Binary(data) => {
            // Binary data from technician is typically input events
            if let Ok(session_uuid) = Uuid::parse_str(session_id) {
                if let Err(e) = device_manager
                    .forward_input_event(session_uuid, data)
                    .await
                {
                    warn!("Failed to forward input: {}", e);
                }
            }
        }
        Message::Text(text) => {
            // Text messages are typically control commands
            debug!("Session {} sent text: {}", session_id, text);

            if let Ok(cmd) = serde_json::from_str::<serde_json::Value>(&text) {
                handle_session_command(device_manager, session_id, cmd).await?;
            }
        }
        Message::Ping(_) => {
            debug!("Ping from session {}", session_id);
        }
        Message::Pong(_) => {
            debug!("Pong from session {}", session_id);
        }
        Message::Close(_) => {
            info!("Session {} requested close", session_id);
        }
    }
    Ok(())
}

/// Handle agent control commands
async fn handle_agent_command(
    device_manager: &Arc<DeviceManager>,
    agent_id: &str,
    cmd: serde_json::Value,
) -> Result<()> {
    let cmd_type = cmd.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match cmd_type {
        "heartbeat" => {
            if let Ok(agent_uuid) = Uuid::parse_str(agent_id) {
                let _ = device_manager.update_device_heartbeat(agent_uuid).await;
            }
        }
        "capabilities" => {
            // Agent is reporting its capabilities
            debug!("Agent {} capabilities: {:?}", agent_id, cmd.get("data"));
        }
        "screen_config" => {
            // Agent is reporting screen configuration
            debug!("Agent {} screen config: {:?}", agent_id, cmd.get("data"));
        }
        "error" => {
            warn!(
                "Agent {} error: {:?}",
                agent_id,
                cmd.get("message").and_then(|v| v.as_str())
            );
        }
        _ => {
            debug!("Unknown command from agent {}: {}", agent_id, cmd_type);
        }
    }
    Ok(())
}

/// Handle session control commands
async fn handle_session_command(
    _device_manager: &Arc<DeviceManager>,
    session_id: &str,
    cmd: serde_json::Value,
) -> Result<()> {
    let cmd_type = cmd.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match cmd_type {
        "request_screen" => {
            // Technician is requesting screen capture
            debug!("Session {} requesting screen", session_id);
        }
        "set_quality" => {
            // Technician is adjusting quality settings
            let quality = cmd.get("quality").and_then(|v| v.as_u64()).unwrap_or(80);
            debug!("Session {} set quality: {}", session_id, quality);
        }
        "request_control" => {
            // Technician is requesting control access
            debug!("Session {} requesting control", session_id);
        }
        "release_control" => {
            // Technician is releasing control
            debug!("Session {} releasing control", session_id);
        }
        _ => {
            debug!(
                "Unknown command from session {}: {}",
                session_id, cmd_type
            );
        }
    }
    Ok(())
}

// ============================================================================
// Relay Manager (for advanced routing)
// ============================================================================

/// Manages relay connections and session routing.
/// This is used for advanced routing when P2P connections fail.
#[allow(dead_code)]
pub struct RelayManager {
    /// Active sessions indexed by session ID
    sessions: Arc<RwLock<HashMap<String, SessionRoute>>>,
    /// Available relay nodes
    relay_nodes: Arc<RwLock<HashMap<Uuid, RelayNode>>>,
    /// Load balancer for optimal routing
    load_balancer: Arc<load_balancer::LoadBalancer>,
}

#[allow(dead_code)]
impl RelayManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            relay_nodes: Arc::new(RwLock::new(HashMap::new())),
            load_balancer: Arc::new(load_balancer::LoadBalancer::new()),
        }
    }

    /// Create an optimal route for a session
    pub async fn create_route(
        &self,
        session_id: String,
        agent_id: String,
        technician_id: String,
    ) -> Result<SessionRoute> {
        // For now, create a direct relay route
        // TODO: Implement P2P detection and optimal relay selection
        let route = SessionRoute {
            session_id: session_id.clone(),
            agent_id,
            technician_id,
            relay_node: None, // Direct through server
            connection_type: ConnectionType::RelayedTcp,
            created_at: Utc::now(),
            last_activity: Utc::now(),
        };

        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id, route.clone());

        Ok(route)
    }

    /// Get session route
    pub async fn get_route(&self, session_id: &str) -> Option<SessionRoute> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }

    /// Remove session route
    pub async fn remove_route(&self, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
    }

    /// Get all active sessions
    pub async fn get_all_sessions(&self) -> Vec<SessionRoute> {
        let sessions = self.sessions.read().await;
        sessions.values().cloned().collect()
    }

    /// Register a relay node
    pub async fn register_relay_node(&self, node: RelayNode) {
        let mut nodes = self.relay_nodes.write().await;
        nodes.insert(node.id, node);
    }

    /// Get relay node stats
    pub async fn get_relay_stats(&self) -> HashMap<Uuid, RelayNode> {
        let nodes = self.relay_nodes.read().await;
        nodes.clone()
    }
}

impl Default for RelayManager {
    fn default() -> Self {
        Self::new()
    }
}
