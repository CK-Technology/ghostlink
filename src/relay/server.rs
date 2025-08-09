use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;
use axum::extract::ws::{WebSocket, Message};
use futures_util::{SinkExt, StreamExt};
use anyhow::Result;

use crate::relay::messages::{RelayMessage, AgentInfo, AgentStatus};
use crate::models::{Session, SessionStatus};

pub struct RelayServer {
    /// Connected agents indexed by agent ID
    agents: Arc<RwLock<HashMap<Uuid, AgentConnection>>>,
    /// Active sessions indexed by session ID
    sessions: Arc<RwLock<HashMap<Uuid, SessionRelay>>>,
    /// Broadcast channel for server-wide messages
    broadcast_tx: broadcast::Sender<ServerEvent>,
    /// Maximum number of concurrent connections
    max_connections: usize,
}

pub struct AgentConnection {
    pub agent_id: Uuid,
    pub info: AgentInfo,
    pub status: AgentStatus,
    pub sender: tokio::sync::mpsc::UnboundedSender<RelayMessage>,
    pub last_heartbeat: std::time::Instant,
    pub authenticated: bool,
    pub user_id: Option<Uuid>,
}

pub struct SessionRelay {
    pub session: Session,
    pub agent_sender: Option<tokio::sync::mpsc::UnboundedSender<RelayMessage>>,
    pub client_sender: Option<tokio::sync::mpsc::UnboundedSender<RelayMessage>>,
    pub encryption_key: [u8; 32],
}

#[derive(Debug, Clone)]
pub enum ServerEvent {
    AgentConnected(Uuid),
    AgentDisconnected(Uuid),
    SessionStarted(Uuid),
    SessionEnded(Uuid),
    BroadcastMessage(RelayMessage),
}

