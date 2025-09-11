use crate::capture::monitor_manager::{MonitorInfo, MonitorSelection, CaptureRegion, MonitorChangeEvent};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Monitor control messages for WebSocket communication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MonitorControlMessage {
    /// Get available monitors
    GetMonitors,
    
    /// Response with monitor information
    MonitorsResponse {
        monitors: HashMap<u32, MonitorInfo>,
        current_selection: MonitorSelection,
    },
    
    /// Select a specific monitor
    SelectMonitor {
        monitor_id: u32,
    },
    
    /// Enable/disable full desktop capture (all monitors)
    CaptureAllMonitors {
        enabled: bool,
    },
    
    /// Set custom capture region
    SetCaptureRegion {
        region: Option<CaptureRegion>,
    },
    
    /// Get current monitor selection
    GetSelection,
    
    /// Response with current selection
    SelectionResponse {
        selection: MonitorSelection,
    },
    
    /// Monitor configuration changed notification
    MonitorChanged {
        event: MonitorChangeEvent,
    },
    
    /// Set capture options
    SetCaptureOptions {
        follow_active_window: bool,
        capture_cursor: bool,
    },
    
    /// Response to monitor control commands
    ControlResponse {
        success: bool,
        error: Option<String>,
        data: Option<serde_json::Value>,
    },
}

/// Extended monitor information for web interface
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebMonitorInfo {
    /// Base monitor information
    #[serde(flatten)]
    pub monitor: MonitorInfo,
    
    /// Whether this monitor is currently selected
    pub selected: bool,
    
    /// Thumbnail image (base64 encoded)
    pub thumbnail: Option<String>,
    
    /// Monitor capabilities
    pub capabilities: MonitorCapabilities,
    
    /// Current capture statistics
    pub stats: Option<MonitorStats>,
}

/// Monitor capabilities for the web interface
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorCapabilities {
    /// Supports custom region capture
    pub supports_region_capture: bool,
    
    /// Supports high refresh rate capture
    pub supports_high_refresh_rate: bool,
    
    /// Maximum supported resolution
    pub max_resolution: (u32, u32),
    
    /// Minimum supported resolution
    pub min_resolution: (u32, u32),
    
    /// Supports HDR capture
    pub supports_hdr: bool,
    
    /// Supports variable refresh rate
    pub supports_vrr: bool,
}

/// Monitor capture statistics for web interface
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorStats {
    /// Current capture FPS
    pub current_fps: f32,
    
    /// Average capture FPS over last minute
    pub average_fps: f32,
    
    /// Dropped frames in last minute
    pub dropped_frames: u64,
    
    /// Current bandwidth usage (bytes/sec)
    pub bandwidth_usage: u64,
    
    /// Total frames captured in this session
    pub total_frames: u64,
    
    /// Capture latency in milliseconds
    pub capture_latency_ms: f32,
    
    /// Encoding latency in milliseconds
    pub encoding_latency_ms: f32,
}

/// Monitor control protocol handler
pub struct MonitorProtocolHandler {
    /// Session ID for this protocol handler
    session_id: String,
}

