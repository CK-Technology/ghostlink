use crate::{
    error::{GhostLinkError, Result},
    capture::{
        x11_fast::X11FastCapturer,
        wayland_fast::WaylandFastCapturer,
        frame_protocol::{FrameMessage, VideoCodec},
    },
};

use std::{
    collections::HashMap,
    sync::{Arc, atomic::{AtomicU32, Ordering}},
    time::{Duration, SystemTime},
};
use tokio::sync::{RwLock, mpsc, broadcast};
use tracing::{debug, error, info, warn, trace};
use serde::{Deserialize, Serialize};

/// Monitor information for multi-monitor support
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MonitorInfo {
    /// Monitor ID (unique identifier)
    pub id: u32,
    /// Monitor name/description
    pub name: String,
    /// X coordinate of monitor
    pub x: i32,
    /// Y coordinate of monitor  
    pub y: i32,
    /// Monitor width in pixels
    pub width: u32,
    /// Monitor height in pixels
    pub height: u32,
    /// Refresh rate in Hz
    pub refresh_rate: f32,
    /// Scale factor (for HiDPI displays)
    pub scale_factor: f32,
    /// Whether this is the primary monitor
    pub is_primary: bool,
    /// Monitor manufacturer
    pub manufacturer: String,
    /// Monitor model
    pub model: String,
    /// Connection type (HDMI, DisplayPort, etc.)
    pub connection_type: String,
    /// Whether monitor is currently active
    pub is_active: bool,
    /// Color depth
    pub color_depth: u8,
    /// Available resolutions
    pub supported_resolutions: Vec<Resolution>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
    pub refresh_rates: Vec<f32>,
}

