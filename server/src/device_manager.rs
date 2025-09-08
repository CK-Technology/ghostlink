use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug};
use axum::extract::ws::Message;

use crate::models::{Agent, Session, SessionType, SessionStatus};
use crate::toolbox::ToolboxManager;
use crate::branding::BrandingManager;
use crate::direct_connect::DirectConnectManager;
use crate::vpn_integration::VpnManager;
use crate::auth::oidc::OidcManager;
use crate::pam::PamManager;
use crate::terminal::TerminalManager;

/// Device connection state
#[derive(Debug, Clone)]
pub struct DeviceConnection {
    pub agent: Agent,
    pub tx: mpsc::UnboundedSender<Message>,
    pub last_ping: DateTime<Utc>,
    pub connection_time: DateTime<Utc>,
    pub active_sessions: Vec<Uuid>,
}

/// Session connection for web clients
#[derive(Debug, Clone)]
pub struct SessionConnection {
    pub session: Session,
    pub tx: mpsc::UnboundedSender<Message>,
    pub connection_time: DateTime<Utc>,
}

/// Manages all device connections and sessions
pub struct DeviceManager {
    /// Connected devices indexed by agent ID
    devices: Arc<RwLock<HashMap<Uuid, DeviceConnection>>>,
    
    /// Active sessions indexed by session ID
    sessions: Arc<RwLock<HashMap<Uuid, SessionConnection>>>,
    
    /// Channel for broadcasting messages between devices and sessions
    broadcast_tx: mpsc::UnboundedSender<BroadcastMessage>,
    broadcast_rx: Arc<RwLock<mpsc::UnboundedReceiver<BroadcastMessage>>>,
    
    /// Toolbox manager for tool deployment and execution
    pub toolbox_manager: Arc<ToolboxManager>,
    
    /// Branding manager for connection banners and themes
    pub branding_manager: Arc<BrandingManager>,
    
    /// Direct connect manager for peer-to-peer connections
    pub direct_connect_manager: Arc<DirectConnectManager>,
    
    /// VPN integration manager
    pub vpn_manager: Arc<VpnManager>,
    
    /// OIDC authentication manager
    pub oidc_manager: Arc<OidcManager>,
    
    /// PAM (Privileged Access Management) manager
    pub pam_manager: Arc<PamManager>,
    
    /// Terminal manager for web-based command execution
    pub terminal_manager: Arc<TerminalManager>,
}

/// Messages that can be broadcast between components
#[derive(Debug, Clone)]
pub enum BroadcastMessage {
    DeviceConnected(Uuid),
    DeviceDisconnected(Uuid),
    SessionStarted(Uuid, Uuid), // session_id, agent_id
    SessionEnded(Uuid),
    ScreenFrame(Uuid, Vec<u8>), // agent_id, frame_data
    InputEvent(Uuid, Vec<u8>),  // agent_id, input_data
}

/// Device registration request
#[derive(Debug, Deserialize)]
pub struct DeviceRegistration {
    pub name: Option<String>,
    pub hostname: String,
    pub platform: String,
    pub architecture: String,
    pub version: String,
    pub public_key: Option<String>,
    pub agent_id: Option<String>,
}

/// Session creation request
#[derive(Debug, Deserialize)]
pub struct SessionRequest {
    pub agent_id: Uuid,
    pub session_type: SessionType,
    pub user_id: Uuid,
}

impl DeviceManager {
    pub fn new() -> Self {
        let (broadcast_tx, broadcast_rx) = mpsc::unbounded_channel();
        
        Self {
            devices: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            broadcast_tx,
            broadcast_rx: Arc::new(RwLock::new(broadcast_rx)),
            toolbox_manager: Arc::new(ToolboxManager::new()),
            branding_manager: Arc::new(BrandingManager::new()),
            direct_connect_manager: Arc::new(DirectConnectManager::new()),
            vpn_manager: Arc::new(VpnManager::new()),
            oidc_manager: Arc::new(OidcManager::new()),
            pam_manager: Arc::new(PamManager::new()),
            terminal_manager: Arc::new(TerminalManager::new()),
        }
    }
    
    /// Initialize all managers
    pub async fn initialize(&self) -> Result<(), String> {
        info!("Initializing device manager and sub-managers");
        
        // Initialize all managers in parallel
        let init_results = tokio::join!(
            self.toolbox_manager.initialize(),
            self.branding_manager.initialize(),
            self.vpn_manager.initialize(),
            self.oidc_manager.initialize(),
            self.pam_manager.initialize(),
            self.terminal_manager.initialize()
        );
        
        // Check for any initialization errors
        init_results.0?;
        init_results.1?;
        init_results.2?;
        init_results.3?;
        init_results.4?;
        init_results.5?;
        
        info!("All device managers initialized successfully");
        Ok(())
    }

