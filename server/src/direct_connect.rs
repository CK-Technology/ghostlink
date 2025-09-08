use axum::{
    extract::{Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;
use tracing::{info, warn, error, debug};

use crate::{AppState, device_manager::DeviceManager};

/// Direct connection manager for RustDesk-style peer-to-peer connections
pub struct DirectConnectManager {
    /// Map of client IDs to their connection info
    clients: Arc<RwLock<HashMap<String, ClientInfo>>>,
    /// Map of active direct connection sessions
    sessions: Arc<RwLock<HashMap<Uuid, DirectSession>>>,
    /// Broadcast channel for relay messages
    relay_tx: mpsc::UnboundedSender<RelayMessage>,
}

#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub id: String,
    pub password: String,
    pub local_ip: IpAddr,
    pub external_ip: IpAddr,
    pub port: u16,
    pub last_seen: chrono::DateTime<chrono::Utc>,
    pub nat_type: NatType,
    pub relay_server: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DirectSession {
    pub session_id: Uuid,
    pub client_a: String,
    pub client_b: String,
    pub connection_type: ConnectionType,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub status: SessionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NatType {
    None,          // Direct connection possible
    FullCone,      // Port prediction works
    Restricted,    // Need hole punching
    PortRestricted, // More complex hole punching
    Symmetric,     // Relay required
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionType {
    Direct,        // Peer-to-peer connection
    Relayed,       // Through relay server
    HolePunched,   // NAT traversal successful
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionStatus {
    Connecting,
    Connected,
    Failed,
    Disconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayMessage {
    pub from: String,
    pub to: String,
    pub message_type: MessageType,
    pub data: Vec<u8>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    ConnectionRequest,
    ConnectionResponse,
    HolePunchRequest,
    HolePunchResponse,
    RelayData,
    Heartbeat,
    Error,
}

#[derive(Debug, Deserialize)]
pub struct ConnectRequest {
    pub target_id: String,
    pub password: String,
    pub connection_mode: ConnectionMode,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum ConnectionMode {
    Auto,      // Try direct first, fall back to relay
    DirectOnly, // Direct connection only
    RelayOnly,  // Force relay connection
}

#[derive(Debug, Serialize)]
pub struct ConnectResponse {
    pub session_id: Uuid,
    pub connection_info: ConnectionInfo,
    pub relay_servers: Vec<RelayServerInfo>,
}

#[derive(Debug, Serialize)]
pub struct ConnectionInfo {
    pub direct_endpoints: Vec<String>,
    pub relay_endpoint: Option<String>,
    pub nat_traversal_info: Option<NatTraversalInfo>,
}

#[derive(Debug, Serialize)]
pub struct NatTraversalInfo {
    pub external_ip: IpAddr,
    pub external_port: u16,
    pub nat_type: NatType,
    pub hole_punch_ports: Vec<u16>,
}

#[derive(Debug, Serialize)]
pub struct RelayServerInfo {
    pub address: String,
    pub port: u16,
    pub region: String,
    pub latency_ms: Option<u32>,
}

impl DirectConnectManager {
    pub fn new() -> Self {
        let (relay_tx, _) = mpsc::unbounded_channel();
        
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            relay_tx,
        }
    }
    
    /// Register a client for direct connections
    pub async fn register_client(&self, client_info: ClientInfo) -> Result<(), String> {
        let mut clients = self.clients.write().await;
        
        info!("Registering client {} for direct connections", client_info.id);
        clients.insert(client_info.id.clone(), client_info);
        
        Ok(())
    }
    
    /// Initiate direct connection between two clients
    pub async fn initiate_connection(
        &self,
        requester_id: &str,
        request: ConnectRequest,
    ) -> Result<ConnectResponse, String> {
        let clients = self.clients.read().await;
        
        // Get target client info
        let target_client = clients.get(&request.target_id)
            .ok_or_else(|| format!("Target client {} not found", request.target_id))?;
        
        // Verify password
        if target_client.password != request.password {
            return Err("Invalid password".to_string());
        }
        
        // Get requester client info
        let requester_client = clients.get(requester_id)
            .ok_or_else(|| format!("Requester client {} not found", requester_id))?;
        
        // Create session
        let session_id = Uuid::new_v4();
        let session = DirectSession {
            session_id,
            client_a: requester_id.to_string(),
            client_b: request.target_id.clone(),
            connection_type: self.determine_connection_type(&requester_client, &target_client).await,
            created_at: chrono::Utc::now(),
            status: SessionStatus::Connecting,
        };
        
        // Store session
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id, session);
        }
        
        // Generate connection info
        let connection_info = self.generate_connection_info(&requester_client, &target_client).await;
        let relay_servers = self.get_available_relay_servers().await;
        
        info!("Created direct connection session {} between {} and {}", 
              session_id, requester_id, request.target_id);
        
        Ok(ConnectResponse {
            session_id,
            connection_info,
            relay_servers,
        })
    }
    
    /// Determine the best connection type based on client NAT types
    async fn determine_connection_type(&self, client_a: &ClientInfo, client_b: &ClientInfo) -> ConnectionType {
        match (&client_a.nat_type, &client_b.nat_type) {
            (NatType::None, NatType::None) => ConnectionType::Direct,
            (NatType::None, _) | (_, NatType::None) => ConnectionType::Direct,
            (NatType::FullCone, NatType::FullCone) => ConnectionType::HolePunched,
            (NatType::Restricted, NatType::Restricted) => ConnectionType::HolePunched,
            (NatType::Symmetric, _) | (_, NatType::Symmetric) => ConnectionType::Relayed,
            _ => ConnectionType::HolePunched,
        }
    }
    
    /// Generate connection information for clients
    async fn generate_connection_info(&self, client_a: &ClientInfo, client_b: &ClientInfo) -> ConnectionInfo {
        let mut direct_endpoints = Vec::new();
        
        // Add direct IP endpoints if possible
        if !matches!(client_b.nat_type, NatType::Symmetric) {
            direct_endpoints.push(format!("{}:{}", client_b.external_ip, client_b.port));
            if client_b.local_ip != client_b.external_ip {
                direct_endpoints.push(format!("{}:{}", client_b.local_ip, client_b.port));
            }
        }
        
        // NAT traversal info for hole punching
        let nat_traversal_info = if matches!(client_b.nat_type, NatType::FullCone | NatType::Restricted | NatType::PortRestricted) {
            Some(NatTraversalInfo {
                external_ip: client_b.external_ip,
                external_port: client_b.port,
                nat_type: client_b.nat_type.clone(),
                hole_punch_ports: self.generate_hole_punch_ports(client_b.port),
            })
        } else {
            None
        };
        
        // Relay endpoint
        let relay_endpoint = client_b.relay_server.clone()
            .or_else(|| Some("relay.ghostlink.com:21118".to_string()));
        
        ConnectionInfo {
            direct_endpoints,
            relay_endpoint,
            nat_traversal_info,
        }
    }
    
    /// Generate hole punch port candidates
    fn generate_hole_punch_ports(&self, base_port: u16) -> Vec<u16> {
        let mut ports = Vec::new();
        
        // Add base port and nearby ports for port prediction
        for offset in 0..10 {
            if let Some(port) = base_port.checked_add(offset) {
                ports.push(port);
            }
            if offset > 0 {
                if let Some(port) = base_port.checked_sub(offset) {
                    ports.push(port);
                }
            }
        }
        
        ports
    }
    
    /// Get available relay servers
    async fn get_available_relay_servers(&self) -> Vec<RelayServerInfo> {
        vec![
            RelayServerInfo {
                address: "relay1.ghostlink.com".to_string(),
                port: 21118,
                region: "us-east-1".to_string(),
                latency_ms: Some(25),
            },
            RelayServerInfo {
                address: "relay2.ghostlink.com".to_string(),
                port: 21118,
                region: "us-west-1".to_string(),
                latency_ms: Some(45),
            },
            RelayServerInfo {
                address: "relay3.ghostlink.com".to_string(),
                port: 21118,
                region: "eu-west-1".to_string(),
                latency_ms: Some(85),
            },
        ]
    }
    
    /// Handle relay messages between clients
    pub async fn handle_relay_message(&self, message: RelayMessage) -> Result<(), String> {
        let clients = self.clients.read().await;
        
        // Verify sender and recipient exist
        if !clients.contains_key(&message.from) {
            return Err(format!("Unknown sender: {}", message.from));
        }
        
        if !clients.contains_key(&message.to) {
            return Err(format!("Unknown recipient: {}", message.to));
        }
        
        // Forward message (in a real implementation, this would go through WebSocket or UDP)
        debug!("Relaying {} message from {} to {}", 
               serde_json::to_string(&message.message_type).unwrap_or_default(),
               message.from, 
               message.to);
        
        // TODO: Actually forward the message to the recipient
        
        Ok(())
    }
    
    /// Get connection statistics
    pub async fn get_connection_stats(&self) -> HashMap<String, serde_json::Value> {
        let clients = self.clients.read().await;
        let sessions = self.sessions.read().await;
        
        let mut stats = HashMap::new();
        stats.insert("total_clients".to_string(), serde_json::Value::Number(clients.len().into()));
        stats.insert("active_sessions".to_string(), serde_json::Value::Number(sessions.len().into()));
        
        let connection_types: HashMap<String, usize> = sessions.values()
            .fold(HashMap::new(), |mut acc, session| {
                let key = format!("{:?}", session.connection_type);
                *acc.entry(key).or_insert(0) += 1;
                acc
            });
        
        stats.insert("connection_types".to_string(), serde_json::to_value(connection_types).unwrap());
        
        stats
    }
}

/// API handlers for direct connections

/// Register client for direct connections
pub async fn api_register_direct_client(
    State(app_state): State<AppState>,
    Json(client_info): Json<ClientInfo>,
) -> Response {
    match app_state.device_manager.direct_connect_manager.register_client(client_info).await {
        Ok(_) => Json(serde_json::json!({
            "status": "success",
            "message": "Client registered for direct connections"
        })).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e
            }))
        ).into_response(),
    }
}

/// Initiate direct connection
pub async fn api_connect_direct(
    State(app_state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
    Json(request): Json<ConnectRequest>,
) -> Response {
    let requester_id = match params.get("client_id") {
        Some(id) => id,
        None => return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Missing client_id parameter"
            }))
        ).into_response(),
    };
    
    match app_state.device_manager.direct_connect_manager.initiate_connection(requester_id, request).await {
        Ok(response) => Json(response).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e
            }))
        ).into_response(),
    }
}

/// Get direct connection statistics
pub async fn api_direct_connect_stats(
    State(app_state): State<AppState>,
) -> impl IntoResponse {
    let stats = app_state.device_manager.direct_connect_manager.get_connection_stats().await;
    Json(stats)
}

/// Handle relay messages via WebSocket
pub async fn websocket_direct_relay_handler(
    ws: WebSocketUpgrade,
    State(app_state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let client_id = params.get("client_id").cloned();
    
    ws.on_upgrade(move |socket| async move {
        if let Some(client_id) = client_id {
            handle_direct_relay_websocket(socket, client_id, app_state.device_manager).await;
        }
    })
}