impl MonitorProtocolHandler {
    /// Create new monitor protocol handler
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
        }
    }
    
    /// Handle incoming monitor control message
    pub async fn handle_message(
        &self,
        message: MonitorControlMessage,
        monitor_manager: &crate::capture::monitor_manager::MonitorManager,
    ) -> Result<Option<MonitorControlMessage>, crate::error::GhostLinkError> {
        use crate::error::GhostLinkError;
        
        match message {
            MonitorControlMessage::GetMonitors => {
                let monitors = monitor_manager.get_monitors().await;
                let selection = monitor_manager.get_selection().await;
                
                Ok(Some(MonitorControlMessage::MonitorsResponse {
                    monitors,
                    current_selection: selection,
                }))
            }
            
            MonitorControlMessage::SelectMonitor { monitor_id } => {
                match monitor_manager.select_monitor(monitor_id).await {
                    Ok(()) => {
                        Ok(Some(MonitorControlMessage::ControlResponse {
                            success: true,
                            error: None,
                            data: Some(serde_json::json!({
                                "monitor_id": monitor_id,
                                "message": "Monitor selected successfully"
                            })),
                        }))
                    }
                    Err(e) => {
                        Ok(Some(MonitorControlMessage::ControlResponse {
                            success: false,
                            error: Some(format!("Failed to select monitor: {}", e)),
                            data: None,
                        }))
                    }
                }
            }
            
            MonitorControlMessage::CaptureAllMonitors { enabled } => {
                match monitor_manager.capture_all_monitors(enabled).await {
                    Ok(()) => {
                        Ok(Some(MonitorControlMessage::ControlResponse {
                            success: true,
                            error: None,
                            data: Some(serde_json::json!({
                                "capture_all": enabled,
                                "message": if enabled { 
                                    "All monitors capture enabled" 
                                } else { 
                                    "Single monitor capture enabled" 
                                }
                            })),
                        }))
                    }
                    Err(e) => {
                        Ok(Some(MonitorControlMessage::ControlResponse {
                            success: false,
                            error: Some(format!("Failed to set capture mode: {}", e)),
                            data: None,
                        }))
                    }
                }
            }
            
            MonitorControlMessage::SetCaptureRegion { region } => {
                match monitor_manager.set_custom_region(region.clone()).await {
                    Ok(()) => {
                        Ok(Some(MonitorControlMessage::ControlResponse {
                            success: true,
                            error: None,
                            data: Some(serde_json::json!({
                                "region": region,
                                "message": if region.is_some() { 
                                    "Custom capture region set" 
                                } else { 
                                    "Custom capture region cleared" 
                                }
                            })),
                        }))
                    }
                    Err(e) => {
                        Ok(Some(MonitorControlMessage::ControlResponse {
                            success: false,
                            error: Some(format!("Failed to set capture region: {}", e)),
                            data: None,
                        }))
                    }
                }
            }
            
            MonitorControlMessage::GetSelection => {
                let selection = monitor_manager.get_selection().await;
                Ok(Some(MonitorControlMessage::SelectionResponse { selection }))
            }
            
            MonitorControlMessage::SetCaptureOptions { 
                follow_active_window, 
                capture_cursor 
            } => {
                let mut selection = monitor_manager.get_selection().await;
                selection.follow_active_window = follow_active_window;
                selection.capture_cursor = capture_cursor;
                
                match monitor_manager.set_selection(selection).await {
                    Ok(()) => {
                        Ok(Some(MonitorControlMessage::ControlResponse {
                            success: true,
                            error: None,
                            data: Some(serde_json::json!({
                                "follow_active_window": follow_active_window,
                                "capture_cursor": capture_cursor,
                                "message": "Capture options updated"
                            })),
                        }))
                    }
                    Err(e) => {
                        Ok(Some(MonitorControlMessage::ControlResponse {
                            success: false,
                            error: Some(format!("Failed to update capture options: {}", e)),
                            data: None,
                        }))
                    }
                }
            }
            
            // These are response/notification messages, don't handle directly
            MonitorControlMessage::MonitorsResponse { .. } |
            MonitorControlMessage::SelectionResponse { .. } |
            MonitorControlMessage::MonitorChanged { .. } |
            MonitorControlMessage::ControlResponse { .. } => {
                Ok(None)
            }
        }
    }
    
    /// Convert monitor info to web-friendly format
    pub fn to_web_monitor_info(
        &self,
        monitor: &MonitorInfo,
        selected: bool,
        stats: Option<MonitorStats>,
    ) -> WebMonitorInfo {
        let capabilities = MonitorCapabilities {
            supports_region_capture: true,
            supports_high_refresh_rate: monitor.refresh_rate >= 120.0,
            max_resolution: (
                monitor.supported_resolutions
                    .iter()
                    .map(|r| r.width)
                    .max()
                    .unwrap_or(monitor.width),
                monitor.supported_resolutions
                    .iter()
                    .map(|r| r.height)
                    .max()
                    .unwrap_or(monitor.height)
            ),
            min_resolution: (
                monitor.supported_resolutions
                    .iter()
                    .map(|r| r.width)
                    .min()
                    .unwrap_or(monitor.width),
                monitor.supported_resolutions
                    .iter()
                    .map(|r| r.height)
                    .min()
                    .unwrap_or(monitor.height)
            ),
            supports_hdr: monitor.color_depth > 8,
            supports_vrr: monitor.supported_resolutions
                .iter()
                .any(|r| r.refresh_rates.len() > 1),
        };
        
        WebMonitorInfo {
            monitor: monitor.clone(),
            selected,
            thumbnail: None, // TODO: Generate monitor thumbnail
            capabilities,
            stats,
        }
    }
    
    /// Create monitor changed notification
    pub fn create_monitor_changed_notification(event: MonitorChangeEvent) -> MonitorControlMessage {
        MonitorControlMessage::MonitorChanged { event }
    }
    
    /// Create monitors response with extended information
    pub async fn create_extended_monitors_response(
        &self,
        monitor_manager: &crate::capture::monitor_manager::MonitorManager,
    ) -> Result<MonitorControlMessage, crate::error::GhostLinkError> {
        let monitors = monitor_manager.get_monitors().await;
        let selection = monitor_manager.get_selection().await;
        
        // Convert to web-friendly format
        let web_monitors: HashMap<u32, MonitorInfo> = monitors
            .into_iter()
            .map(|(id, monitor)| {
                let selected = !selection.capture_all_monitors && selection.monitor_id == id;
                (id, monitor)
            })
            .collect();
        
        Ok(MonitorControlMessage::MonitorsResponse {
            monitors: web_monitors,
            current_selection: selection,
        })
    }
}