    /// Register a new device connection
    pub async fn register_device(
        &self,
        registration: DeviceRegistration,
        tx: mpsc::UnboundedSender<Message>,
    ) -> Result<Uuid, String> {
        let agent_id = if let Some(id_str) = registration.agent_id {
            Uuid::parse_str(&id_str).map_err(|e| format!("Invalid agent ID: {}", e))?
        } else {
            Uuid::new_v4()
        };

        let agent = Agent {
            id: agent_id,
            name: registration.name.unwrap_or_else(|| registration.hostname.clone()),
            hostname: registration.hostname,
            platform: registration.platform,
            architecture: registration.architecture,
            version: registration.version,
            public_key: registration.public_key.unwrap_or_default(),
            last_seen: Some(Utc::now()),
            is_online: true,
            owner_id: Uuid::new_v4(), // TODO: Get from authentication context
            group_id: None,
            tags: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let connection = DeviceConnection {
            agent: agent.clone(),
            tx,
            last_ping: Utc::now(),
            connection_time: Utc::now(),
            active_sessions: Vec::new(),
        };

        let mut devices = self.devices.write().await;
        devices.insert(agent_id, connection);
        drop(devices);

        info!("Device registered: {} ({})", agent.name, agent_id);
        
        // Broadcast device connection
        let _ = self.broadcast_tx.send(BroadcastMessage::DeviceConnected(agent_id));

        Ok(agent_id)
    }

    /// Remove a device connection
    pub async fn disconnect_device(&self, agent_id: Uuid) {
        let mut devices = self.devices.write().await;
        if let Some(connection) = devices.remove(&agent_id) {
            info!("Device disconnected: {} ({})", connection.agent.name, agent_id);
            
            // End all active sessions for this device
            let mut sessions = self.sessions.write().await;
            let session_ids_to_remove: Vec<Uuid> = sessions
                .iter()
                .filter(|(_, session_conn)| session_conn.session.agent_id == agent_id)
                .map(|(session_id, _)| *session_id)
                .collect();

            for session_id in session_ids_to_remove {
                sessions.remove(&session_id);
                let _ = self.broadcast_tx.send(BroadcastMessage::SessionEnded(session_id));
            }

            let _ = self.broadcast_tx.send(BroadcastMessage::DeviceDisconnected(agent_id));
        }
    }

    /// Update device heartbeat
    pub async fn update_device_heartbeat(&self, agent_id: Uuid) -> Result<(), String> {
        let mut devices = self.devices.write().await;
        if let Some(connection) = devices.get_mut(&agent_id) {
            connection.last_ping = Utc::now();
            debug!("Heartbeat updated for device: {}", agent_id);
            Ok(())
        } else {
            Err(format!("Device not found: {}", agent_id))
        }
    }

    /// Create a new session
    pub async fn create_session(
        &self,
        request: SessionRequest,
        tx: mpsc::UnboundedSender<Message>,
    ) -> Result<Uuid, String> {
        // Verify device exists and is connected
        let devices = self.devices.read().await;
        if !devices.contains_key(&request.agent_id) {
            return Err(format!("Device not found or offline: {}", request.agent_id));
        }
        drop(devices);

        let session_id = Uuid::new_v4();
        let session = Session {
            id: session_id,
            agent_id: request.agent_id,
            user_id: request.user_id,
            session_type: request.session_type,
            status: SessionStatus::Connecting,
            started_at: Utc::now(),
            ended_at: None,
            ip_address: "0.0.0.0".to_string(), // TODO: Get from request context
            user_agent: "GhostLink Web".to_string(),
        };

        let session_connection = SessionConnection {
            session: session.clone(),
            tx,
            connection_time: Utc::now(),
        };

        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id, session_connection);
        drop(sessions);

        // Add session to device's active sessions
        let mut devices = self.devices.write().await;
        if let Some(device) = devices.get_mut(&request.agent_id) {
            device.active_sessions.push(session_id);
        }

        info!("Session created: {} for device {}", session_id, request.agent_id);
        
        // Broadcast session start
        let _ = self.broadcast_tx.send(BroadcastMessage::SessionStarted(session_id, request.agent_id));

        Ok(session_id)
    }

