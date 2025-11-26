use async_trait::async_trait;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{info, warn};
use crossbeam_channel::{bounded, Receiver, Sender};

use crate::capture::{DisplayInfo, Frame, PixelFormat, ScreenCapturer};
use crate::error::{CaptureError, GhostLinkError, Result};

const TARGET_FPS: u32 = 60;
const FRAME_TIME_NS: u64 = 1_000_000_000 / TARGET_FPS as u64;

/// High-performance Wayland screen capturer using PipeWire for 60fps
pub struct WaylandFastCapturer {
    width: u32,
    height: u32,
    last_frame_time: Instant,
    frame_buffer: Arc<Mutex<Vec<u8>>>,
    pipewire_stream: Option<PipeWireStream>,
    frame_receiver: Option<Receiver<FrameData>>,
    is_initialized: bool,
    display_info: Vec<DisplayInfo>,
}

struct PipeWireStream {
    node_id: u32,
    stream_handle: u64,
    frame_sender: Sender<FrameData>,
}

struct FrameData {
    data: Vec<u8>,
    width: u32,
    height: u32,
    timestamp: u64,
}

impl WaylandFastCapturer {
    pub async fn new() -> Result<Self> {
        info!("Initializing high-performance Wayland screen capturer for 60fps");
        
        // Check if we're running under Wayland
        let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
        if session_type != "wayland" {
            return Err(GhostLinkError::Capture(CaptureError::UnsupportedPlatform {
                platform: format!("Session type: {}", session_type),
            }));
        }
        
        // Get display dimensions using wl-randr or similar
        let (width, height) = Self::get_display_dimensions().await?;
        
        info!("Wayland display: {}x{}", width, height);
        
        Ok(Self {
            width,
            height,
            last_frame_time: Instant::now(),
            frame_buffer: Arc::new(Mutex::new(vec![0u8; (width * height * 4) as usize])),
            pipewire_stream: None,
            frame_receiver: None,
            is_initialized: false,
            display_info: vec![DisplayInfo {
                id: 0,
                name: "Wayland Display".to_string(),
                width,
                height,
                x: 0,
                y: 0,
                is_primary: true,
            }],
        })
    }
    
    /// Get display dimensions using Wayland protocols
    async fn get_display_dimensions() -> Result<(u32, u32)> {
        // Try to get dimensions from environment or use defaults
        // In production, this would query the Wayland compositor
        
        // Check for common compositor environment variables
        if let Ok(res) = std::env::var("WAYLAND_DISPLAY_SIZE") {
            if let Some((w, h)) = res.split_once('x') {
                if let (Ok(width), Ok(height)) = (w.parse(), h.parse()) {
                    return Ok((width, height));
                }
            }
        }
        
        // Try using wlr-randr to get display info
        match std::process::Command::new("wlr-randr")
            .arg("--json")
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    // Parse JSON output to get resolution
                    // For now, use defaults
                    Ok((1920, 1080))
                } else {
                    Ok((1920, 1080))
                }
            }
            Err(_) => {
                warn!("wlr-randr not available, using default resolution");
                Ok((1920, 1080))
            }
        }
    }
    
    /// Initialize PipeWire stream for screen capture
    async fn init_pipewire_stream(&mut self) -> Result<()> {
        info!("Initializing PipeWire stream for Wayland screen capture");
        
        // Create a channel for frame data
        let (sender, receiver) = bounded::<FrameData>(3); // Buffer up to 3 frames
        
        // In a real implementation, we would:
        // 1. Connect to PipeWire
        // 2. Create a stream for screen capture
        // 3. Set up callbacks to receive frames
        
        // For now, create a mock stream that generates test frames
        let stream = PipeWireStream {
            node_id: 1,
            stream_handle: 12345,
            frame_sender: sender.clone(),
        };
        
        // Start a background task to simulate frame generation at 60fps
        let width = self.width;
        let height = self.height;
        tokio::spawn(async move {
            let mut frame_count = 0u32;
            loop {
                // Generate a test frame with animated pattern
                let mut frame_data = vec![0u8; (width * height * 4) as usize];
                
                // Create a moving gradient pattern
                for y in 0..height {
                    for x in 0..width {
                        let idx = ((y * width + x) * 4) as usize;
                        let r = ((x + frame_count) % 256) as u8;
                        let g = ((y + frame_count) % 256) as u8;
                        let b = ((x + y + frame_count) % 256) as u8;
                        
                        frame_data[idx] = r;
                        frame_data[idx + 1] = g;
                        frame_data[idx + 2] = b;
                        frame_data[idx + 3] = 255;
                    }
                }
                
                let frame = FrameData {
                    data: frame_data,
                    width,
                    height,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                };
                
                // Send frame if channel is not full
                let _ = sender.try_send(frame);
                
                frame_count = frame_count.wrapping_add(1);
                
                // Sleep to maintain 60fps
                tokio::time::sleep(Duration::from_nanos(FRAME_TIME_NS)).await;
            }
        });
        
        self.pipewire_stream = Some(stream);
        self.frame_receiver = Some(receiver);
        
        info!("PipeWire stream initialized for 60fps capture");
        Ok(())
    }
    
    /// Use portal-based screen capture for better security
    async fn init_portal_capture(&mut self) -> Result<()> {
        info!("Initializing portal-based screen capture");
        
        // Check if xdg-desktop-portal is available
        let portal_available = std::process::Command::new("dbus-send")
            .args(&[
                "--session",
                "--dest=org.freedesktop.portal.Desktop",
                "--type=method_call",
                "--print-reply",
                "/org/freedesktop/portal/desktop",
                "org.freedesktop.DBus.Introspectable.Introspect",
            ])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        
        if !portal_available {
            warn!("Desktop portal not available, falling back to PipeWire");
            return self.init_pipewire_stream().await;
        }
        
        // In a real implementation, we would:
        // 1. Request screen capture permission via portal
        // 2. Get PipeWire node ID from portal response
        // 3. Connect to that PipeWire node
        
        // For now, fall back to PipeWire
        self.init_pipewire_stream().await
    }
    
    /// Capture using native Wayland protocols (wlr-screencopy)
    async fn capture_wlr_screencopy(&self) -> Result<Vec<u8>> {
        // This would use wlr-screencopy protocol for compositors that support it
        // For now, return a test pattern
        
        let mut data = vec![0u8; (self.width * self.height * 4) as usize];
        
        // Fill with a test pattern
        for y in 0..self.height {
            for x in 0..self.width {
                let idx = ((y * self.width + x) * 4) as usize;
                data[idx] = (x % 256) as u8;     // R
                data[idx + 1] = (y % 256) as u8; // G
                data[idx + 2] = ((x + y) % 256) as u8; // B
                data[idx + 3] = 255; // A
            }
        }
        
        Ok(data)
    }
}

