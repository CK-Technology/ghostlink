use crate::error::{GhostLinkError, Result};
use serde::{Deserialize, Serialize};
use std::{
    sync::{Arc, atomic::{AtomicBool, Ordering}},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    sync::RwLock,
    task::JoinHandle,
    time::{interval, sleep},
};
use tracing::{debug, error, info, trace, warn};

#[cfg(target_os = "linux")]
use x11rb::{
    connection::Connection,
    protocol::{
        xproto::{ConnectionExt, CreateWindowAux, EventMask, WindowClass, Screen},
        xfixes::ConnectionExt as XFixesConnectionExt,
    },
    rust_connection::RustConnection,
};

/// ScreenConnect-style connection banner displayed at top of screen
pub struct ConnectionBanner {
    /// Banner configuration
    config: BannerConfig,
    /// Current banner state
    state: Arc<RwLock<BannerState>>,
    /// X11 connection for banner display
    #[cfg(target_os = "linux")]
    x11_connection: Option<Arc<RustConnection>>,
    /// Banner window ID
    #[cfg(target_os = "linux")]
    window_id: Option<u32>,
    /// Banner display task
    banner_task: Arc<RwLock<Option<JoinHandle<()>>>>,
    /// Banner visibility
    is_visible: AtomicBool,
}

/// Banner configuration (customizable from server web GUI)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BannerConfig {
    /// Enable banner display
    pub enabled: bool,
    /// Banner message template
    pub message_template: String,
    /// Banner colors
    pub colors: BannerColors,
    /// Banner dimensions
    pub dimensions: BannerDimensions,
    /// Animation settings
    pub animation: AnimationConfig,
    /// Display behavior
    pub behavior: DisplayBehavior,
    /// Customization options
    pub customization: CustomizationOptions,
}

/// Banner color scheme
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BannerColors {
    /// Background color (RGBA)
    pub background: [u8; 4],
    /// Text color (RGBA)
    pub text: [u8; 4],
    /// Border color (RGBA)
    pub border: [u8; 4],
    /// Warning color (RGBA)
    pub warning: [u8; 4],
    /// Success color (RGBA)
    pub success: [u8; 4],
}

/// Banner dimensions and positioning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BannerDimensions {
    /// Height in pixels
    pub height: u32,
    /// Position from top
    pub position: BannerPosition,
    /// Text size
    pub font_size: u32,
    /// Padding
    pub padding: u32,
    /// Border width
    pub border_width: u32,
}

/// Banner position options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BannerPosition {
    /// Top center (ScreenConnect style)
    TopCenter,
    /// Top left
    TopLeft,
    /// Top right
    TopRight,
    /// Bottom center
    BottomCenter,
    /// Custom offset from top
    Custom { x: i32, y: i32 },
}

/// Animation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationConfig {
    /// Fade in duration (ms)
    pub fade_in_duration: u32,
    /// Fade out duration (ms)
    pub fade_out_duration: u32,
    /// Slide animation enabled
    pub slide_animation: bool,
    /// Pulse animation for alerts
    pub pulse_on_alert: bool,
    /// Animation easing
    pub easing: AnimationEasing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnimationEasing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

/// Display behavior settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayBehavior {
    /// Auto-hide after duration (ms)
    pub auto_hide_duration: Option<u32>,
    /// Hide on user activity
    pub hide_on_activity: bool,
    /// Show on connection events
    pub show_on_connection: bool,
    /// Show technician join/leave messages
    pub show_technician_events: bool,
    /// Require user acknowledgment
    pub require_acknowledgment: bool,
    /// Click-through enabled
    pub click_through: bool,
}

/// Customization options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomizationOptions {
    /// Company logo URL
    pub logo_url: Option<String>,
    /// Custom CSS styles
    pub custom_css: Option<String>,
    /// Custom font family
    pub font_family: Option<String>,
    /// Show session info
    pub show_session_info: bool,
    /// Show technician info
    pub show_technician_info: bool,
    /// Custom fields to display
    pub custom_fields: Vec<CustomField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomField {
    pub key: String,
    pub label: String,
    pub value: String,
    pub visible: bool,
}

/// Current banner state
#[derive(Debug, Clone)]
pub struct BannerState {
    /// Current message
    pub message: String,
    /// Message type
    pub message_type: MessageType,
    /// Connected technicians
    pub technicians: Vec<TechnicianInfo>,
    /// Session information
    pub session_info: SessionInfo,
    /// Banner visibility
    pub visible: bool,
    /// Last update time
    pub last_update: u64,
}