/// Monitor selection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorSelection {
    /// Selected monitor ID
    pub monitor_id: u32,
    /// Whether to capture full desktop (all monitors)
    pub capture_all_monitors: bool,
    /// Custom capture region
    pub custom_region: Option<CaptureRegion>,
    /// Follow active window
    pub follow_active_window: bool,
    /// Capture cursor
    pub capture_cursor: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureRegion {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Multi-monitor management system
pub struct MonitorManager {
    /// Currently detected monitors
    monitors: Arc<RwLock<HashMap<u32, MonitorInfo>>>,
    /// Currently selected monitor configuration
    selection: Arc<RwLock<MonitorSelection>>,
    /// Active capturer for current selection
    active_capturer: Arc<RwLock<Option<Box<dyn MultiMonitorCapturer + Send + Sync>>>>,
    /// Monitor change notification sender
    monitor_change_tx: broadcast::Sender<MonitorChangeEvent>,
    /// Monitor change notification receiver (for internal use)
    _monitor_change_rx: broadcast::Receiver<MonitorChangeEvent>,
    /// Current capture session ID
    session_id: AtomicU32,
    /// Whether we're running on Wayland
    is_wayland: bool,
}

#[derive(Debug, Clone)]
pub enum MonitorChangeEvent {
    MonitorAdded(MonitorInfo),
    MonitorRemoved(u32),
    MonitorChanged(MonitorInfo),
    SelectionChanged(MonitorSelection),
    ConfigurationChanged,
}

/// Trait for multi-monitor capable capturers
pub trait MultiMonitorCapturer {
    /// Start capturing from selected monitor/region
    async fn start_capture(&mut self, selection: &MonitorSelection) -> Result<()>;
    
    /// Stop capturing
    async fn stop_capture(&mut self) -> Result<()>;
    
    /// Capture current frame
    async fn capture_frame(&mut self) -> Result<Option<FrameMessage>>;
    
    /// Update capture selection
    async fn update_selection(&mut self, selection: &MonitorSelection) -> Result<()>;
    
    /// Get current capture statistics
    fn get_stats(&self) -> CaptureStats;
    
    /// Check if capturer supports monitor
    fn supports_monitor(&self, monitor: &MonitorInfo) -> bool;
}

#[derive(Debug, Clone, Default)]
pub struct CaptureStats {
    pub frames_captured: u64,
    pub bytes_captured: u64,
    pub average_fps: f32,
    pub capture_errors: u64,
    pub current_resolution: (u32, u32),
}

impl Default for MonitorSelection {
    fn default() -> Self {
        Self {
            monitor_id: 0,
            capture_all_monitors: false,
            custom_region: None,
            follow_active_window: false,
            capture_cursor: true,
        }
    }
}

impl MonitorManager {
    /// Create new monitor manager
    pub async fn new() -> Result<Self> {
        info!("Initializing monitor manager");
        
        let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok() || 
                        std::env::var("XDG_SESSION_TYPE").map(|s| s == "wayland").unwrap_or(false);
        
        info!("Detected display server: {}", if is_wayland { "Wayland" } else { "X11" });
        
        let (monitor_change_tx, monitor_change_rx) = broadcast::channel(100);
        
        let manager = Self {
            monitors: Arc::new(RwLock::new(HashMap::new())),
            selection: Arc::new(RwLock::new(MonitorSelection::default())),
            active_capturer: Arc::new(RwLock::new(None)),
            monitor_change_tx,
            _monitor_change_rx: monitor_change_rx,
            session_id: AtomicU32::new(0),
            is_wayland,
        };
        
        // Initial monitor detection
        manager.refresh_monitors().await?;
        
        // Start monitor detection task
        manager.start_monitor_detection().await;
        
        Ok(manager)
    }
    
    /// Get all detected monitors
    pub async fn get_monitors(&self) -> HashMap<u32, MonitorInfo> {
        self.monitors.read().await.clone()
    }
    
    /// Get primary monitor
    pub async fn get_primary_monitor(&self) -> Option<MonitorInfo> {
        let monitors = self.monitors.read().await;
        monitors.values().find(|m| m.is_primary).cloned()
    }
    
    /// Get current monitor selection
    pub async fn get_selection(&self) -> MonitorSelection {
        self.selection.read().await.clone()
    }
    
    /// Set monitor selection
    pub async fn set_selection(&self, selection: MonitorSelection) -> Result<()> {
        info!("Setting monitor selection: {:?}", selection);
        
        // Validate selection
        if !selection.capture_all_monitors {
            let monitors = self.monitors.read().await;
            if !monitors.contains_key(&selection.monitor_id) {
                return Err(GhostLinkError::Other(format!(
                    "Monitor {} not found", selection.monitor_id
                )));
            }
        }
        
        // Update selection
        *self.selection.write().await = selection.clone();
        
        // Update active capturer
        self.update_active_capturer().await?;
        
        // Notify listeners
        let _ = self.monitor_change_tx.send(MonitorChangeEvent::SelectionChanged(selection));
        
        Ok(())
    }
    
    /// Select monitor by ID
    pub async fn select_monitor(&self, monitor_id: u32) -> Result<()> {
        let mut selection = self.selection.read().await.clone();
        selection.monitor_id = monitor_id;
        selection.capture_all_monitors = false;
        
        self.set_selection(selection).await
    }
    
    /// Enable full desktop capture (all monitors)
    pub async fn capture_all_monitors(&self, enable: bool) -> Result<()> {
        let mut selection = self.selection.read().await.clone();
        selection.capture_all_monitors = enable;
        
        self.set_selection(selection).await
    }
    
    /// Set custom capture region
    pub async fn set_custom_region(&self, region: Option<CaptureRegion>) -> Result<()> {
        let mut selection = self.selection.read().await.clone();
        selection.custom_region = region;
        
        self.set_selection(selection).await
    }
    
    /// Start capturing from current selection
    pub async fn start_capture(&self) -> Result<mpsc::Receiver<FrameMessage>> {
        info!("Starting multi-monitor capture");
        
        let selection = self.selection.read().await.clone();
        let mut capturer = self.active_capturer.write().await;
        
        if let Some(ref mut cap) = *capturer {
            cap.start_capture(&selection).await?;
        } else {
            return Err(GhostLinkError::Other("No active capturer available".to_string()));
        }
        
        // Create frame channel
        let (frame_tx, frame_rx) = mpsc::channel(100);
        
        // Start capture loop
        let capturer_ref = Arc::clone(&self.active_capturer);
        let session_id = self.session_id.fetch_add(1, Ordering::Relaxed);
        
        tokio::spawn(async move {
            let mut capture_interval = tokio::time::interval(Duration::from_millis(16)); // ~60fps
            capture_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            loop {
                capture_interval.tick().await;
                
                let mut capturer_guard = capturer_ref.write().await;
                if let Some(ref mut capturer) = *capturer_guard {
                    match capturer.capture_frame().await {
                        Ok(Some(frame)) => {
                            if frame_tx.send(frame).await.is_err() {
                                debug!("Frame receiver closed, stopping capture loop {}", session_id);
                                break;
                            }
                        }
                        Ok(None) => {
                            // No frame available, continue
                            trace!("No frame captured");
                        }
                        Err(e) => {
                            error!("Capture error in session {}: {}", session_id, e);
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                    }
                } else {
                    warn!("No active capturer, stopping capture loop {}", session_id);
                    break;
                }
                drop(capturer_guard);
            }
        });
        
        Ok(frame_rx)
    }
    
    /// Stop capturing
    pub async fn stop_capture(&self) -> Result<()> {
        info!("Stopping multi-monitor capture");
        
        let mut capturer = self.active_capturer.write().await;
        if let Some(ref mut cap) = *capturer {
            cap.stop_capture().await?;
        }
        
        Ok(())
    }
    
    /// Get capture statistics
    pub async fn get_capture_stats(&self) -> Option<CaptureStats> {
        let capturer = self.active_capturer.read().await;
        capturer.as_ref().map(|cap| cap.get_stats())
    }
    
    /// Subscribe to monitor change events
    pub fn subscribe_monitor_changes(&self) -> broadcast::Receiver<MonitorChangeEvent> {
        self.monitor_change_tx.subscribe()
    }
    
    /// Refresh monitor list
    pub async fn refresh_monitors(&self) -> Result<()> {
        debug!("Refreshing monitor list");
        
        let detected_monitors = if self.is_wayland {
            self.detect_wayland_monitors().await?
        } else {
            self.detect_x11_monitors().await?
        };
        
        let mut monitors = self.monitors.write().await;
        
        // Check for changes
        let mut changes = Vec::new();
        for (id, monitor) in &detected_monitors {
            if let Some(existing) = monitors.get(id) {
                if existing != monitor {
                    changes.push(MonitorChangeEvent::MonitorChanged(monitor.clone()));
                }
            } else {
                changes.push(MonitorChangeEvent::MonitorAdded(monitor.clone()));
            }
        }
        
        // Check for removed monitors
        for id in monitors.keys() {
            if !detected_monitors.contains_key(id) {
                changes.push(MonitorChangeEvent::MonitorRemoved(*id));
            }
        }
        
        // Update monitor list
        *monitors = detected_monitors;
        
        // Send notifications
        for change in changes {
            let _ = self.monitor_change_tx.send(change);
        }
        
        info!("Detected {} monitors", monitors.len());
        for (id, monitor) in monitors.iter() {
            info!("Monitor {}: {} {}x{}@{:.1}Hz {}", 
                id, monitor.name, monitor.width, monitor.height, 
                monitor.refresh_rate, if monitor.is_primary { "(primary)" } else { "" });
        }
        
        Ok(())
    }
    
    /// Detect monitors on X11
    async fn detect_x11_monitors(&self) -> Result<HashMap<u32, MonitorInfo>> {
        use x11rb::{
            connection::Connection,
            protocol::{randr, xproto},
            rust_connection::RustConnection,
        };
        
        let (conn, screen_num) = RustConnection::connect(None)
            .map_err(|e| GhostLinkError::Other(format!("Failed to connect to X11: {}", e)))?;
        
        let screen = &conn.setup().roots[screen_num];
        let root = screen.root;
        
        // Get screen resources
        let resources = randr::get_screen_resources(&conn, root)
            .map_err(|e| GhostLinkError::Other(format!("Failed to get screen resources: {}", e)))?
            .reply()
            .map_err(|e| GhostLinkError::Other(format!("Failed to get screen resources reply: {}", e)))?;
        
        let mut monitors = HashMap::new();
        let mut monitor_id = 0u32;
        
        // Get information for each output
        for &output in &resources.outputs {
            let output_info = randr::get_output_info(&conn, output, resources.config_timestamp)
                .map_err(|e| GhostLinkError::Other(format!("Failed to get output info: {}", e)))?
                .reply()
                .map_err(|e| GhostLinkError::Other(format!("Failed to get output info reply: {}", e)))?;
            
            if output_info.connection != randr::Connection::CONNECTED {
                continue;
            }
            
            if let Some(crtc) = output_info.crtc.checked_sub(0).and_then(|_| Some(output_info.crtc)) {
                let crtc_info = randr::get_crtc_info(&conn, crtc, resources.config_timestamp)
                    .map_err(|e| GhostLinkError::Other(format!("Failed to get CRTC info: {}", e)))?
                    .reply()
                    .map_err(|e| GhostLinkError::Other(format!("Failed to get CRTC info reply: {}", e)))?;
                
                if crtc_info.width == 0 || crtc_info.height == 0 {
                    continue;
                }
                
                let name = String::from_utf8_lossy(&output_info.name).to_string();
                let is_primary = monitor_id == 0; // Simple primary detection
                
                let monitor = MonitorInfo {
                    id: monitor_id,
                    name: name.clone(),
                    x: crtc_info.x as i32,
                    y: crtc_info.y as i32,
                    width: crtc_info.width as u32,
                    height: crtc_info.height as u32,
                    refresh_rate: 60.0, // Default, could be calculated from mode info
                    scale_factor: 1.0,
                    is_primary,
                    manufacturer: "Unknown".to_string(),
                    model: name,
                    connection_type: "Unknown".to_string(),
                    is_active: true,
                    color_depth: 24,
                    supported_resolutions: vec![Resolution {
                        width: crtc_info.width as u32,
                        height: crtc_info.height as u32,
                        refresh_rates: vec![60.0],
                    }],
                };
                
                monitors.insert(monitor_id, monitor);
                monitor_id += 1;
            }
        }
        
        // If no monitors detected, add a default one
        if monitors.is_empty() {
            let default_monitor = MonitorInfo {
                id: 0,
                name: "Default Display".to_string(),
                x: 0,
                y: 0,
                width: screen.width_in_pixels as u32,
                height: screen.height_in_pixels as u32,
                refresh_rate: 60.0,
                scale_factor: 1.0,
                is_primary: true,
                manufacturer: "Unknown".to_string(),
                model: "Default".to_string(),
                connection_type: "Unknown".to_string(),
                is_active: true,
                color_depth: 24,
                supported_resolutions: vec![Resolution {
                    width: screen.width_in_pixels as u32,
                    height: screen.height_in_pixels as u32,
                    refresh_rates: vec![60.0],
                }],
            };
            monitors.insert(0, default_monitor);
        }
        
        Ok(monitors)
    }
    
    /// Detect monitors on Wayland
    async fn detect_wayland_monitors(&self) -> Result<HashMap<u32, MonitorInfo>> {
        // For Wayland, we'll use a simplified approach since PipeWire handles most of this
        // In a full implementation, you'd use wayland protocols to enumerate outputs
        
        let mut monitors = HashMap::new();
        
        // Add a default monitor for now
        let default_monitor = MonitorInfo {
            id: 0,
            name: "Wayland Display".to_string(),
            x: 0,
            y: 0,
            width: 1920,  // Default resolution, should be detected from portal
            height: 1080,
            refresh_rate: 60.0,
            scale_factor: 1.0,
            is_primary: true,
            manufacturer: "Unknown".to_string(),
            model: "Wayland".to_string(),
            connection_type: "Wayland".to_string(),
            is_active: true,
            color_depth: 24,
            supported_resolutions: vec![
                Resolution {
                    width: 1920,
                    height: 1080,
                    refresh_rates: vec![60.0, 144.0],
                },
                Resolution {
                    width: 2560,
                    height: 1440,
                    refresh_rates: vec![60.0, 144.0],
                },
            ],
        };
        
        monitors.insert(0, default_monitor);
        
        // TODO: Implement proper Wayland monitor detection using wayland-protocols
        // This would involve:
        // 1. Connect to Wayland compositor
        // 2. Bind to wl_output interface
        // 3. Listen for output events
        // 4. Get output modes and properties
        
        Ok(monitors)
    }
    
    /// Start background monitor detection task
    async fn start_monitor_detection(&self) {
        let monitors = Arc::clone(&self.monitors);
        let tx = self.monitor_change_tx.clone();
        let is_wayland = self.is_wayland;
        
        tokio::spawn(async move {
            let mut detection_interval = tokio::time::interval(Duration::from_secs(5));
            detection_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            loop {
                detection_interval.tick().await;
                
                // Detect current monitors
                let current_monitors = if is_wayland {
                    Self::detect_wayland_monitors_static().await
                } else {
                    Self::detect_x11_monitors_static().await
                };
                
                if let Ok(detected) = current_monitors {
                    let mut monitors_guard = monitors.write().await;
                    
                    // Check for changes
                    let mut has_changes = false;
                    
                    if monitors_guard.len() != detected.len() {
                        has_changes = true;
                    } else {
                        for (id, monitor) in &detected {
                            if let Some(existing) = monitors_guard.get(id) {
                                if existing != monitor {
                                    has_changes = true;
                                    break;
                                }
                            } else {
                                has_changes = true;
                                break;
                            }
                        }
                    }
                    
                    if has_changes {
                        *monitors_guard = detected;
                        let _ = tx.send(MonitorChangeEvent::ConfigurationChanged);
                        debug!("Monitor configuration changed");
                    }
                }
            }
        });
    }
    
    /// Static version of X11 monitor detection for background task
    async fn detect_x11_monitors_static() -> Result<HashMap<u32, MonitorInfo>> {
        // Simplified version for background detection
        // In practice, you'd reuse the logic from detect_x11_monitors
        Ok(HashMap::new())
    }
    
    /// Static version of Wayland monitor detection for background task
    async fn detect_wayland_monitors_static() -> Result<HashMap<u32, MonitorInfo>> {
        // Simplified version for background detection
        Ok(HashMap::new())
    }
    
    /// Update active capturer based on current selection
    async fn update_active_capturer(&self) -> Result<()> {
        let selection = self.selection.read().await.clone();
        let monitors = self.monitors.read().await;
        
        // Determine which capturer to use
        let new_capturer: Box<dyn MultiMonitorCapturer + Send + Sync> = if self.is_wayland {
            Box::new(WaylandMultiMonitorCapturer::new(&monitors, &selection).await?)
        } else {
            Box::new(X11MultiMonitorCapturer::new(&monitors, &selection).await?)
        };
        
        let mut capturer = self.active_capturer.write().await;
        *capturer = Some(new_capturer);
        
        Ok(())
    }
}

/// X11 multi-monitor capturer implementation
pub struct X11MultiMonitorCapturer {
    capturer: X11FastCapturer,
    selection: MonitorSelection,
    stats: CaptureStats,
}

impl X11MultiMonitorCapturer {
    pub async fn new(monitors: &HashMap<u32, MonitorInfo>, selection: &MonitorSelection) -> Result<Self> {
        let capturer = X11FastCapturer::new().await
            .map_err(|e| GhostLinkError::Other(format!("Failed to create X11 capturer: {}", e)))?;
        
        Ok(Self {
            capturer,
            selection: selection.clone(),
            stats: CaptureStats::default(),
        })
    }
}

impl MultiMonitorCapturer for X11MultiMonitorCapturer {
    async fn start_capture(&mut self, selection: &MonitorSelection) -> Result<()> {
        self.selection = selection.clone();
        self.capturer.start_capture(60).await
    }
    
    async fn stop_capture(&mut self) -> Result<()> {
        self.capturer.stop_capture().await
    }
    
    async fn capture_frame(&mut self) -> Result<Option<FrameMessage>> {
        let frame = self.capturer.capture_frame().await?;
        if frame.is_some() {
            self.stats.frames_captured += 1;
        }
        Ok(frame)
    }
    
    async fn update_selection(&mut self, selection: &MonitorSelection) -> Result<()> {
        self.selection = selection.clone();
        // TODO: Update capture region based on selection
        Ok(())
    }
    
    fn get_stats(&self) -> CaptureStats {
        self.stats.clone()
    }
    
    fn supports_monitor(&self, _monitor: &MonitorInfo) -> bool {
        true // X11 supports all monitors
    }
}

/// Wayland multi-monitor capturer implementation
pub struct WaylandMultiMonitorCapturer {
    capturer: WaylandFastCapturer,
    selection: MonitorSelection,
    stats: CaptureStats,
}

impl WaylandMultiMonitorCapturer {
    pub async fn new(monitors: &HashMap<u32, MonitorInfo>, selection: &MonitorSelection) -> Result<Self> {
        let capturer = WaylandFastCapturer::new().await
            .map_err(|e| GhostLinkError::Other(format!("Failed to create Wayland capturer: {}", e)))?;
        
        Ok(Self {
            capturer,
            selection: selection.clone(),
            stats: CaptureStats::default(),
        })
    }
}

impl MultiMonitorCapturer for WaylandMultiMonitorCapturer {
    async fn start_capture(&mut self, selection: &MonitorSelection) -> Result<()> {
        self.selection = selection.clone();
        self.capturer.start_capture(60).await
    }
    
    async fn stop_capture(&mut self) -> Result<()> {
        self.capturer.stop_capture().await
    }
    
    async fn capture_frame(&mut self) -> Result<Option<FrameMessage>> {
        let frame = self.capturer.capture_frame().await?;
        if frame.is_some() {
            self.stats.frames_captured += 1;
        }
        Ok(frame)
    }
    
    async fn update_selection(&mut self, selection: &MonitorSelection) -> Result<()> {
        self.selection = selection.clone();
        // TODO: Update capture region based on selection  
        Ok(())
    }
    
    fn get_stats(&self) -> CaptureStats {
        self.stats.clone()
    }
    
    fn supports_monitor(&self, _monitor: &MonitorInfo) -> bool {
        true // Wayland supports all monitors through portal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_monitor_manager_creation() {
        let manager = MonitorManager::new().await.unwrap();
        let monitors = manager.get_monitors().await;
        
        // Should have at least one monitor (default)
        assert!(!monitors.is_empty());
    }
    
    #[tokio::test]
    async fn test_monitor_selection() {
        let manager = MonitorManager::new().await.unwrap();
        let monitors = manager.get_monitors().await;
        
        if let Some((&id, _)) = monitors.iter().next() {
            manager.select_monitor(id).await.unwrap();
            let selection = manager.get_selection().await;
            assert_eq!(selection.monitor_id, id);
            assert!(!selection.capture_all_monitors);
        }
    }
    
    #[tokio::test]
    async fn test_capture_all_monitors() {
        let manager = MonitorManager::new().await.unwrap();
        
        manager.capture_all_monitors(true).await.unwrap();
        let selection = manager.get_selection().await;
        assert!(selection.capture_all_monitors);
    }
}