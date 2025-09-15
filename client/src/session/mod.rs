pub mod window;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::capture::ScreenCapture;
use crate::config::ClientConfig;
use crate::input::InputController;

pub use window::{SessionWindow, SessionTab};

// pub mod backstage;
// pub mod console;

/// Session types available in AtlasConnect
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionType {
    /// Unattended admin access - silent, no user notification
    Backstage,
    /// Interactive remote control - user sees what's happening
    Console,
    /// One-time access code session
    AdHoc,
}

impl std::fmt::Display for SessionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionType::Backstage => write!(f, "backstage"),
            SessionType::Console => write!(f, "console"),
            SessionType::AdHoc => write!(f, "adhoc"),
        }
    }
}

/// Represents an active remote session
#[derive(Clone)]
pub struct Session {
    pub id: String,
    pub session_type: SessionType,
    screen_capture: Arc<RwLock<Option<ScreenCapture>>>,
    input_controller: Arc<RwLock<Option<InputController>>>,
    is_active: Arc<RwLock<bool>>,
    config: ClientConfig,
}

impl Session {
    /// Create a new session
    pub async fn new(
        id: String,
        session_type: SessionType,
        config: &ClientConfig,
    ) -> Result<Self> {
        info!("Creating new {} session: {}", session_type, id);
        
        let session = Self {
            id,
            session_type,
            screen_capture: Arc::new(RwLock::new(None)),
            input_controller: Arc::new(RwLock::new(None)),
            is_active: Arc::new(RwLock::new(false)),
            config: config.clone(),
        };
        
        // Initialize session based on type
        session.initialize_session().await?;
        
        Ok(session)
    }

    /// Initialize session components based on session type
    async fn initialize_session(&self) -> Result<()> {
        match self.session_type {
            SessionType::Backstage => {
                self.initialize_backstage_session().await?;
            }
            SessionType::Console => {
                self.initialize_console_session().await?;
            }
            SessionType::AdHoc => {
                self.initialize_adhoc_session().await?;
            }
        }
        
        let mut active_guard = self.is_active.write().await;
        *active_guard = true;
        
        info!("Session {} initialized successfully", self.id);
        Ok(())
    }

    /// Initialize backstage (unattended admin) session
    async fn initialize_backstage_session(&self) -> Result<()> {
        info!("Initializing backstage session: {}", self.id);
        
        // For backstage sessions:
        // 1. No user notification
        // 2. Start screen capture immediately
        // 3. Enable full input control
        // 4. Optional: blank user screen
        // 5. Elevate privileges if needed
        
        // Initialize screen capture
        let capture = ScreenCapture::new(self.session_type).await?;
        let mut capture_guard = self.screen_capture.write().await;
        *capture_guard = Some(capture);
        
        // Initialize input controller
        let input = InputController::new(self.session_type).await?;
        let mut input_guard = self.input_controller.write().await;
        *input_guard = Some(input);
        
        // TODO: Implement privilege elevation
        // TODO: Implement optional screen blanking
        
        Ok(())
    }

    /// Initialize console (interactive) session
    async fn initialize_console_session(&self) -> Result<()> {
        info!("Initializing console session: {}", self.id);
        
        // For console sessions:
        // 1. Show user notification (optional)
        // 2. Start screen capture
        // 3. Enable input control
        // 4. User can see remote cursor
        
        // Initialize screen capture
        let capture = ScreenCapture::new(self.session_type).await?;
        let mut capture_guard = self.screen_capture.write().await;
        *capture_guard = Some(capture);
        
        // Initialize input controller
        let input = InputController::new(self.session_type).await?;
        let mut input_guard = self.input_controller.write().await;
        *input_guard = Some(input);
        
        // TODO: Show user notification
        // TODO: Enable remote cursor display
        
        Ok(())
    }

    /// Initialize ad-hoc (temporary) session
    async fn initialize_adhoc_session(&self) -> Result<()> {
        info!("Initializing ad-hoc session: {}", self.id);
        
        // Ad-hoc sessions are similar to console but temporary
        // They auto-expire and don't persist agent registration
        
        self.initialize_console_session().await?;
        
        // TODO: Set session expiration timer
        
        Ok(())
    }