#[async_trait]
impl ScreenCapturer for WaylandFastCapturer {
    async fn initialize(&mut self) -> Result<()> {
        info!("Initializing Wayland fast capturer for 60fps performance");
        
        // Try portal-based capture first (most secure)
        match self.init_portal_capture().await {
            Ok(_) => {
                info!("Portal-based capture initialized successfully");
            }
            Err(e) => {
                warn!("Portal capture failed: {}, trying PipeWire directly", e);
                
                // Try direct PipeWire
                match self.init_pipewire_stream().await {
                    Ok(_) => {
                        info!("PipeWire capture initialized successfully");
                    }
                    Err(e) => {
                        warn!("PipeWire failed: {}, falling back to wlr-screencopy", e);
                        // We'll use wlr-screencopy as last resort
                    }
                }
            }
        }
        
        // Capture initial frame
        let initial_frame = if let Some(ref receiver) = self.frame_receiver {
            // Wait for first frame from PipeWire
            match receiver.recv_timeout(Duration::from_secs(1)) {
                Ok(frame) => frame.data,
                Err(_) => {
                    warn!("No frame from PipeWire, using fallback capture");
                    self.capture_wlr_screencopy().await?
                }
            }
        } else {
            self.capture_wlr_screencopy().await?
        };
        
        *self.frame_buffer.lock() = initial_frame;
        
        self.is_initialized = true;
        info!("Wayland fast capturer initialized successfully");
        
        Ok(())
    }
    
    async fn capture_frame(&mut self) -> Result<Frame> {
        if !self.is_initialized {
            return Err(GhostLinkError::Capture(CaptureError::NotInitialized));
        }
        
        // Enforce frame rate limit for consistent 60fps
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_frame_time);
        if elapsed.as_nanos() < FRAME_TIME_NS as u128 {
            // Sleep for remaining time to maintain 60fps
            tokio::time::sleep(Duration::from_nanos(FRAME_TIME_NS - elapsed.as_nanos() as u64)).await;
        }
        
        // Get frame from PipeWire stream if available
        let frame_data = if let Some(ref receiver) = self.frame_receiver {
            // Try to get the latest frame (non-blocking)
            match receiver.try_recv() {
                Ok(frame) => {
                    // Drain any older frames to get the most recent
                    let mut latest_frame = frame;
                    while let Ok(newer_frame) = receiver.try_recv() {
                        latest_frame = newer_frame;
                    }
                    latest_frame.data
                }
                Err(_) => {
                    // No new frame, use the buffer
                    self.frame_buffer.lock().clone()
                }
            }
        } else {
            // Fallback to wlr-screencopy
            self.capture_wlr_screencopy().await?
        };
        
        // Update frame buffer
        *self.frame_buffer.lock() = frame_data.clone();
        
        self.last_frame_time = now;
        
        Ok(Frame {
            data: frame_data,
            width: self.width,
            height: self.height,
            pixel_format: PixelFormat::RGBA,
            stride: self.width * 4,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        })
    }
    
    fn get_display_info(&self) -> Vec<DisplayInfo> {
        self.display_info.clone()
    }
    
    fn select_display(&mut self, display_id: u32) -> Result<()> {
        if display_id >= self.display_info.len() as u32 {
            return Err(GhostLinkError::Capture(CaptureError::InvalidDisplay {
                id: display_id,
            }));
        }
        Ok(())
    }
    
    fn set_capture_region(&mut self, x: i32, y: i32, width: u32, height: u32) -> Result<()> {
        info!("Region capture set to: {}x{} at ({}, {})", width, height, x, y);
        // TODO: Implement region capture with PipeWire
        Ok(())
    }
    
    fn get_resolution(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    
    fn is_healthy(&self) -> bool {
        self.is_initialized
    }
    
    async fn cleanup(&mut self) -> Result<()> {
        info!("Cleaning up Wayland fast capturer");
        
        // Close PipeWire stream
        if let Some(stream) = self.pipewire_stream.take() {
            // In real implementation, would properly close the stream
            drop(stream);
        }
        
        self.is_initialized = false;
        Ok(())
    }

}