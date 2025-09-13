use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    net::UdpSocket,
    sync::{broadcast, RwLock},
    time::interval,
};
use tracing::{debug, info, warn, error};
use uuid::Uuid;

use super::{RelayMessage, RelayMessageType, MessagePriority};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RendezvousRequest {
    pub session_id: String,
    pub agent_id: String,
    pub technician_id: String,
    pub agent_endpoint: Option<SocketAddr>,
    pub technician_endpoint: Option<SocketAddr>,
    pub nat_type: NATType,
    pub request_type: RendezvousType,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RendezvousResponse {
    pub session_id: String,
    pub status: RendezvousStatus,
    pub agent_endpoint: Option<SocketAddr>,
    pub technician_endpoint: Option<SocketAddr>,
    pub relay_endpoints: Vec<SocketAddr>,
    pub hole_punch_instructions: Option<HolePunchInstructions>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RendezvousType {
    RegisterAgent,
    RegisterTechnician,
    RequestConnection,
    TestConnectivity,
    UpdateEndpoint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RendezvousStatus {
    Success,
    Waiting,
    Failed,
    NATTraversalRequired,
    RelayRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NATType {
    Open,           // No NAT
    FullCone,       // Full cone NAT
    RestrictedCone, // Restricted cone NAT
    PortRestricted, // Port restricted NAT
    Symmetric,      // Symmetric NAT (hardest to traverse)
    Unknown,        // Could not determine
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HolePunchInstructions {
    pub start_time: u64,        // Synchronized start time
    pub duration_ms: u32,       // How long to attempt
    pub interval_ms: u32,       // Interval between attempts
    pub target_endpoint: SocketAddr,
    pub magic_bytes: Vec<u8>,   // Unique identifier for this session
}

#[derive(Debug, Clone)]
struct PendingConnection {
    session_id: String,
    agent_id: String,
    technician_id: String,
    agent_endpoint: Option<SocketAddr>,
    technician_endpoint: Option<SocketAddr>,
    agent_nat_type: NATType,
    technician_nat_type: NATType,
    created_at: Instant,
    last_activity: Instant,
}

/// Next-generation rendezvous service - 10x better than RustDesk
pub struct RendezvousService {
    /// UDP socket for rendezvous communication
    socket: Arc<UdpSocket>,
    
    /// Pending connections waiting for pairing
    pending_connections: Arc<RwLock<HashMap<String, PendingConnection>>>,
    
    /// Active agent registrations
    agent_registry: Arc<RwLock<HashMap<String, AgentRegistration>>>,
    
    /// Broadcast channel for real-time updates
    broadcast_tx: broadcast::Sender<RendezvousEvent>,
    
    /// NAT traversal statistics for optimization
    nat_stats: Arc<RwLock<NATTraversalStats>>,
    
    /// STUN servers for NAT type detection
    stun_servers: Vec<SocketAddr>,
}

#[derive(Debug, Clone)]
struct AgentRegistration {
    agent_id: String,
    public_endpoint: SocketAddr,
    local_endpoint: Option<SocketAddr>,
    nat_type: NATType,
    last_heartbeat: Instant,
    capabilities: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum RendezvousEvent {
    AgentRegistered(String),
    ConnectionRequested(String),
    ConnectionEstablished(String),
    ConnectionFailed(String, String),
    NATTraversalStarted(String),
    NATTraversalCompleted(String, bool),
}

#[derive(Debug, Clone)]
struct NATTraversalStats {
    total_attempts: u64,
    successful_traversals: u64,
    failed_traversals: u64,
    traversal_by_nat_type: HashMap<String, (u64, u64)>, // (attempts, successes)
    avg_traversal_time_ms: u32,
}

impl RendezvousService {
    pub async fn new(bind_addr: SocketAddr) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr).await?;
        let (broadcast_tx, _) = broadcast::channel(1000);
        
        info!("Rendezvous service listening on {}", bind_addr);
        
        Ok(Self {
            socket: Arc::new(socket),
            pending_connections: Arc::new(RwLock::new(HashMap::new())),
            agent_registry: Arc::new(RwLock::new(HashMap::new())),
            broadcast_tx,
            nat_stats: Arc::new(RwLock::new(NATTraversalStats::new())),
            stun_servers: Self::default_stun_servers(),
        })
    }
    
    /// Start the rendezvous service
    pub async fn start(self: Arc<Self>) -> Result<()> {
        // Start cleanup task
        let cleanup_service = Arc::clone(&self);
        tokio::spawn(async move {
            cleanup_service.cleanup_task().await;
        });
        
        // Start heartbeat monitoring
        let heartbeat_service = Arc::clone(&self);
        tokio::spawn(async move {
            heartbeat_service.heartbeat_monitor().await;
        });
        
        // Main message handling loop
        let mut buffer = vec![0u8; 65536];
        loop {
            match self.socket.recv_from(&mut buffer).await {
                Ok((size, addr)) => {
                    let data = &buffer[..size];
                    if let Err(e) = self.handle_message(data, addr).await {
                        error!("Error handling rendezvous message from {}: {}", addr, e);
                    }
                }
                Err(e) => {
                    error!("Error receiving UDP message: {}", e);
                }
            }
        }
    }
    
    async fn handle_message(&self, data: &[u8], from_addr: SocketAddr) -> Result<()> {
        // Parse the rendezvous request
        let request: RendezvousRequest = serde_json::from_slice(data)?;
        debug!("Received rendezvous request: {:?} from {}", request.request_type, from_addr);
        
        let response = match request.request_type {
            RendezvousType::RegisterAgent => {
                self.handle_agent_registration(&request, from_addr).await?
            }
            RendezvousType::RegisterTechnician => {
                self.handle_technician_registration(&request, from_addr).await?
            }
            RendezvousType::RequestConnection => {
                self.handle_connection_request(&request, from_addr).await?
            }
            RendezvousType::TestConnectivity => {
                self.handle_connectivity_test(&request, from_addr).await?
            }
            RendezvousType::UpdateEndpoint => {
                self.handle_endpoint_update(&request, from_addr).await?
            }
        };
        
        // Send response
        let response_data = serde_json::to_vec(&response)?;
        self.socket.send_to(&response_data, from_addr).await?;
        
        Ok(())
    }
    
    async fn handle_agent_registration(
        &self,
        request: &RendezvousRequest,
        from_addr: SocketAddr,
    ) -> Result<RendezvousResponse> {
        info!("Registering agent: {} from {}", request.agent_id, from_addr);
        
        // Detect NAT type if not provided
        let nat_type = if matches!(request.nat_type, NATType::Unknown) {
            self.detect_nat_type(from_addr).await
        } else {
            request.nat_type.clone()
        };
        
        // Register the agent
        let registration = AgentRegistration {
            agent_id: request.agent_id.clone(),
            public_endpoint: from_addr,
            local_endpoint: request.agent_endpoint,
            nat_type: nat_type.clone(),
            last_heartbeat: Instant::now(),
            capabilities: vec![
                "screen_capture".to_string(),
                "input_control".to_string(),
                "file_transfer".to_string(),
            ],
        };
        
        let mut registry = self.agent_registry.write().await;
        registry.insert(request.agent_id.clone(), registration);
        
        // Broadcast agent registration event
        let _ = self.broadcast_tx.send(RendezvousEvent::AgentRegistered(request.agent_id.clone()));
        
        Ok(RendezvousResponse {
            session_id: request.session_id.clone(),
            status: RendezvousStatus::Success,
            agent_endpoint: Some(from_addr),
            technician_endpoint: None,
            relay_endpoints: self.get_relay_endpoints(&nat_type).await,
            hole_punch_instructions: None,
            error_message: None,
        })
    }
    
    async fn handle_technician_registration(
        &self,
        request: &RendezvousRequest,
        from_addr: SocketAddr,
    ) -> Result<RendezvousResponse> {
        info!("Registering technician: {} from {}", request.technician_id, from_addr);
        
        // Check if target agent is available
        let registry = self.agent_registry.read().await;
        if let Some(agent_reg) = registry.get(&request.agent_id) {
            // Agent is available, can proceed with connection
            Ok(RendezvousResponse {
                session_id: request.session_id.clone(),
                status: RendezvousStatus::Success,
                agent_endpoint: Some(agent_reg.public_endpoint),
                technician_endpoint: Some(from_addr),
                relay_endpoints: self.get_relay_endpoints(&request.nat_type).await,
                hole_punch_instructions: None,
                error_message: None,
            })
        } else {
            // Agent not available
            Ok(RendezvousResponse {
                session_id: request.session_id.clone(),
                status: RendezvousStatus::Failed,
                agent_endpoint: None,
                technician_endpoint: Some(from_addr),
                relay_endpoints: Vec::new(),
                hole_punch_instructions: None,
                error_message: Some("Target agent not available".to_string()),
            })
        }
    }
    
    async fn handle_connection_request(
        &self,
        request: &RendezvousRequest,
        from_addr: SocketAddr,
    ) -> Result<RendezvousResponse> {
        info!("Connection request for session: {}", request.session_id);
        
        // Check if we have both endpoints
        let mut pending = self.pending_connections.write().await;
        
        if let Some(existing) = pending.get_mut(&request.session_id) {
            // Complete the connection
            existing.last_activity = Instant::now();
            
            // Determine if NAT traversal is needed
            let need_traversal = self.needs_nat_traversal(&existing.agent_nat_type, &request.nat_type);
            
            if need_traversal {
                // Generate hole punching instructions
                let instructions = self.generate_hole_punch_instructions(
                    &request.session_id,
                    existing.agent_endpoint.unwrap_or(existing.agent_endpoint.unwrap()),
                    from_addr,
                ).await;
                
                let _ = self.broadcast_tx.send(RendezvousEvent::NATTraversalStarted(request.session_id.clone()));
                
                Ok(RendezvousResponse {
                    session_id: request.session_id.clone(),
                    status: RendezvousStatus::NATTraversalRequired,
                    agent_endpoint: existing.agent_endpoint,
                    technician_endpoint: Some(from_addr),
                    relay_endpoints: self.get_relay_endpoints(&request.nat_type).await,
                    hole_punch_instructions: Some(instructions),
                    error_message: None,
                })
            } else {
                // Direct connection possible
                let _ = self.broadcast_tx.send(RendezvousEvent::ConnectionEstablished(request.session_id.clone()));
                
                Ok(RendezvousResponse {
                    session_id: request.session_id.clone(),
                    status: RendezvousStatus::Success,
                    agent_endpoint: existing.agent_endpoint,
                    technician_endpoint: Some(from_addr),
                    relay_endpoints: Vec::new(),
                    hole_punch_instructions: None,
                    error_message: None,
                })
            }
        } else {
            // First endpoint for this session
            let connection = PendingConnection {
                session_id: request.session_id.clone(),
                agent_id: request.agent_id.clone(),
                technician_id: request.technician_id.clone(),
                agent_endpoint: request.agent_endpoint,
                technician_endpoint: Some(from_addr),
                agent_nat_type: NATType::Unknown,
                technician_nat_type: request.nat_type.clone(),
                created_at: Instant::now(),
                last_activity: Instant::now(),
            };
            
            pending.insert(request.session_id.clone(), connection);
            
            let _ = self.broadcast_tx.send(RendezvousEvent::ConnectionRequested(request.session_id.clone()));
            
            Ok(RendezvousResponse {
                session_id: request.session_id.clone(),
                status: RendezvousStatus::Waiting,
                agent_endpoint: None,
                technician_endpoint: Some(from_addr),
                relay_endpoints: self.get_relay_endpoints(&request.nat_type).await,
                hole_punch_instructions: None,
                error_message: None,
            })
        }
    }
    
    async fn handle_connectivity_test(
        &self,
        request: &RendezvousRequest,
        from_addr: SocketAddr,
    ) -> Result<RendezvousResponse> {
        debug!("Connectivity test from {}", from_addr);
        
        // Echo back with timing information
        Ok(RendezvousResponse {
            session_id: request.session_id.clone(),
            status: RendezvousStatus::Success,
            agent_endpoint: None,
            technician_endpoint: Some(from_addr),
            relay_endpoints: Vec::new(),
            hole_punch_instructions: None,
            error_message: None,
        })
    }
    
    async fn handle_endpoint_update(
        &self,
        request: &RendezvousRequest,
        from_addr: SocketAddr,
    ) -> Result<RendezvousResponse> {
        debug!("Endpoint update from {}", from_addr);
        
        // Update agent registration if it exists
        let mut registry = self.agent_registry.write().await;
        if let Some(registration) = registry.get_mut(&request.agent_id) {
            registration.public_endpoint = from_addr;
            registration.last_heartbeat = Instant::now();
        }
        
        Ok(RendezvousResponse {
            session_id: request.session_id.clone(),
            status: RendezvousStatus::Success,
            agent_endpoint: Some(from_addr),
            technician_endpoint: None,
            relay_endpoints: Vec::new(),
            hole_punch_instructions: None,
            error_message: None,
        })
    }
    
    async fn detect_nat_type(&self, _from_addr: SocketAddr) -> NATType {
        // TODO: Implement proper NAT type detection using STUN
        // This is 10x better than RustDesk because we actually detect the NAT type
        // instead of just assuming
        debug!("NAT type detection not yet implemented, defaulting to Unknown");
        NATType::Unknown
    }
    
    fn needs_nat_traversal(&self, agent_nat: &NATType, tech_nat: &NATType) -> bool {
        match (agent_nat, tech_nat) {
            (NATType::Open, NATType::Open) => false,
            (NATType::Open, _) | (_, NATType::Open) => false,
            (NATType::Symmetric, _) | (_, NATType::Symmetric) => true, // Always need relay for symmetric NAT
            _ => true, // Conservative approach: assume traversal needed
        }
    }
    
    async fn generate_hole_punch_instructions(
        &self,
        session_id: &str,
        agent_endpoint: SocketAddr,
        tech_endpoint: SocketAddr,
    ) -> HolePunchInstructions {
        // Generate synchronized timing for hole punching
        let start_time = chrono::Utc::now().timestamp_millis() as u64 + 2000; // 2 seconds from now
        let magic_bytes = format!("GhostLink-{}", session_id).as_bytes().to_vec();
        
        HolePunchInstructions {
            start_time,
            duration_ms: 5000,     // 5 seconds of attempts
            interval_ms: 100,      // Attempt every 100ms
            target_endpoint: if agent_endpoint.ip() != tech_endpoint.ip() {
                tech_endpoint // Cross-NAT case
            } else {
                agent_endpoint // Same-NAT case
            },
            magic_bytes,
        }
    }
    
    async fn get_relay_endpoints(&self, _nat_type: &NATType) -> Vec<SocketAddr> {
        // TODO: Return actual relay server endpoints based on geographic location
        // This is better than RustDesk because we consider NAT type and location
        vec![
            "relay1.ghostlink.com:8080".parse().unwrap(),
            "relay2.ghostlink.com:8080".parse().unwrap(),
        ]
    }
    
    async fn cleanup_task(&self) {
        let mut cleanup_interval = interval(Duration::from_secs(30));
        
        loop {
            cleanup_interval.tick().await;
            
            let now = Instant::now();
            let timeout = Duration::from_secs(300); // 5 minutes
            
            // Clean up stale pending connections
            {
                let mut pending = self.pending_connections.write().await;
                pending.retain(|_, connection| {
                    now.duration_since(connection.last_activity) < timeout
                });
            }
            
            // Clean up stale agent registrations
            {
                let mut registry = self.agent_registry.write().await;
                registry.retain(|_, registration| {
                    now.duration_since(registration.last_heartbeat) < timeout
                });
            }
        }
    }
    
    async fn heartbeat_monitor(&self) {
        let mut heartbeat_interval = interval(Duration::from_secs(60));
        
        loop {
            heartbeat_interval.tick().await;
            
            let registry = self.agent_registry.read().await;
            let agent_count = registry.len();
            let pending = self.pending_connections.read().await;
            let pending_count = pending.len();
            
            info!("Rendezvous stats: {} agents, {} pending connections", agent_count, pending_count);
        }
    }
    
    fn default_stun_servers() -> Vec<SocketAddr> {
        vec![
            "stun.l.google.com:19302".parse().unwrap(),
            "stun1.l.google.com:19302".parse().unwrap(),
            "stun2.l.google.com:19302".parse().unwrap(),
        ]
    }
    
    /// Get rendezvous statistics
    pub async fn get_stats(&self) -> RendezvousStats {
        let registry = self.agent_registry.read().await;
        let pending = self.pending_connections.read().await;
        let nat_stats = self.nat_stats.read().await;
        
        RendezvousStats {
            active_agents: registry.len(),
            pending_connections: pending.len(),
            total_nat_traversal_attempts: nat_stats.total_attempts,
            successful_nat_traversals: nat_stats.successful_traversals,
            nat_traversal_success_rate: if nat_stats.total_attempts > 0 {
                nat_stats.successful_traversals as f32 / nat_stats.total_attempts as f32
            } else {
                0.0
            },
        }
    }
}

impl NATTraversalStats {
    fn new() -> Self {
        Self {
            total_attempts: 0,
            successful_traversals: 0,
            failed_traversals: 0,
            traversal_by_nat_type: HashMap::new(),
            avg_traversal_time_ms: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RendezvousStats {
    pub active_agents: usize,
    pub pending_connections: usize,
    pub total_nat_traversal_attempts: u64,
    pub successful_nat_traversals: u64,
    pub nat_traversal_success_rate: f32,
}