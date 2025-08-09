use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::session::{Session, SessionType};

/// Manages multiple concurrent remote sessions
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, Session>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a new session
    pub async fn add_session(&self, session_id: String, session: Session) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        
        if sessions.contains_key(&session_id) {
            warn!("Session {} already exists, replacing", session_id);
        }
        
        sessions.insert(session_id.clone(), session);
        info!("Added session: {}", session_id);
        
        Ok(())
    }

    /// Remove and stop a session
    pub async fn remove_session(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(session) = sessions.remove(session_id) {
            // Stop the session gracefully
            if let Err(e) = session.stop().await {
                error!("Error stopping session {}: {}", session_id, e);
            }
            info!("Removed session: {}", session_id);
        } else {
            warn!("Attempted to remove non-existent session: {}", session_id);
        }
        
        Ok(())
    }

    /// Get a reference to a session
    pub async fn get_session(&self, session_id: &str) -> Option<Session> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }

    /// List all active session IDs
    pub async fn list_sessions(&self) -> Vec<String> {
        let sessions = self.sessions.read().await;
        sessions.keys().cloned().collect()
    }

    /// Get session count by type
    pub async fn get_session_stats(&self) -> HashMap<SessionType, usize> {
        let sessions = self.sessions.read().await;
        let mut stats = HashMap::new();
        
        for session in sessions.values() {
            let session_type = session.session_type();
            *stats.entry(session_type).or_insert(0) += 1;
        }
        
        stats
    }

    /// Check if we can accept more sessions (rate limiting)
    pub async fn can_accept_session(&self, session_type: SessionType) -> bool {
        let sessions = self.sessions.read().await;
        let current_count = sessions.values()
            .filter(|s| s.session_type() == session_type)
            .count();
        
        // Limit concurrent sessions by type
        match session_type {
            SessionType::Backstage => current_count < 5,  // Max 5 backstage sessions
            SessionType::Console => current_count < 2,    // Max 2 console sessions
            SessionType::AdHoc => current_count < 1,      // Max 1 ad-hoc session
        }
    }

    /// Shutdown all sessions gracefully
    pub async fn shutdown_all(&self) -> Result<()> {
        info!("Shutting down all sessions");
        
        let mut sessions = self.sessions.write().await;
        let session_ids: Vec<String> = sessions.keys().cloned().collect();
        
        for session_id in session_ids {
            if let Some(session) = sessions.remove(&session_id) {
                if let Err(e) = session.stop().await {
                    error!("Error stopping session {} during shutdown: {}", session_id, e);
                }
            }
        }
        
        info!("All sessions shut down");
        Ok(())
    }

    /// Get health status of all sessions
    pub async fn get_health_status(&self) -> HashMap<String, bool> {
        let sessions = self.sessions.read().await;
        let mut status = HashMap::new();
        
        for (id, session) in sessions.iter() {
            status.insert(id.clone(), session.is_healthy().await);
        }
        
        status
    }
}
