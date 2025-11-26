use async_trait::async_trait;
use tracing::{debug, info, warn};
use std::process::Command;
use crate::error::{Result, CaptureError, GhostLinkError};
use super::{Frame, PixelFormat, ScreenCapturer};

/// Wayland screen capturer using PipeWire and portal APIs
pub struct WaylandCapturer {
    session_token: Option<String>,
    pipewire_node_id: Option<u32>,
    width: u32,
    height: u32,
    is_initialized: bool,
    use_portal: bool,
}

impl WaylandCapturer {
    pub async fn new() -> Result<Self> {
        info!("Initializing Wayland screen capturer");
        
        // Check if we're running under Wayland
        let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
        if session_type != "wayland" {
            return Err(GhostLinkError::Capture(CaptureError::UnsupportedPlatform {
                platform: format!("Session type: {}", session_type),
            }));
        }
        
        // Check for PipeWire support
        let has_pipewire = Self::check_pipewire_available();
        if !has_pipewire {
            warn!("PipeWire not available, some features may be limited");
        }
        
        // Check for xdg-desktop-portal support (for secure screen capture)
        let has_portal = Self::check_portal_available();
        
        Ok(Self {
            session_token: None,
            pipewire_node_id: None,
            width: 0,
            height: 0,
            is_initialized: false,
            use_portal: has_portal,
        })
    }
    
    /// Check if PipeWire is available
    fn check_pipewire_available() -> bool {
        Command::new("pw-cli")
            .arg("--version")
            .output()
            .is_ok()
    }
    
    /// Check if xdg-desktop-portal is available
    fn check_portal_available() -> bool {
        // Check for portal support via D-Bus
        Command::new("busctl")
            .args(&[
                "--user",
                "introspect",
                "org.freedesktop.portal.Desktop",
                "/org/freedesktop/portal/desktop"
            ])
            .output()
            .is_ok()
    }
    
    /// Initialize screen capture via portal (secure method)
    async fn init_portal_capture(&mut self) -> Result<()> {
        info!("Initializing screen capture via xdg-desktop-portal");
        
        // This would use D-Bus to communicate with xdg-desktop-portal
        // For now, we'll use a simplified approach
        
        // Request screen cast permission
        let output = Command::new("gdbus")
            .args(&[
                "call",
                "--session",
                "--dest", "org.freedesktop.portal.Desktop",
                "--object-path", "/org/freedesktop/portal/desktop",
                "--method", "org.freedesktop.portal.ScreenCast.CreateSession",
                "{'session_handle_token': <'ghostlink_session'>, 'handle_token': <'ghostlink_handle'>}"
            ])
            .output()
            .map_err(|e| GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: format!("Failed to create portal session: {}", e),
            }))?;
        
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: format!("Portal session creation failed: {}", error),
            }));
        }
        
        // Parse session token from output
        let stdout = String::from_utf8_lossy(&output.stdout);
        debug!("Portal session created: {}", stdout);
        
        // Store session token for later use
        self.session_token = Some("ghostlink_session".to_string());
        
        Ok(())
    }
    
    /// Initialize PipeWire capture directly (may require permissions)
    async fn init_pipewire_direct(&mut self) -> Result<()> {
        info!("Initializing direct PipeWire capture");
        
        // Get available sources using pw-cli
        let output = Command::new("pw-cli")
            .args(&["ls", "Node"])
            .output()
            .map_err(|e| GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: format!("Failed to list PipeWire nodes: {}", e),
            }))?;
        
        if !output.status.success() {
            return Err(GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: "Failed to list PipeWire nodes".to_string(),
            }));
        }
        
        let nodes = String::from_utf8_lossy(&output.stdout);
        debug!("Available PipeWire nodes: {}", nodes);
        
        // TODO: Parse nodes and find screen capture source
        // For now, use a default node ID
        self.pipewire_node_id = Some(0);
        
        Ok(())
    }
    
    /// Capture frame using grim (Wayland screenshot tool)
    async fn capture_with_grim(&self) -> Result<Vec<u8>> {
        // Use grim for basic screenshot capability
        let output = Command::new("grim")
            .args(&["-t", "png", "-"])  // Output to stdout as PNG
            .output()
            .map_err(|e| GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: format!("grim capture failed: {}", e),
            }))?;
        
        if !output.status.success() {
            return Err(GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: "grim capture failed".to_string(),
            }));
        }
        
        Ok(output.stdout)
    }
    
    /// Capture frame using wlr-screencopy protocol (if available)
    async fn capture_with_wlr_screencopy(&self) -> Result<Vec<u8>> {
        // This would use the wlr-screencopy Wayland protocol
        // For now, fall back to grim
        self.capture_with_grim().await
    }
    
    /// Get screen dimensions
    async fn get_screen_info(&mut self) -> Result<()> {
        // Try to get screen info using swaymsg (for Sway compositor)
        if let Ok(output) = Command::new("swaymsg")
            .args(&["-t", "get_outputs", "-r"])
            .output()
        {
            if output.status.success() {
                let json_str = String::from_utf8_lossy(&output.stdout);
                // Parse JSON to get screen dimensions
                // For now, use default values
                self.width = 1920;
                self.height = 1080;
                debug!("Sway outputs: {}", json_str);
                return Ok(());
            }
        }
        
        // Try wlr-randr as fallback
        if let Ok(output) = Command::new("wlr-randr")
            .output()
        {
            if output.status.success() {
                let info = String::from_utf8_lossy(&output.stdout);
                // Parse output to get screen dimensions
                // For now, use default values
                self.width = 1920;
                self.height = 1080;
                debug!("wlr-randr output: {}", info);
                return Ok(());
            }
        }
        
        // Default fallback
        self.width = 1920;
        self.height = 1080;
        warn!("Could not detect screen dimensions, using defaults: {}x{}", self.width, self.height);
        
        Ok(())
    }
}

