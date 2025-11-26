use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;
use tracing::{debug, info};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PConnectionInfo {
    pub session_id: String,
    pub local_addr: SocketAddr,
    pub public_addr: Option<SocketAddr>,
    pub nat_type: NATType,
    pub connection_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NATType {
    Open,           // No NAT
    FullCone,       // Full cone NAT
    RestrictedCone, // Restricted cone NAT  
    PortRestricted, // Port restricted NAT
    Symmetric,      // Symmetric NAT
    Unknown,        // Could not determine
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub peer_id: String,
    pub local_addr: SocketAddr,
    pub public_addr: Option<SocketAddr>,
    pub nat_type: NATType,
    pub relay_servers: Vec<SocketAddr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionMethod {
    Direct,         // Direct P2P connection
    HolePunching,   // TCP hole punching
    Relay,          // Through relay server
}

pub struct P2PManager {
    session_id: String,
    local_info: P2PConnectionInfo,
    relay_servers: Vec<SocketAddr>,
    rendezvous_server: SocketAddr,
}

impl P2PManager {
    pub async fn new(
        session_id: String,
        relay_servers: Vec<SocketAddr>,
        rendezvous_server: SocketAddr,
    ) -> Result<Self> {
        info!("Initializing P2P manager for session: {}", session_id);
        
        let local_info = Self::discover_local_info(&session_id).await?;
        
        Ok(Self {
            session_id,
            local_info,
            relay_servers,
            rendezvous_server,
        })
    }
    
    async fn discover_local_info(session_id: &str) -> Result<P2PConnectionInfo> {
        // Bind to a random port to discover local address
        let listener = TcpListener::bind("0.0.0.0:0").await?;
        let local_addr = listener.local_addr()?;
        
        info!("Local address discovered: {}", local_addr);
        
        // TODO: Implement STUN-like discovery for public address and NAT type
        let public_addr = Self::discover_public_address().await;
        let nat_type = Self::detect_nat_type(&local_addr, &public_addr).await;
        
        Ok(P2PConnectionInfo {
            session_id: session_id.to_string(),
            local_addr,
            public_addr,
            nat_type,
            connection_id: Uuid::new_v4(),
        })
    }
    
    async fn discover_public_address() -> Option<SocketAddr> {
        // TODO: Implement STUN server query to discover public IP/port
        // For now, return None (behind NAT)
        debug!("Public address discovery not yet implemented");
        None
    }
    
    async fn detect_nat_type(
        _local_addr: &SocketAddr,
        public_addr: &Option<SocketAddr>,
    ) -> NATType {
        match public_addr {
            Some(_) => {
                // TODO: Implement proper NAT type detection using STUN
                debug!("NAT type detection not yet implemented");
                NATType::Unknown
            }
            None => NATType::Unknown,
        }
    }
    
    pub async fn connect_to_peer(&self, peer_info: PeerInfo) -> Result<TcpStream> {
        info!("Attempting to connect to peer: {}", peer_info.peer_id);
        
        // Try connection methods in order of preference
        let methods = self.get_connection_methods(&peer_info);
        
        for method in methods {
            match self.try_connection_method(&peer_info, &method).await {
                Ok(stream) => {
                    info!("Connected to peer using method: {:?}", method);
                    return Ok(stream);
                }
                Err(e) => {
                    debug!("Connection method {:?} failed: {}", method, e);
                    continue;
                }
            }
        }
        
        Err(anyhow::anyhow!("All connection methods failed"))
    }
    
    fn get_connection_methods(&self, peer_info: &PeerInfo) -> Vec<ConnectionMethod> {
        let mut methods = Vec::new();
        
        // Try direct connection first if both have public addresses
        if self.local_info.public_addr.is_some() && peer_info.public_addr.is_some() {
            methods.push(ConnectionMethod::Direct);
        }
        
        // Try hole punching for NAT traversal
        if !matches!(self.local_info.nat_type, NATType::Symmetric) 
            && !matches!(peer_info.nat_type, NATType::Symmetric) {
            methods.push(ConnectionMethod::HolePunching);
        }
        
        // Always fall back to relay
        methods.push(ConnectionMethod::Relay);
        
        methods
    }
    
    async fn try_connection_method(
        &self,
        peer_info: &PeerInfo,
        method: &ConnectionMethod,
    ) -> Result<TcpStream> {
        match method {
            ConnectionMethod::Direct => {
                self.try_direct_connection(peer_info).await
            }
            ConnectionMethod::HolePunching => {
                self.try_hole_punching(peer_info).await
            }
            ConnectionMethod::Relay => {
                self.try_relay_connection(peer_info).await
            }
        }
    }
    
    async fn try_direct_connection(&self, peer_info: &PeerInfo) -> Result<TcpStream> {
        if let Some(public_addr) = peer_info.public_addr {
            debug!("Attempting direct connection to: {}", public_addr);
            
            let stream = timeout(
                Duration::from_secs(5),
                TcpStream::connect(public_addr)
            ).await??;
            
            info!("Direct connection established");
            return Ok(stream);
        }
        
        Err(anyhow::anyhow!("No public address available for direct connection"))
    }
    
    async fn try_hole_punching(&self, peer_info: &PeerInfo) -> Result<TcpStream> {
        info!("Attempting TCP hole punching");
        
        // Step 1: Register with rendezvous server
        self.register_with_rendezvous().await?;
        
        // Step 2: Coordinate hole punching through rendezvous server
        let punch_info = self.coordinate_hole_punch(&peer_info.peer_id).await?;
        
        // Step 3: Simultaneous connection attempts
        let stream = self.perform_hole_punch(&punch_info).await?;
        
        info!("Hole punching successful");
        Ok(stream)
    }
    
    async fn register_with_rendezvous(&self) -> Result<()> {
        debug!("Registering with rendezvous server: {}", self.rendezvous_server);
        
        // TODO: Implement rendezvous server registration
        // Send our local_info to the rendezvous server
        
        Ok(())
    }
    
    async fn coordinate_hole_punch(&self, peer_id: &str) -> Result<HolePunchInfo> {
        debug!("Coordinating hole punch with peer: {}", peer_id);
        
        // TODO: Implement rendezvous server coordination
        // Exchange connection info and timing with peer
        
        Ok(HolePunchInfo {
            target_addr: "127.0.0.1:12345".parse()?,
            start_time: std::time::Instant::now() + Duration::from_secs(1),
        })
    }
    
    async fn perform_hole_punch(&self, punch_info: &HolePunchInfo) -> Result<TcpStream> {
        debug!("Performing hole punch to: {}", punch_info.target_addr);
        
        // Wait for coordinated start time
        let now = std::time::Instant::now();
        if punch_info.start_time > now {
            tokio::time::sleep(punch_info.start_time - now).await;
        }
        
        // Simultaneous connection attempts
        let mut attempts = 0;
        while attempts < 10 {
            match TcpStream::connect(punch_info.target_addr).await {
                Ok(stream) => return Ok(stream),
                Err(_) => {
                    attempts += 1;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
        
        Err(anyhow::anyhow!("Hole punching attempts exhausted"))
    }
    
    async fn try_relay_connection(&self, peer_info: &PeerInfo) -> Result<TcpStream> {
        info!("Attempting relay connection");
        
        for relay_addr in &self.relay_servers {
            match self.connect_through_relay(relay_addr, &peer_info.peer_id).await {
                Ok(stream) => {
                    info!("Relay connection established through: {}", relay_addr);
                    return Ok(stream);
                }
                Err(e) => {
                    debug!("Relay {} failed: {}", relay_addr, e);
                    continue;
                }
            }
        }
        
        Err(anyhow::anyhow!("All relay servers failed"))
    }
    
    async fn connect_through_relay(
        &self,
        relay_addr: &SocketAddr,
        _peer_id: &str,
    ) -> Result<TcpStream> {
        debug!("Connecting through relay: {}", relay_addr);
        
        let stream = TcpStream::connect(relay_addr).await?;
        
        // TODO: Implement relay protocol
        // Send relay request with peer_id
        // Handle relay response
        
        Ok(stream)
    }
    
    pub async fn wait_for_peer_connection(&self) -> Result<TcpStream> {
        info!("Waiting for incoming peer connection");
        
        let listener = TcpListener::bind(self.local_info.local_addr).await?;
        
        // Register ourselves as available for connections
        self.register_with_rendezvous().await?;
        
        // Wait for incoming connection
        let (stream, addr) = listener.accept().await?;
        info!("Incoming connection from: {}", addr);
        
        // TODO: Verify connection is from expected peer
        
        Ok(stream)
    }
    
    pub fn get_local_info(&self) -> &P2PConnectionInfo {
        &self.local_info
    }
    
    pub fn is_p2p_capable(&self) -> bool {
        !matches!(self.local_info.nat_type, NATType::Symmetric)
    }
}

#[derive(Debug)]
struct HolePunchInfo {
    target_addr: SocketAddr,
    start_time: std::time::Instant,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_p2p_manager_creation() {
        let relay_servers = vec!["127.0.0.1:8080".parse().unwrap()];
        let rendezvous_server = "127.0.0.1:8081".parse().unwrap();
        
        let manager = P2PManager::new(
            "test_session".to_string(),
            relay_servers,
            rendezvous_server,
        ).await;
        
        assert!(manager.is_ok());
    }
    
    #[test]
    fn test_nat_type_serialization() {
        let nat_type = NATType::FullCone;
        let serialized = serde_json::to_string(&nat_type).unwrap();
        let deserialized: NATType = serde_json::from_str(&serialized).unwrap();
        
        assert!(matches!(deserialized, NATType::FullCone));
    }
}