    /// End a session
    pub async fn end_session(&self, session_id: Uuid) -> Result<(), String> {
        let mut sessions = self.sessions.write().await;
        if let Some(session_conn) = sessions.remove(&session_id) {
            info!("Session ended: {}", session_id);

            // Remove session from device's active sessions
            let mut devices = self.devices.write().await;
            if let Some(device) = devices.get_mut(&session_conn.session.agent_id) {
                device.active_sessions.retain(|&id| id != session_id);
            }

            let _ = self.broadcast_tx.send(BroadcastMessage::SessionEnded(session_id));
            Ok(())
        } else {
            Err(format!("Session not found: {}", session_id))
        }
    }

    /// Get all connected devices
    pub async fn get_connected_devices(&self) -> Vec<Agent> {
        let devices = self.devices.read().await;
        devices.values().map(|conn| conn.agent.clone()).collect()
    }

    /// Get active sessions for a device
    pub async fn get_device_sessions(&self, agent_id: Uuid) -> Vec<Session> {
        let sessions = self.sessions.read().await;
        sessions
            .values()
            .filter(|conn| conn.session.agent_id == agent_id)
            .map(|conn| conn.session.clone())
            .collect()
    }

    /// Send message to a specific device
    pub async fn send_to_device(&self, agent_id: Uuid, message: Message) -> Result<(), String> {
        let devices = self.devices.read().await;
        if let Some(connection) = devices.get(&agent_id) {
            connection.tx.send(message)
                .map_err(|_| "Failed to send message to device".to_string())?;
            Ok(())
        } else {
            Err(format!("Device not connected: {}", agent_id))
        }
    }

    /// Send message to a specific session
    pub async fn send_to_session(&self, session_id: Uuid, message: Message) -> Result<(), String> {
        let sessions = self.sessions.read().await;
        if let Some(connection) = sessions.get(&session_id) {
            connection.tx.send(message)
                .map_err(|_| "Failed to send message to session".to_string())?;
            Ok(())
        } else {
            Err(format!("Session not found: {}", session_id))
        }
    }

    /// Broadcast screen frame to all sessions viewing a device
    pub async fn broadcast_screen_frame(&self, agent_id: Uuid, frame_data: Vec<u8>) {
        let sessions = self.sessions.read().await;
        for connection in sessions.values() {
            if connection.session.agent_id == agent_id && 
               matches!(connection.session.session_type, SessionType::View | SessionType::Control) {
                let message = Message::Binary(frame_data.clone());
                if let Err(e) = connection.tx.send(message) {
                    warn!("Failed to send screen frame to session {}: {}", connection.session.id, e);
                }
            }
        }
        
        let _ = self.broadcast_tx.send(BroadcastMessage::ScreenFrame(agent_id, frame_data));
    }

    /// Forward input event from session to device
    pub async fn forward_input_event(&self, session_id: Uuid, input_data: Vec<u8>) -> Result<(), String> {
        let sessions = self.sessions.read().await;
        if let Some(session_conn) = sessions.get(&session_id) {
            // Only allow input for control sessions
            if !matches!(session_conn.session.session_type, SessionType::Control) {
                return Err("Session does not have control permissions".to_string());
            }

            let agent_id = session_conn.session.agent_id;
            drop(sessions);

            // Send to device
            let devices = self.devices.read().await;
            if let Some(device_conn) = devices.get(&agent_id) {
                let message = Message::Binary(input_data.clone());
                device_conn.tx.send(message)
                    .map_err(|_| "Failed to forward input to device".to_string())?;
                
                let _ = self.broadcast_tx.send(BroadcastMessage::InputEvent(agent_id, input_data));
                Ok(())
            } else {
                Err("Target device not connected".to_string())
            }
        } else {
            Err(format!("Session not found: {}", session_id))
        }
    }

    /// Get device statistics
    pub async fn get_stats(&self) -> DeviceManagerStats {
        let devices = self.devices.read().await;
        let sessions = self.sessions.read().await;

        DeviceManagerStats {
            connected_devices: devices.len(),
            active_sessions: sessions.len(),
            devices_by_platform: devices.values()
                .map(|conn| conn.agent.platform.clone())
                .fold(HashMap::new(), |mut acc, platform| {
                    *acc.entry(platform).or_insert(0) += 1;
                    acc
                }),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct DeviceManagerStats {
    pub connected_devices: usize,
    pub active_sessions: usize,
    pub devices_by_platform: HashMap<String, usize>,
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new()
    }
}