/// Message type for styling
#[derive(Debug, Clone, PartialEq)]
pub enum MessageType {
    Info,
    Warning,
    Error,
    Success,
    TechnicianJoined,
    TechnicianLeft,
    SessionStarted,
    SessionEnded,
}

/// Technician information for banner display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnicianInfo {
    /// User ID
    pub user_id: String,
    /// Display name
    pub display_name: String,
    /// Email
    pub email: String,
    /// Organization
    pub organization: String,
    /// Join time
    pub joined_at: u64,
    /// Permission level
    pub permission_level: String,
    /// Connection info
    pub connection_info: ConnectionInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub ip_address: String,
    pub user_agent: String,
    pub connection_method: String,
    pub quality: String,
}

/// Session information for banner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Session ID
    pub session_id: String,
    /// Session type
    pub session_type: String,
    /// Start time
    pub started_at: u64,
    /// Recording enabled
    pub recording_enabled: bool,
    /// Session purpose
    pub purpose: Option<String>,
}

impl Default for BannerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            message_template: "Remote session active - Technician: {technician_name} ({organization})".to_string(),
            colors: BannerColors {
                background: [255, 165, 0, 220],  // Orange with transparency
                text: [255, 255, 255, 255],      // White
                border: [255, 140, 0, 255],      // Dark orange
                warning: [255, 0, 0, 255],       // Red
                success: [0, 255, 0, 255],       // Green
            },
            dimensions: BannerDimensions {
                height: 60,
                position: BannerPosition::TopCenter,
                font_size: 14,
                padding: 10,
                border_width: 2,
            },
            animation: AnimationConfig {
                fade_in_duration: 500,
                fade_out_duration: 300,
                slide_animation: true,
                pulse_on_alert: true,
                easing: AnimationEasing::EaseInOut,
            },
            behavior: DisplayBehavior {
                auto_hide_duration: Some(5000), // 5 seconds
                hide_on_activity: false,
                show_on_connection: true,
                show_technician_events: true,
                require_acknowledgment: false,
                click_through: false,
            },
            customization: CustomizationOptions {
                logo_url: None,
                custom_css: None,
                font_family: Some("Arial, sans-serif".to_string()),
                show_session_info: true,
                show_technician_info: true,
                custom_fields: Vec::new(),
            },
        }
    }
}

impl ConnectionBanner {
    /// Create new connection banner
    pub async fn new(config: BannerConfig) -> Result<Self> {
        info!("Creating connection banner");
        
        let state = BannerState {
            message: "Initializing remote session...".to_string(),
            message_type: MessageType::Info,
            technicians: Vec::new(),
            session_info: SessionInfo {
                session_id: "unknown".to_string(),
                session_type: "unknown".to_string(),
                started_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                recording_enabled: false,
                purpose: None,
            },
            visible: false,
            last_update: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        };
        
        #[cfg(target_os = "linux")]
        let (x11_connection, window_id) = if config.enabled {
            Self::initialize_x11_banner(&config).await?
        } else {
            (None, None)
        };
        
        #[cfg(not(target_os = "linux"))]
        let (x11_connection, window_id) = (None, None);
        
        Ok(Self {
            config,
            state: Arc::new(RwLock::new(state)),
            #[cfg(target_os = "linux")]
            x11_connection,
            #[cfg(target_os = "linux")]
            window_id,
            banner_task: Arc::new(RwLock::new(None)),
            is_visible: AtomicBool::new(false),
        })
    }
    
    /// Show banner with message
    pub async fn show_message(&self, message: &str, message_type: MessageType) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        
        debug!("Showing banner message: {} ({:?})", message, message_type);
        
        {
            let mut state = self.state.write().await;
            state.message = message.to_string();
            state.message_type = message_type;
            state.visible = true;
            state.last_update = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        }
        
        self.is_visible.store(true, Ordering::Relaxed);
        self.update_banner_display().await?;
        
        // Auto-hide if configured
        if let Some(duration) = self.config.behavior.auto_hide_duration {
            let state = Arc::clone(&self.state);
            let is_visible = Arc::clone(&self.is_visible);
            
            tokio::spawn(async move {
                sleep(Duration::from_millis(duration as u64)).await;
                let mut state_guard = state.write().await;
                state_guard.visible = false;
                is_visible.store(false, Ordering::Relaxed);
            });
        }
        
