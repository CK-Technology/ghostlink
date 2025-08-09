use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};

use crate::config::ClientConfig;
use crate::connection::RelayConnection;
use crate::session::{Session, SessionType};

pub mod heartbeat;
// pub mod installer;
pub mod session_manager;

// Re-export SessionManager
pub use session_manager::SessionManager;

/// Main agent orchestrator that manages all client operations
pub struct Agent {
    config: ClientConfig,
    relay_connection: Arc<RwLock<Option<RelayConnection>>>,
    session_manager: Arc<SessionManager>,
    shutdown_tx: mpsc::Sender<()>,
    shutdown_rx: mpsc::Receiver<()>,
}

#[derive(Debug, Clone)]
pub enum AgentMessage {
    Connect,
    Disconnect,
    StartSession { session_type: SessionType, session_id: String },
    StopSession { session_id: String },
    Shutdown,
}

impl Agent {
    pub fn new(config: ClientConfig) -> Result<Self> {
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
        
        Ok(Self {
            config,
            relay_connection: Arc::new(RwLock::new(None)),
            session_manager: Arc::new(SessionManager::new()),
            shutdown_tx,
            shutdown_rx,
        })
    }

    /// Start the agent and all background tasks
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting AtlasConnect Agent");
        
        // Start heartbeat task
        self.start_heartbeat_task().await?;
        
        // Connect to server
        self.connect_to_server().await?;
        
        // Start main event loop
        self.run_event_loop().await
    }

    /// Connect to the AtlasConnect server
    async fn connect_to_server(&self) -> Result<()> {
        info!("Connecting to server: {}", self.config.server_url);
        
        let connection = RelayConnection::new(&self.config).await
            .context("Failed to create relay connection")?;
        
        let mut relay_lock = self.relay_connection.write().await;
        *relay_lock = Some(connection);
        
        info!("Successfully connected to server");
        Ok(())
    }

    /// Start the heartbeat task to maintain server connection
    async fn start_heartbeat_task(&self) -> Result<()> {
        let connection = Arc::clone(&self.relay_connection);
        let heartbeat_interval = Duration::from_secs(self.config.heartbeat_interval);
        
        tokio::spawn(async move {
            let mut interval = interval(heartbeat_interval);
            
            loop {
                interval.tick().await;
                
                let conn_guard = connection.read().await;
                if let Some(conn) = conn_guard.as_ref() {
                    if let Err(e) = conn.send_heartbeat().await {
                        error!("Failed to send heartbeat: {}", e);
                        // TODO: Trigger reconnection
                    }
                }
            }
        });
        
        Ok(())
    }

    /// Main event loop for processing agent messages
    async fn run_event_loop(&mut self) -> Result<()> {
        info!("Agent event loop started");
        
        // TODO: Set up message channel for handling server messages
        loop {
            tokio::select! {
                // Handle shutdown signal
                _ = self.shutdown_rx.recv() => {
                    info!("Received shutdown signal");
                    break;
                }
                
                // Handle server messages
                // TODO: Implement server message handling
                
                // Keep the loop alive
                _ = tokio::time::sleep(Duration::from_millis(100)) => {}
            }
        }
        
        self.cleanup().await?;
        Ok(())
    }

    /// Handle incoming session request from server
    pub async fn handle_session_request(
        &self,
        session_type: SessionType,
        session_id: String,
    ) -> Result<()> {
        info!("Received session request: {} ({})", session_id, session_type);
        
        // Create new session
        let session = Session::new(session_id.clone(), session_type, &self.config).await?;
        
        // Register with session manager
        self.session_manager.add_session(session_id, session).await?;
        
        Ok(())
    }

    /// Stop a running session
    pub async fn stop_session(&self, session_id: &str) -> Result<()> {
        info!("Stopping session: {}", session_id);
        
        self.session_manager.remove_session(session_id).await?;
        
        Ok(())
    }

    /// Get agent system information for server registration
    pub fn get_system_info(&self) -> serde_json::Value {
        use sysinfo::{System, Disks};
        
        let mut sys = System::new_all();
        sys.refresh_all();
        
        serde_json::json!({
            "hostname": System::host_name().unwrap_or_else(|| "unknown".to_string()),
            "os": format!("{} {}", System::name().unwrap_or_default(), System::os_version().unwrap_or_default()),
            "kernel": System::kernel_version().unwrap_or_default(),
            "cpu": {
                "brand": sys.cpus().first().map(|cpu| cpu.brand()).unwrap_or("Unknown"),
                "cores": sys.cpus().len(),
                "frequency": sys.cpus().first().map(|cpu| cpu.frequency()).unwrap_or(0),
            },
            "memory": {
                "total": sys.total_memory(),
                "available": sys.available_memory(),
            },
            "disks": Disks::new_with_refreshed_list().iter().map(|disk| {
                serde_json::json!({
                    "name": disk.name().to_string_lossy(),
                    "mount_point": disk.mount_point().to_string_lossy(),
                    "total_space": disk.total_space(),
                    "available_space": disk.available_space(),
                    "file_system": disk.file_system().to_string_lossy(),
                })
            }).collect::<Vec<_>>(),
            "uptime": System::uptime(),
            "agent_version": env!("CARGO_PKG_VERSION"),
        })
    }

    /// Graceful shutdown
    pub async fn shutdown(&self) -> Result<()> {
        info!("Initiating agent shutdown");
        
        let _ = self.shutdown_tx.send(()).await;
        
        Ok(())
    }

    /// Cleanup resources on shutdown
    async fn cleanup(&self) -> Result<()> {
        info!("Cleaning up agent resources");
        
        // Stop all active sessions
        self.session_manager.shutdown_all().await?;
        
        // Close server connection
        let mut relay_lock = self.relay_connection.write().await;
        if let Some(connection) = relay_lock.take() {
            connection.disconnect().await?;
        }
        
        info!("Agent cleanup completed");
        Ok(())
    }
}
