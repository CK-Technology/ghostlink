//! High-level Wayland screen capturer
//!
//! Combines Portal session management with PipeWire frame capture
//! into a simple, async-compatible interface.

use async_trait::async_trait;
use tracing::{info, warn};

use crate::error::{Result, GhostLinkError, CaptureError};
use crate::capture::{Frame, DisplayInfo, ScreenCapturer};

use super::portal::{ScreenCastPortal, PortalSession};
use super::pipewire::{PipeWireRecorder, PipeWireStream};
use super::{CompositorType, detect_compositor};

/// Wayland-native screen capturer using XDG Portal + PipeWire
pub struct WaylandPortalCapturer {
    /// Portal session (kept alive for the duration of capture)
    session: Option<PortalSession>,
    /// PipeWire recorders (one per stream/monitor)
    recorders: Vec<PipeWireRecorder>,
    /// Currently selected display index
    selected_display: usize,
    /// Cached display info
    displays: Vec<DisplayInfo>,
    /// Detected compositor type
    compositor: CompositorType,
    /// Whether to capture cursor
    capture_cursor: bool,
    /// Restore token for persistent permissions
    restore_token: Option<String>,
    /// Initialization state
    is_initialized: bool,
}

impl WaylandPortalCapturer {
    /// Create a new Wayland portal capturer
    pub async fn new() -> Result<Self> {
        // Verify we're running under Wayland
        if std::env::var("WAYLAND_DISPLAY").is_err() {
            return Err(GhostLinkError::Capture(CaptureError::UnsupportedPlatform {
                platform: "Not running under Wayland".into(),
            }));
        }

        let compositor = detect_compositor();
        info!("Detected Wayland compositor: {:?}", compositor);

        Ok(Self {
            session: None,
            recorders: Vec::new(),
            selected_display: 0,
            displays: Vec::new(),
            compositor,
            capture_cursor: true,
            restore_token: None,
            is_initialized: false,
        })
    }

    /// Set whether to capture the cursor
    pub fn set_capture_cursor(&mut self, capture: bool) {
        self.capture_cursor = capture;
    }

    /// Set a restore token for persistent permissions
    pub fn set_restore_token(&mut self, token: Option<String>) {
        self.restore_token = token;
    }

    /// Get the current restore token (if any)
    pub fn get_restore_token(&self) -> Option<&str> {
        self.restore_token.as_deref()
    }

    /// Initialize the portal session and create recorders
    async fn init_session(&mut self) -> Result<()> {
        info!("Initializing Wayland portal session");

        // Create portal connection
        let portal = ScreenCastPortal::new()?;

        // Request screen capture (this may show a permission dialog)
        let session = portal.request_screen_capture(
            self.capture_cursor,
            self.restore_token.as_deref(),
        )?;

        info!("Portal session established with {} streams", session.streams.len());

        // Build display info from streams
        self.displays = session.streams.iter().enumerate().map(|(i, stream)| {
            DisplayInfo {
                id: i as u32,
                name: format!("Display {} (PipeWire {})", i + 1, stream.path),
                width: stream.size.0,
                height: stream.size.1,
                x: stream.position.0,
                y: stream.position.1,
                is_primary: i == 0,
            }
        }).collect();

        // Create PipeWire recorders for each stream
        for stream in &session.streams {
            let pw_stream = PipeWireStream::from(stream);
            let recorder = PipeWireRecorder::new(&session.fd, &pw_stream)?;
            self.recorders.push(recorder);
        }

        self.session = Some(session);
        self.is_initialized = true;

        info!("Wayland capture initialized with {} displays", self.displays.len());
        Ok(())
    }
}

#[async_trait]
impl ScreenCapturer for WaylandPortalCapturer {
    async fn initialize(&mut self) -> Result<()> {
        if self.is_initialized {
            return Ok(());
        }
        self.init_session().await
    }

    async fn capture_frame(&mut self) -> Result<Frame> {
        if !self.is_initialized {
            return Err(GhostLinkError::Capture(CaptureError::NotInitialized));
        }

        if self.recorders.is_empty() {
            return Err(GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: "No recorders available".into(),
            }));
        }

        // Get the selected recorder
        let recorder = self.recorders.get_mut(self.selected_display)
            .ok_or_else(|| GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: format!("Invalid display index: {}", self.selected_display),
            }))?;

        // Capture with 100ms timeout
        match recorder.capture_frame(100)? {
            Some(captured) => Ok(captured.to_frame()),
            None => {
                // No new frame - return a placeholder or wait
                // For now, try again with longer timeout
                match recorder.capture_frame(500)? {
                    Some(captured) => Ok(captured.to_frame()),
                    None => Err(GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                        reason: "No frame available".into(),
                    })),
                }
            }
        }
    }

    fn get_display_info(&self) -> Vec<DisplayInfo> {
        self.displays.clone()
    }

    fn select_display(&mut self, display_id: u32) -> Result<()> {
        let idx = display_id as usize;
        if idx >= self.recorders.len() {
            return Err(GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: format!("Invalid display ID: {} (have {} displays)", display_id, self.recorders.len()),
            }));
        }
        self.selected_display = idx;
        info!("Selected display {}", display_id);
        Ok(())
    }

    fn set_capture_region(&mut self, _x: i32, _y: i32, _width: u32, _height: u32) -> Result<()> {
        // Region capture would require re-requesting portal with specific window
        warn!("Region capture not supported with portal API");
        Ok(())
    }

    fn get_resolution(&self) -> (u32, u32) {
        if let Some(recorder) = self.recorders.get(self.selected_display) {
            recorder.resolution()
        } else if let Some(display) = self.displays.get(self.selected_display) {
            (display.width, display.height)
        } else {
            (1920, 1080) // Fallback
        }
    }

    fn is_healthy(&self) -> bool {
        self.is_initialized && !self.recorders.is_empty()
    }

    async fn cleanup(&mut self) -> Result<()> {
        info!("Cleaning up Wayland portal capturer");

        // Drop recorders (stops GStreamer pipelines)
        self.recorders.clear();

        // Drop session (closes portal session)
        self.session = None;

        self.is_initialized = false;
        self.displays.clear();

        Ok(())
    }
}

/// Legacy wrapper for compatibility with existing WaylandCapturer interface
pub struct WaylandCapturer {
    inner: WaylandPortalCapturer,
}

impl WaylandCapturer {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            inner: WaylandPortalCapturer::new().await?,
        })
    }
}

#[async_trait]
impl ScreenCapturer for WaylandCapturer {
    async fn initialize(&mut self) -> Result<()> {
        self.inner.initialize().await
    }

    async fn capture_frame(&mut self) -> Result<Frame> {
        self.inner.capture_frame().await
    }

    fn get_display_info(&self) -> Vec<DisplayInfo> {
        self.inner.get_display_info()
    }

    fn select_display(&mut self, display_id: u32) -> Result<()> {
        self.inner.select_display(display_id)
    }

    fn set_capture_region(&mut self, x: i32, y: i32, width: u32, height: u32) -> Result<()> {
        self.inner.set_capture_region(x, y, width, height)
    }

    fn get_resolution(&self) -> (u32, u32) {
        self.inner.get_resolution()
    }

    fn is_healthy(&self) -> bool {
        self.inner.is_healthy()
    }

    async fn cleanup(&mut self) -> Result<()> {
        self.inner.cleanup().await
    }
}