#[async_trait]
impl ScreenCapturer for WaylandCapturer {
    async fn initialize(&mut self) -> Result<()> {
        info!("Initializing Wayland screen capturer");
        
        // Get screen information
        self.get_screen_info().await?;
        
        // Try portal-based capture first (more secure)
        if self.use_portal {
            match self.init_portal_capture().await {
                Ok(_) => {
                    info!("Portal-based capture initialized successfully");
                    self.is_initialized = true;
                    return Ok(());
                }
                Err(e) => {
                    warn!("Portal capture failed, trying direct method: {}", e);
                }
            }
        }
        
        // Try direct PipeWire capture
        if Self::check_pipewire_available() {
            match self.init_pipewire_direct().await {
                Ok(_) => {
                    info!("Direct PipeWire capture initialized successfully");
                    self.is_initialized = true;
                    return Ok(());
                }
                Err(e) => {
                    warn!("Direct PipeWire capture failed: {}", e);
                }
            }
        }
        
        // Fall back to basic screenshot tools
        info!("Using fallback screenshot-based capture method");
        self.is_initialized = true;
        
        Ok(())
    }
    
    async fn capture_frame(&mut self) -> Result<Frame> {
        if !self.is_initialized {
            return Err(GhostLinkError::Capture(CaptureError::NotInitialized));
        }
        
        // Try different capture methods in order of preference
        let png_data = if self.session_token.is_some() {
            // Use portal-based capture
            // TODO: Implement actual portal frame capture
            self.capture_with_wlr_screencopy().await?
        } else if self.pipewire_node_id.is_some() {
            // Use direct PipeWire capture
            // TODO: Implement actual PipeWire frame capture
            self.capture_with_grim().await?
        } else {
            // Fallback to screenshot tools
            self.capture_with_grim().await?
        };
        
        // Decode PNG to raw frame data
        let decoder = png::Decoder::new(&png_data[..]);
        let mut reader = decoder.read_info()
            .map_err(|e| GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: format!("PNG decode failed: {}", e),
            }))?;
        
        let mut buf = vec![0; reader.output_buffer_size()];
        reader.next_frame(&mut buf)
            .map_err(|e| GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: format!("PNG frame read failed: {}", e),
            }))?;
        
        let info = reader.info();
        // Calculate stride: width * bytes_per_pixel (4 for RGBA)
        let stride = info.width * 4;

        Ok(Frame {
            data: buf,
            width: info.width,
            height: info.height,
            stride,
            pixel_format: PixelFormat::RGBA,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        })
    }
    
    fn get_display_info(&self) -> Vec<super::DisplayInfo> {
        vec![super::DisplayInfo {
            id: 0,
            name: "Wayland Display".to_string(),
            x: 0,
            y: 0,
            width: self.width,
            height: self.height,
            is_primary: true,
        }]
    }
    
    fn select_display(&mut self, _display_id: u32) -> Result<()> {
        // Wayland typically captures all outputs
        // Individual display selection would require portal API
        Ok(())
    }
    
    fn set_capture_region(&mut self, _x: i32, _y: i32, _width: u32, _height: u32) -> Result<()> {
        // Region capture would require compositor support
        warn!("Region capture not yet implemented for Wayland");
        Ok(())
    }
    
    fn is_healthy(&self) -> bool {
        self.is_initialized
    }
    
    fn get_resolution(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    
    async fn cleanup(&mut self) -> Result<()> {
        info!("Cleaning up Wayland screen capturer");
        
        self.session_token = None;
        self.pipewire_node_id = None;
        self.is_initialized = false;
        
        // TODO: Properly close PipeWire connections and portal sessions
        
        Ok(())
    }
}

/// Helper to detect the Wayland compositor type
pub fn detect_compositor() -> String {
    // Check for common compositor environment variables
    if std::env::var("SWAYSOCK").is_ok() {
        return "sway".to_string();
    }
    
    if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
        return "hyprland".to_string();
    }
    
    if std::env::var("WAYFIRE_SOCKET").is_ok() {
        return "wayfire".to_string();
    }
    
    // Check XDG_CURRENT_DESKTOP
    if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
        return desktop.to_lowercase();
    }
    
    "unknown".to_string()
}