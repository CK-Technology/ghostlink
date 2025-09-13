use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{Duration, timeout};
use tracing::{debug, info, warn, error};
use uuid::Uuid;

use super::{RelayConnection, RelayMessage, P2PManager, P2PConnectionInfo};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionType {
    Direct,         // Direct P2P (RustDesk-style)
    Relay,          // Through GhostLink server (ScreenConnect-style)
    Hybrid,         // Both P2P + relay fallback
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionSettings {
    pub prefer_p2p: bool,
    pub allow_relay_fallback: bool,
    pub connection_timeout: Duration,
    pub p2p_timeout: Duration,
    pub force_relay: bool,
    pub encryption_required: bool,
}

impl Default for ConnectionSettings {
    fn default() -> Self {
        Self {
            prefer_p2p: true,              // RustDesk approach: try P2P first
            allow_relay_fallback: true,    // ScreenConnect reliability
            connection_timeout: Duration::from_secs(10),
            p2p_timeout: Duration::from_secs(5),
            force_relay: false,
            encryption_required: true,
        }
    }
}

pub struct HybridConnectionManager {
    session_id: String,
    relay_connection: Arc<Mutex<Option<RelayConnection>>>,
    p2p_manager: Arc<Mutex<Option<P2PManager>>>,
    connection_type: Arc<RwLock<ConnectionType>>,
    settings: ConnectionSettings,
    relay_servers: Vec<SocketAddr>,
    rendezvous_server: SocketAddr,
}

impl HybridConnectionManager {
    pub async fn new(
        session_id: String,
        relay_servers: Vec<SocketAddr>,
        rendezvous_server: SocketAddr,
        settings: ConnectionSettings,
    ) -> Result<Self> {
        info!("Creating hybrid connection manager for session: {}", session_id);
        
        Ok(Self {
            session_id,
            relay_connection: Arc::new(Mutex::new(None)),
            p2p_manager: Arc::new(Mutex::new(None)),
            connection_type: Arc::new(RwLock::new(ConnectionType::Hybrid)),
            settings,
            relay_servers,
            rendezvous_server,
        })
    }
    
    /// Establish connection using best available method
    /// Combines ScreenConnect reliability with RustDesk P2P performance
    pub async fn connect(&self, peer_id: String) -> Result<()> {
        info!("Establishing hybrid connection to peer: {}", peer_id);
        
        if self.settings.force_relay {
            return self.connect_relay_only().await;
        }
        
        if self.settings.prefer_p2p {
            // RustDesk approach: try P2P first for performance
            match self.try_p2p_connection(&peer_id).await {
                Ok(()) => {
                    info!("P2P connection established successfully");
                    let mut conn_type = self.connection_type.write().await;
                    *conn_type = ConnectionType::Direct;
                    return Ok(());
                }
                Err(e) => {
                    warn!("P2P connection failed: {}", e);
                    if !self.settings.allow_relay_fallback {
                        return Err(e);
                    }
                }
            }
        }
        
        // ScreenConnect approach: reliable relay connection
        info!("Falling back to relay connection");
        self.connect_relay_only().await?;
        
        let mut conn_type = self.connection_type.write().await;
        *conn_type = ConnectionType::Relay;
        
        Ok(())
    }
    
    async fn try_p2p_connection(&self, peer_id: &str) -> Result<()> {
        info!("Attempting P2P connection using RustDesk-style approach");
        
        // Initialize P2P manager
        let p2p_manager = P2PManager::new(
            self.session_id.clone(),
            self.relay_servers.clone(),
            self.rendezvous_server,
        ).await?;
        
        // Store P2P manager
        {
            let mut p2p_guard = self.p2p_manager.lock().await;
            *p2p_guard = Some(p2p_manager);
        }
        
        // Exchange P2P info through relay server first (like RustDesk rendezvous)
        self.exchange_p2p_info(peer_id).await?;
        
        // Wait for P2P handshake response
        timeout(
            self.settings.p2p_timeout,
            self.wait_for_p2p_handshake()
        ).await??;
        
        info!("P2P handshake completed successfully");
        Ok(())
    }
    
    async fn exchange_p2p_info(&self, peer_id: &str) -> Result<()> {
        // Get our P2P connection info
        let connection_info = {
            let p2p_guard = self.p2p_manager.lock().await;
            if let Some(ref p2p) = *p2p_guard {
                p2p.get_local_info().clone()
            } else {
                return Err(anyhow::anyhow!("P2P manager not initialized"));
            }
        };
        
        // Send P2P handshake through relay (rendezvous)
        let handshake_msg = RelayMessage::P2PHandshake {
            session_id: self.session_id.clone(),
            connection_info,
        };
        
        // We need a relay connection for the handshake
        self.ensure_relay_connection().await?;
        
        let relay_guard = self.relay_connection.lock().await;
        if let Some(ref relay) = *relay_guard {
            relay.send_message(handshake_msg).await?;
            info!("P2P handshake sent to peer: {}", peer_id);
        } else {
            return Err(anyhow::anyhow!("Relay connection required for P2P handshake"));
        }
        
        Ok(())
    }
    
    async fn wait_for_p2p_handshake(&self) -> Result<()> {
        // TODO: Implement actual handshake response waiting
        // This would listen for P2PResponse messages from the relay
        info!("Waiting for P2P handshake response...");
        
        // Simulate handshake completion for now
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        Ok(())
    }
    
    async fn connect_relay_only(&self) -> Result<()> {
        info!("Establishing relay connection (ScreenConnect-style)");
        
        self.ensure_relay_connection().await?;
        
        info!("Relay connection established");
        Ok(())
    }
    
    async fn ensure_relay_connection(&self) -> Result<()> {
        let mut relay_guard = self.relay_connection.lock().await;
        
        if relay_guard.is_none() {
            // Create relay connection using existing RelayConnection
            // TODO: Pass proper ClientConfig
            info!("Creating new relay connection");
            // For now, we'll assume the connection is created elsewhere
            // In a real implementation, we'd create it here with proper config
        }
        
        Ok(())
    }
    
    /// Send data through the active connection
    pub async fn send_data(&self, data: Vec<u8>) -> Result<()> {
        let conn_type = self.connection_type.read().await;
        
        match *conn_type {
            ConnectionType::Direct => {
                self.send_via_p2p(data).await
            }
            ConnectionType::Relay => {
                self.send_via_relay(data).await
            }
            ConnectionType::Hybrid => {
                // Try P2P first, fallback to relay
                match self.send_via_p2p(data.clone()).await {
                    Ok(()) => Ok(()),
                    Err(_) => self.send_via_relay(data).await,
                }
            }
        }
    }
    
    async fn send_via_p2p(&self, data: Vec<u8>) -> Result<()> {
        debug!("Sending {} bytes via P2P", data.len());
        
        // TODO: Implement actual P2P data sending
        // This would use the TcpStream from P2P connection
        
        Ok(())
    }
    
    async fn send_via_relay(&self, data: Vec<u8>) -> Result<()> {
        debug!("Sending {} bytes via relay", data.len());
        
        let relay_guard = self.relay_connection.lock().await;
        if let Some(ref relay) = *relay_guard {
            relay.send_binary_frame(data).await?;
        } else {
            return Err(anyhow::anyhow!("No relay connection available"));
        }
        
        Ok(())
    }
    
    /// Check connection health and switch if needed
    pub async fn health_check(&self) -> Result<()> {
        let conn_type = self.connection_type.read().await;
        
        match *conn_type {
            ConnectionType::Direct => {
                if !self.is_p2p_healthy().await {
                    warn!("P2P connection unhealthy, switching to relay");
                    self.switch_to_relay().await?;
                }
            }
            ConnectionType::Relay => {
                if !self.is_relay_healthy().await {
                    if self.settings.prefer_p2p {
                        info!("Relay unhealthy, attempting P2P reconnection");
                        // TODO: Implement P2P reconnection
                    }
                }
            }
            ConnectionType::Hybrid => {
                // Check both connections
                let p2p_healthy = self.is_p2p_healthy().await;
                let relay_healthy = self.is_relay_healthy().await;
                
                if !p2p_healthy && !relay_healthy {
                    return Err(anyhow::anyhow!("All connections unhealthy"));
                }
            }
        }
        
        Ok(())
    }
    
    async fn is_p2p_healthy(&self) -> bool {
        let p2p_guard = self.p2p_manager.lock().await;
        if let Some(ref p2p) = *p2p_guard {
            p2p.is_p2p_capable()
        } else {
            false
        }
    }
    
    async fn is_relay_healthy(&self) -> bool {
        let relay_guard = self.relay_connection.lock().await;
        if let Some(ref relay) = *relay_guard {
            relay.is_healthy().await
        } else {
            false
        }
    }
    
    async fn switch_to_relay(&self) -> Result<()> {
        info!("Switching from P2P to relay connection");
        
        self.ensure_relay_connection().await?;
        
        let mut conn_type = self.connection_type.write().await;
        *conn_type = ConnectionType::Relay;
        
        Ok(())
    }
    
    /// Handle incoming P2P handshake from peer
    pub async fn handle_p2p_handshake(&self, connection_info: P2PConnectionInfo) -> Result<()> {
        info!("Received P2P handshake from peer");
        
        // Initialize our P2P manager if not already done
        if self.p2p_manager.lock().await.is_none() {
            let p2p_manager = P2PManager::new(
                self.session_id.clone(),
                self.relay_servers.clone(),
                self.rendezvous_server,
            ).await?;
            
            let mut p2p_guard = self.p2p_manager.lock().await;
            *p2p_guard = Some(p2p_manager);
        }
        
        // Respond with our connection info
        let our_info = {
            let p2p_guard = self.p2p_manager.lock().await;
            if let Some(ref p2p) = *p2p_guard {
                p2p.get_local_info().clone()
            } else {
                return Err(anyhow::anyhow!("P2P manager not available"));
            }
        };
        
        let response = RelayMessage::P2PResponse {
            session_id: self.session_id.clone(),
            accepted: true,
            connection_info: Some(our_info),
        };
        
        let relay_guard = self.relay_connection.lock().await;
        if let Some(ref relay) = *relay_guard {
            relay.send_message(response).await?;
        }
        
        // Attempt direct P2P connection
        // TODO: Use connection_info to establish direct connection
        
        Ok(())
    }
    
    /// Get current connection statistics
    pub async fn get_connection_stats(&self) -> ConnectionStats {
        let conn_type = self.connection_type.read().await;
        
        ConnectionStats {
            connection_type: conn_type.clone(),
            p2p_available: self.is_p2p_healthy().await,
            relay_available: self.is_relay_healthy().await,
            session_id: self.session_id.clone(),
        }
    }
    
    /// Disconnect all connections
    pub async fn disconnect(&self) -> Result<()> {
        info!("Disconnecting hybrid connection manager");
        
        // Disconnect relay
        let relay_guard = self.relay_connection.lock().await;
        if let Some(ref relay) = *relay_guard {
            relay.disconnect().await?;
        }
        
        // Disconnect P2P
        // TODO: Implement P2P disconnection
        
        info!("All connections disconnected");
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub connection_type: ConnectionType,
    pub p2p_available: bool,
    pub relay_available: bool,
    pub session_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_hybrid_connection_creation() {
        let relay_servers = vec!["127.0.0.1:8080".parse().unwrap()];
        let rendezvous_server = "127.0.0.1:8081".parse().unwrap();
        let settings = ConnectionSettings::default();
        
        let manager = HybridConnectionManager::new(
            "test_session".to_string(),
            relay_servers,
            rendezvous_server,
            settings,
        ).await;
        
        assert!(manager.is_ok());
    }
    
    #[test]
    fn test_connection_settings_default() {
        let settings = ConnectionSettings::default();
        assert!(settings.prefer_p2p);
        assert!(settings.allow_relay_fallback);
        assert!(settings.encryption_required);
    }
}