/// Monitor control message serialization helpers
impl MonitorControlMessage {
    /// Serialize message to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
    
    /// Deserialize message from JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
    
    /// Get message type as string
    pub fn message_type(&self) -> &'static str {
        match self {
            MonitorControlMessage::GetMonitors => "GetMonitors",
            MonitorControlMessage::MonitorsResponse { .. } => "MonitorsResponse",
            MonitorControlMessage::SelectMonitor { .. } => "SelectMonitor",
            MonitorControlMessage::CaptureAllMonitors { .. } => "CaptureAllMonitors",
            MonitorControlMessage::SetCaptureRegion { .. } => "SetCaptureRegion",
            MonitorControlMessage::GetSelection => "GetSelection",
            MonitorControlMessage::SelectionResponse { .. } => "SelectionResponse",
            MonitorControlMessage::MonitorChanged { .. } => "MonitorChanged",
            MonitorControlMessage::SetCaptureOptions { .. } => "SetCaptureOptions",
            MonitorControlMessage::ControlResponse { .. } => "ControlResponse",
        }
    }
    
    /// Check if message is a request (needs response)
    pub fn is_request(&self) -> bool {
        matches!(self,
            MonitorControlMessage::GetMonitors |
            MonitorControlMessage::SelectMonitor { .. } |
            MonitorControlMessage::CaptureAllMonitors { .. } |
            MonitorControlMessage::SetCaptureRegion { .. } |
            MonitorControlMessage::GetSelection |
            MonitorControlMessage::SetCaptureOptions { .. }
        )
    }
    
    /// Check if message is a response
    pub fn is_response(&self) -> bool {
        matches!(self,
            MonitorControlMessage::MonitorsResponse { .. } |
            MonitorControlMessage::SelectionResponse { .. } |
            MonitorControlMessage::ControlResponse { .. }
        )
    }
    
    /// Check if message is a notification
    pub fn is_notification(&self) -> bool {
        matches!(self,
            MonitorControlMessage::MonitorChanged { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_monitor_control_message_serialization() {
        let message = MonitorControlMessage::SelectMonitor { monitor_id: 1 };
        let json = message.to_json().unwrap();
        let deserialized = MonitorControlMessage::from_json(&json).unwrap();
        
        match deserialized {
            MonitorControlMessage::SelectMonitor { monitor_id } => {
                assert_eq!(monitor_id, 1);
            }
            _ => panic!("Wrong message type"),
        }
    }
    
    #[test]
    fn test_message_type_classification() {
        let request = MonitorControlMessage::GetMonitors;
        assert!(request.is_request());
        assert!(!request.is_response());
        assert!(!request.is_notification());
        
        let response = MonitorControlMessage::ControlResponse {
            success: true,
            error: None,
            data: None,
        };
        assert!(!response.is_request());
        assert!(response.is_response());
        assert!(!response.is_notification());
    }
}