    /// Start screen capture streaming
    pub async fn start_screen_capture(&self) -> Result<()> {
        let capture_guard = self.screen_capture.read().await;
        
        if let Some(capture) = capture_guard.as_ref() {
            capture.start_streaming().await?;
            info!("Screen capture started for session: {}", self.id);
        } else {
            return Err(anyhow::anyhow!("Screen capture not initialized"));
        }
        
        Ok(())
    }

    /// Stop screen capture streaming
    pub async fn stop_screen_capture(&self) -> Result<()> {
        let capture_guard = self.screen_capture.read().await;
        
        if let Some(capture) = capture_guard.as_ref() {
            capture.stop_streaming().await?;
            info!("Screen capture stopped for session: {}", self.id);
        }
        
        Ok(())
    }

    /// Handle input event from remote operator
    pub async fn handle_input_event(&self, event_data: &serde_json::Value) -> Result<()> {
        let input_guard = self.input_controller.read().await;
        
        if let Some(input) = input_guard.as_ref() {
            input.handle_event(event_data).await?;
        } else {
            warn!("Input controller not initialized for session: {}", self.id);
        }
        
        Ok(())
    }

    /// Enable screen blanking (hide user's screen)
    pub async fn enable_screen_blanking(&self) -> Result<()> {
        if self.session_type != SessionType::Backstage {
            warn!("Screen blanking only available for backstage sessions");
            return Ok(());
        }
        
        // TODO: Implement platform-specific screen blanking
        info!("Screen blanking enabled for session: {}", self.id);
        Ok(())
    }

    /// Disable screen blanking (restore user's screen)
    pub async fn disable_screen_blanking(&self) -> Result<()> {
        // TODO: Implement platform-specific screen un-blanking
        info!("Screen blanking disabled for session: {}", self.id);
        Ok(())
    }

    /// Enable input blocking (disable user input)
    pub async fn enable_input_blocking(&self) -> Result<()> {
        let input_guard = self.input_controller.read().await;
        
        if let Some(input) = input_guard.as_ref() {
            input.block_user_input().await?;
            info!("Input blocking enabled for session: {}", self.id);
        }
        
        Ok(())
    }

    /// Disable input blocking (restore user input)
    pub async fn disable_input_blocking(&self) -> Result<()> {
        let input_guard = self.input_controller.read().await;
        
        if let Some(input) = input_guard.as_ref() {
            input.unblock_user_input().await?;
            info!("Input blocking disabled for session: {}", self.id);
        }
        
        Ok(())
    }

    /// Get session type
    pub fn session_type(&self) -> SessionType {
        self.session_type
    }

    /// Check if session is active
    pub async fn is_active(&self) -> bool {
        let active_guard = self.is_active.read().await;
        *active_guard
    }

    /// Check session health
    pub async fn is_healthy(&self) -> bool {
        let is_active = self.is_active().await;
        
        if !is_active {
            return false;
        }
        
        // Check if capture and input are healthy
        let capture_guard = self.screen_capture.read().await;
        let input_guard = self.input_controller.read().await;
        
        let capture_healthy = if let Some(capture) = capture_guard.as_ref() {
            capture.is_healthy().await
        } else {
            false
        };
        let input_healthy = input_guard.as_ref().map_or(false, |i| i.is_healthy());
        
        capture_healthy && input_healthy
    }

    /// Stop the session
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping session: {}", self.id);
        
        let mut active_guard = self.is_active.write().await;
        *active_guard = false;
        
        // Stop screen capture
        if let Err(e) = self.stop_screen_capture().await {
            error!("Error stopping screen capture: {}", e);
        }
        
        // Disable input blocking if enabled
        if let Err(e) = self.disable_input_blocking().await {
            error!("Error disabling input blocking: {}", e);
        }
        
        // Disable screen blanking if enabled
        if let Err(e) = self.disable_screen_blanking().await {
            error!("Error disabling screen blanking: {}", e);
        }
        
        // Clean up resources
        let mut capture_guard = self.screen_capture.write().await;
        *capture_guard = None;
        
        let mut input_guard = self.input_controller.write().await;
        *input_guard = None;
        
        info!("Session {} stopped successfully", self.id);
        Ok(())
    }
}