        Ok(())
    }
    
    /// Show technician connected message
    pub async fn show_technician_connected(&self, technician: TechnicianInfo) -> Result<()> {
        if !self.config.enabled || !self.config.behavior.show_technician_events {
            return Ok(());
        }
        
        let message = format!(
            "Technician {} ({}) has connected to this session",
            technician.display_name,
            technician.organization
        );
        
        // Add to technician list
        {
            let mut state = self.state.write().await;
            state.technicians.push(technician);
        }
        
        self.show_message(&message, MessageType::TechnicianJoined).await?;
        
        info!("Showed technician connected banner");
        Ok(())
    }
    
    /// Show technician disconnected message
    pub async fn show_technician_disconnected(&self, user_id: &str) -> Result<()> {
        if !self.config.enabled || !self.config.behavior.show_technician_events {
            return Ok(());
        }
        
        let technician_name = {
            let mut state = self.state.write().await;
            if let Some(pos) = state.technicians.iter().position(|t| t.user_id == user_id) {
                let technician = state.technicians.remove(pos);
                technician.display_name
            } else {
                "Unknown".to_string()
            }
        };
        
        let message = format!("Technician {} has disconnected", technician_name);
        self.show_message(&message, MessageType::TechnicianLeft).await?;
        
        info!("Showed technician disconnected banner");
        Ok(())
    }
    
    /// Update session information
    pub async fn update_session_info(&self, session_info: SessionInfo) -> Result<()> {
        {
            let mut state = self.state.write().await;
            state.session_info = session_info;
        }
        
        if self.is_visible.load(Ordering::Relaxed) {
            self.update_banner_display().await?;
        }
        
        Ok(())
    }
    
    /// Hide banner
    pub async fn hide(&self) -> Result<()> {
        debug!("Hiding connection banner");
        
        {
            let mut state = self.state.write().await;
            state.visible = false;
        }
        
        self.is_visible.store(false, Ordering::Relaxed);
        
        #[cfg(target_os = "linux")]
        self.hide_x11_banner().await?;
        
        Ok(())
    }
    
    /// Update banner configuration from server
    pub async fn update_config(&mut self, new_config: BannerConfig) -> Result<()> {
        info!("Updating banner configuration");
        
        // Hide current banner if enabled status changed
        if self.config.enabled && !new_config.enabled {
            self.hide().await?;
        }
        
        self.config = new_config;
        
        // Reinitialize if needed
        #[cfg(target_os = "linux")]
        if self.config.enabled {
            let (connection, window_id) = Self::initialize_x11_banner(&self.config).await?;
            self.x11_connection = connection;
            self.window_id = window_id;
        }
        
        Ok(())
    }
    
    /// Update banner display with current state
    async fn update_banner_display(&self) -> Result<()> {
        #[cfg(target_os = "linux")]
        self.update_x11_banner().await?;
        
        #[cfg(not(target_os = "linux"))]
        {
            debug!("Banner display not implemented for this platform");
        }
        
        Ok(())
    }
    
    /// Initialize X11 banner window
    #[cfg(target_os = "linux")]
    async fn initialize_x11_banner(config: &BannerConfig) -> Result<(Option<Arc<RustConnection>>, Option<u32>)> {
        // Connect to X11 display
        let (connection, screen_num) = x11rb::connect(None)
            .map_err(|e| GhostLinkError::Other(format!("Failed to connect to X11: {}", e)))?;
        
        let connection = Arc::new(connection);
        let screen = &connection.setup().roots[screen_num];
        
        // Calculate banner dimensions and position
        let (banner_x, banner_y, banner_width) = Self::calculate_banner_geometry(config, screen);
        
        // Create banner window
        let window_id = connection.generate_id()?;
        let window_aux = CreateWindowAux::new()
            .background_pixel(Self::rgba_to_pixel(&config.colors.background))
            .border_pixel(Self::rgba_to_pixel(&config.colors.border))
            .event_mask(EventMask::EXPOSURE | EventMask::BUTTON_PRESS)
            .override_redirect(1); // Make it appear above all windows
        
        connection.create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            window_id,
            screen.root,
            banner_x,
            banner_y,
            banner_width,
            config.dimensions.height as u16,
            config.dimensions.border_width as u16,
            WindowClass::INPUT_OUTPUT,
            screen.root_visual,
            &window_aux,
        )?;
        
        // Make window always on top
        connection.map_window(window_id)?;
        connection.flush()?;
        
        info!("X11 banner window created: {}x{} at ({}, {})", 
            banner_width, config.dimensions.height, banner_x, banner_y);
        
        Ok((Some(connection), Some(window_id)))
    }
    
    /// Calculate banner geometry based on screen and config
    #[cfg(target_os = "linux")]
    fn calculate_banner_geometry(config: &BannerConfig, screen: &Screen) -> (i16, i16, u16) {
        let screen_width = screen.width_in_pixels;
        let screen_height = screen.height_in_pixels;
        
        let banner_width = (screen_width as f32 * 0.8) as u16; // 80% of screen width
        let banner_height = config.dimensions.height as u16;
        
        let (x, y) = match config.dimensions.position {
            BannerPosition::TopCenter => (
                ((screen_width - banner_width) / 2) as i16,
                10,
            ),
            BannerPosition::TopLeft => (10, 10),
            BannerPosition::TopRight => (
                (screen_width - banner_width - 10) as i16,
                10,
            ),
            BannerPosition::BottomCenter => (
                ((screen_width - banner_width) / 2) as i16,
                (screen_height - banner_height - 10) as i16,
            ),
            BannerPosition::Custom { x, y } => (x as i16, y as i16),
        };
        
        (x, y, banner_width)
    }
    
    /// Convert RGBA to X11 pixel value
    #[cfg(target_os = "linux")]
    fn rgba_to_pixel(rgba: &[u8; 4]) -> u32 {
        ((rgba[3] as u32) << 24) | // Alpha
        ((rgba[0] as u32) << 16) | // Red
        ((rgba[1] as u32) << 8) |  // Green
        (rgba[2] as u32)           // Blue
    }
    
    /// Update X11 banner content
    #[cfg(target_os = "linux")]
    async fn update_x11_banner(&self) -> Result<()> {
        if let (Some(ref connection), Some(window_id)) = (&self.x11_connection, self.window_id) {
            let state = self.state.read().await;
            
            if !state.visible {
                // Unmap window to hide it
                connection.unmap_window(window_id)?;
            } else {
                // Map window to show it
                connection.map_window(window_id)?;
                
                // TODO: Draw banner content with text and graphics
                // This would involve creating a graphics context and drawing text
                // For now, we just show/hide the colored window
            }
            
            connection.flush()?;
        }
        
        Ok(())
    }
    
    /// Hide X11 banner
    #[cfg(target_os = "linux")]
    async fn hide_x11_banner(&self) -> Result<()> {
        if let (Some(ref connection), Some(window_id)) = (&self.x11_connection, self.window_id) {
            connection.unmap_window(window_id)?;
            connection.flush()?;
        }
        
        Ok(())
    }
    
    /// Format message with template variables
    fn format_message_template(&self, template: &str, state: &BannerState) -> String {
        let mut message = template.to_string();
        
        // Replace template variables
        if let Some(first_tech) = state.technicians.first() {
            message = message.replace("{technician_name}", &first_tech.display_name);
            message = message.replace("{organization}", &first_tech.organization);
            message = message.replace("{technician_email}", &first_tech.email);
        }
        
        message = message.replace("{session_id}", &state.session_info.session_id);
        message = message.replace("{session_type}", &state.session_info.session_type);
        message = message.replace("{technician_count}", &state.technicians.len().to_string());
        
        if state.session_info.recording_enabled {
            message = message.replace("{recording_status}", "Recording");
        } else {
            message = message.replace("{recording_status}", "Not Recording");
        }
        
        message
    }
    
    /// Get current banner state
    pub async fn get_state(&self) -> BannerState {
        self.state.read().await.clone()
    }
    
    /// Check if banner is visible
    pub fn is_visible(&self) -> bool {
        self.is_visible.load(Ordering::Relaxed)
    }
}