impl RelayServer {
    pub fn new(max_connections: usize) -> Self {
        let (broadcast_tx, _) = broadcast::channel(1000);
        
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            broadcast_tx,
            max_connections,
        }
    }

    /// Handle a new WebSocket connection
    pub async fn handle_connection(
        &self,
        socket: WebSocket,
        client_ip: String,
    ) -> Result<()> {
        let (mut sender, mut receiver) = socket.split();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<RelayMessage>();
        
        // Handle outgoing messages
        let sender_task = tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                if let Ok(json) = serde_json::to_string(&message) {
                    if sender.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
            }
        });

        // Handle incoming messages
        let agents = Arc::clone(&self.agents);
        let sessions = Arc::clone(&self.sessions);
        let broadcast_tx = self.broadcast_tx.clone();
        
        let receiver_task = tokio::spawn(async move {
            let mut connection_id: Option<Uuid> = None;
            
            while let Some(msg) = receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Ok(relay_msg) = serde_json::from_str::<RelayMessage>(&text) {
                            match Self::handle_message(
                                relay_msg,
                                &tx,
                                &agents,
                                &sessions,
                                &broadcast_tx,
                                &mut connection_id,
                                &client_ip,
                            ).await {
                                Ok(_) => {},
                                Err(e) => {
                                    tracing::error!("Error handling message: {}", e);
                                    let error_msg = RelayMessage::Error {
                                        code: 500,
                                        message: "Internal server error".to_string(),
                                    };
                                    let _ = tx.send(error_msg);
                                }
                            }
                        }
                    },
                    Ok(Message::Close(_)) => break,
                    Err(e) => {
                        tracing::error!("WebSocket error: {}", e);
                        break;
                    },
                    _ => {},
                }
            }

            // Cleanup on disconnect
            if let Some(id) = connection_id {
                agents.write().await.remove(&id);
                let _ = broadcast_tx.send(ServerEvent::AgentDisconnected(id));
                tracing::info!("Agent {} disconnected", id);
            }
        });

        // Wait for either task to complete
        tokio::select! {
            _ = sender_task => {},
            _ = receiver_task => {},
        }

        Ok(())
    }

    async fn handle_message(
        message: RelayMessage,
        sender: &tokio::sync::mpsc::UnboundedSender<RelayMessage>,
        agents: &Arc<RwLock<HashMap<Uuid, AgentConnection>>>,
        sessions: &Arc<RwLock<HashMap<Uuid, SessionRelay>>>,
        broadcast_tx: &broadcast::Sender<ServerEvent>,
        connection_id: &mut Option<Uuid>,
        client_ip: &str,
    ) -> Result<()> {
        match message {
            RelayMessage::Authenticate { token } => {
                // TODO: Validate JWT token
                let response = RelayMessage::AuthResult {
                    success: true,
                    message: "Authentication successful".to_string(),
                    user_id: Some(Uuid::new_v4()), // TODO: Get from token
                };
                sender.send(response)?;
            },

            RelayMessage::AgentRegister { agent_info } => {
                let agent_id = agent_info.id;
                *connection_id = Some(agent_id);

                let agent_connection = AgentConnection {
                    agent_id,
                    info: agent_info.clone(),
                    status: AgentStatus {
                        cpu_usage: 0.0,
                        memory_usage: 0.0,
                        disk_usage: 0.0,
                        network_rx_bytes: 0,
                        network_tx_bytes: 0,
                        active_sessions: 0,
                        uptime_seconds: 0,
                    },
                    sender: sender.clone(),
                    last_heartbeat: std::time::Instant::now(),
                    authenticated: true,
                    user_id: None, // TODO: Set from auth
                };

                agents.write().await.insert(agent_id, agent_connection);
                let _ = broadcast_tx.send(ServerEvent::AgentConnected(agent_id));
                
                tracing::info!("Agent {} registered: {}", agent_id, agent_info.name);
            },

            RelayMessage::AgentHeartbeat { agent_id, status } => {
                if let Some(agent) = agents.write().await.get_mut(&agent_id) {
                    agent.status = status;
                    agent.last_heartbeat = std::time::Instant::now();
                }
            },

            RelayMessage::SessionRequest { session_id, agent_id, session_type, user_id } => {
                // Find the target agent
                if let Some(agent) = agents.read().await.get(&agent_id) {
                    // Forward session request to agent
                    let request = RelayMessage::SessionRequest {
                        session_id,
                        agent_id,
                        session_type,
                        user_id,
                    };
                    agent.sender.send(request)?;
                } else {
                    let error = RelayMessage::Error {
                        code: 404,
                        message: "Agent not found or offline".to_string(),
                    };
                    sender.send(error)?;
                }
            },

            RelayMessage::ScreenFrame { session_id, .. } => {
                // Forward screen frame to session participants
                if let Some(session) = sessions.read().await.get(&session_id) {
                    if let Some(client_sender) = &session.client_sender {
                        client_sender.send(message)?;
                    }
                }
            },

            RelayMessage::ScreenControl { session_id, .. } => {
                // Forward control events to agent
                if let Some(session) = sessions.read().await.get(&session_id) {
                    if let Some(agent_sender) = &session.agent_sender {
                        agent_sender.send(message)?;
                    }
                }
            },

            RelayMessage::Ping { timestamp } => {
                let pong = RelayMessage::Pong { timestamp };
                sender.send(pong)?;
            },

            _ => {
                tracing::warn!("Unhandled message type: {:?}", message);
            }
        }

        Ok(())
    }

    /// Get list of online agents
    pub async fn get_online_agents(&self) -> Vec<AgentInfo> {
        self.agents
            .read()
            .await
            .values()
            .map(|conn| conn.info.clone())
            .collect()
    }

    /// Get active sessions count
    pub async fn get_active_sessions_count(&self) -> usize {
        self.sessions.read().await.len()
    }

    /// Start a cleanup task to remove stale connections
    pub fn start_cleanup_task(self: Arc<Self>) {
        let agents = Arc::clone(&self.agents);
        let heartbeat_timeout = std::time::Duration::from_secs(60);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                let mut to_remove = Vec::new();
                {
                    let agents_read = agents.read().await;
                    for (id, agent) in agents_read.iter() {
                        if agent.last_heartbeat.elapsed() > heartbeat_timeout {
                            to_remove.push(*id);
                        }
                    }
                }

                if !to_remove.is_empty() {
                    let mut agents_write = agents.write().await;
                    for id in to_remove {
                        agents_write.remove(&id);
                        tracing::info!("Removed stale agent connection: {}", id);
                    }
                }
            }
        });
    }
}
