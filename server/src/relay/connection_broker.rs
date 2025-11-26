//! Connection broker for managing P2P and relay connections
//!
//! This module handles the brokering of connections between agents and technicians,
//! deciding whether to use P2P or relay based on network conditions.

#![allow(dead_code)]

use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use uuid::Uuid;
use tracing::{debug, info};

/// Connection broker for managing session connections
pub struct ConnectionBroker {
    /// Active connection requests
    pending_requests: Arc<RwLock<HashMap<Uuid, ConnectionRequest>>>,
    /// Established connections
    active_connections: Arc<RwLock<HashMap<Uuid, ActiveConnection>>>,
}

#[derive(Debug, Clone)]
pub struct ConnectionRequest {
    pub session_id: Uuid,
    pub agent_id: String,
    pub technician_id: String,
    pub preferred_type: ConnectionPreference,
}

#[derive(Debug, Clone)]
pub enum ConnectionPreference {
    P2PPreferred,
    RelayPreferred,
    P2POnly,
    RelayOnly,
}

#[derive(Debug)]
pub struct ActiveConnection {
    pub session_id: Uuid,
    pub connection_type: EstablishedConnectionType,
}

#[derive(Debug, Clone)]
pub enum EstablishedConnectionType {
    DirectP2P { local_addr: String, remote_addr: String },
    Relayed { relay_node: String },
    Hybrid { p2p_addr: Option<String>, relay_node: String },
}

impl ConnectionBroker {
    pub fn new() -> Self {
        info!("Initializing connection broker");
        Self {
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            active_connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Request a new connection
    pub async fn request_connection(&self, request: ConnectionRequest) -> anyhow::Result<Uuid> {
        let request_id = Uuid::new_v4();
        debug!("New connection request: {:?}", request);

        let mut pending = self.pending_requests.write().await;
        pending.insert(request_id, request);

        Ok(request_id)
    }

    /// Get connection status
    pub async fn get_connection_status(&self, session_id: Uuid) -> Option<EstablishedConnectionType> {
        let connections = self.active_connections.read().await;
        connections.get(&session_id).map(|c| c.connection_type.clone())
    }

    /// Close a connection
    pub async fn close_connection(&self, session_id: Uuid) {
        let mut connections = self.active_connections.write().await;
        connections.remove(&session_id);
        info!("Closed connection for session: {}", session_id);
    }
}

impl Default for ConnectionBroker {
    fn default() -> Self {
        Self::new()
    }
}