impl Drop for ConnectionBanner {
    fn drop(&mut self) {
        // Cleanup X11 resources
        #[cfg(target_os = "linux")]
        if let (Some(ref connection), Some(window_id)) = (&self.x11_connection, self.window_id) {
            let _ = connection.destroy_window(window_id);
            let _ = connection.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_banner_creation() {
        let config = BannerConfig::default();
        let banner = ConnectionBanner::new(config).await.unwrap();
        
        assert!(!banner.is_visible());
    }
    
    #[tokio::test]
    async fn test_message_formatting() {
        let config = BannerConfig::default();
        let banner = ConnectionBanner::new(config).await.unwrap();
        
        let technician = TechnicianInfo {
            user_id: "tech1".to_string(),
            display_name: "John Doe".to_string(),
            email: "john@example.com".to_string(),
            organization: "TechCorp".to_string(),
            joined_at: 0,
            permission_level: "admin".to_string(),
            connection_info: ConnectionInfo {
                ip_address: "192.168.1.100".to_string(),
                user_agent: "GhostLink".to_string(),
                connection_method: "direct".to_string(),
                quality: "high".to_string(),
            },
        };
        
        banner.show_technician_connected(technician).await.unwrap();
        
        let state = banner.get_state().await;
        assert!(state.message.contains("John Doe"));
        assert!(state.message.contains("TechCorp"));
    }
}