#![allow(dead_code)]

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn, trace};

use crate::{
    session::SessionType,
    error::Result,
};


#[cfg(target_os = "linux")]
pub mod wayland;
#[cfg(target_os = "linux")]
pub mod wayland_legacy;
#[cfg(target_os = "linux")]
pub mod x11;
#[cfg(target_os = "linux")]
pub mod x11_fast;
#[cfg(target_os = "linux")]
pub mod wayland_fast;

// Re-export the new Wayland capturer

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

pub mod encoding;
pub mod h264_encoder;
pub mod hevc_encoder;
pub mod nvenc_encoder;
pub mod encoder_factory;
pub mod frame_streaming;
pub mod frame_protocol;
pub mod monitor_manager;

/// Cross-platform screen capture abstraction
pub struct ScreenCapture {
    capturer: Arc<Mutex<ScreenCapturerEnum>>,
    encoder: Arc<RwLock<Option<VideoEncoderEnum>>>,
    is_streaming: Arc<RwLock<bool>>,
    session_type: SessionType,
    capture_task_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

/// Enum to hold different screen capturer implementations
pub enum ScreenCapturerEnum {
    #[cfg(target_os = "linux")]
    WaylandPortal(wayland::WaylandPortalCapturer),
    #[cfg(target_os = "linux")]
    Wayland(wayland::capturer::WaylandCapturer),
    #[cfg(target_os = "linux")]
    X11(x11::X11Capturer),
    #[cfg(target_os = "linux")]
    X11Fast(x11_fast::X11FastCapturer),
    #[cfg(target_os = "linux")]
    WaylandFast(wayland_fast::WaylandFastCapturer),
    #[cfg(target_os = "windows")]
    Windows(windows::DxgiCapturer),
    #[cfg(target_os = "macos")]
    MacOS(macos::CoreGraphicsCapturer),
}

#[async_trait]
impl ScreenCapturer for ScreenCapturerEnum {
    async fn initialize(&mut self) -> Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            Self::WaylandPortal(capturer) => capturer.initialize().await,
            #[cfg(target_os = "linux")]
            Self::Wayland(capturer) => capturer.initialize().await,
            #[cfg(target_os = "linux")]
            Self::X11(capturer) => capturer.initialize().await,
            #[cfg(target_os = "linux")]
            Self::X11Fast(capturer) => capturer.initialize().await,
            #[cfg(target_os = "linux")]
            Self::WaylandFast(capturer) => capturer.initialize().await,
            #[cfg(target_os = "windows")]
            Self::Windows(capturer) => capturer.initialize().await,
            #[cfg(target_os = "macos")]
            Self::MacOS(capturer) => capturer.initialize().await,
        }
    }

    async fn capture_frame(&mut self) -> Result<Frame> {
        match self {
            #[cfg(target_os = "linux")]
            Self::WaylandPortal(capturer) => capturer.capture_frame().await,
            #[cfg(target_os = "linux")]
            Self::Wayland(capturer) => capturer.capture_frame().await,
            #[cfg(target_os = "linux")]
            Self::X11(capturer) => capturer.capture_frame().await,
            #[cfg(target_os = "linux")]
            Self::X11Fast(capturer) => capturer.capture_frame().await,
            #[cfg(target_os = "linux")]
            Self::WaylandFast(capturer) => capturer.capture_frame().await,
            #[cfg(target_os = "windows")]
            Self::Windows(capturer) => capturer.capture_frame().await,
            #[cfg(target_os = "macos")]
            Self::MacOS(capturer) => capturer.capture_frame().await,
        }
    }

    fn get_display_info(&self) -> Vec<DisplayInfo> {
        match self {
            #[cfg(target_os = "linux")]
            Self::WaylandPortal(capturer) => capturer.get_display_info(),
            #[cfg(target_os = "linux")]
            Self::Wayland(capturer) => capturer.get_display_info(),
            #[cfg(target_os = "linux")]
            Self::X11(capturer) => capturer.get_display_info(),
            #[cfg(target_os = "linux")]
            Self::X11Fast(capturer) => capturer.get_display_info(),
            #[cfg(target_os = "linux")]
            Self::WaylandFast(capturer) => capturer.get_display_info(),
            #[cfg(target_os = "windows")]
            Self::Windows(capturer) => capturer.get_display_info(),
            #[cfg(target_os = "macos")]
            Self::MacOS(capturer) => capturer.get_display_info(),
        }
    }

    fn select_display(&mut self, display_id: u32) -> Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            Self::WaylandPortal(capturer) => capturer.select_display(display_id),
            #[cfg(target_os = "linux")]
            Self::Wayland(capturer) => capturer.select_display(display_id),
            #[cfg(target_os = "linux")]
            Self::X11(capturer) => capturer.select_display(display_id),
            #[cfg(target_os = "linux")]
            Self::X11Fast(capturer) => capturer.select_display(display_id),
            #[cfg(target_os = "linux")]
            Self::WaylandFast(capturer) => capturer.select_display(display_id),
            #[cfg(target_os = "windows")]
            Self::Windows(capturer) => capturer.select_display(display_id),
            #[cfg(target_os = "macos")]
            Self::MacOS(capturer) => capturer.select_display(display_id),
        }
    }

    fn set_capture_region(&mut self, x: i32, y: i32, width: u32, height: u32) -> Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            Self::WaylandPortal(capturer) => capturer.set_capture_region(x, y, width, height),
            #[cfg(target_os = "linux")]
            Self::Wayland(capturer) => capturer.set_capture_region(x, y, width, height),
            #[cfg(target_os = "linux")]
            Self::X11(capturer) => capturer.set_capture_region(x, y, width, height),
            #[cfg(target_os = "linux")]
            Self::X11Fast(capturer) => capturer.set_capture_region(x, y, width, height),
            #[cfg(target_os = "linux")]
            Self::WaylandFast(capturer) => capturer.set_capture_region(x, y, width, height),
            #[cfg(target_os = "windows")]
            Self::Windows(capturer) => capturer.set_capture_region(x, y, width, height),
            #[cfg(target_os = "macos")]
            Self::MacOS(capturer) => capturer.set_capture_region(x, y, width, height),
        }
    }

    fn get_resolution(&self) -> (u32, u32) {
        match self {
            #[cfg(target_os = "linux")]
            Self::WaylandPortal(capturer) => capturer.get_resolution(),
            #[cfg(target_os = "linux")]
            Self::Wayland(capturer) => capturer.get_resolution(),
            #[cfg(target_os = "linux")]
            Self::X11(capturer) => capturer.get_resolution(),
            #[cfg(target_os = "linux")]
            Self::X11Fast(capturer) => capturer.get_resolution(),
            #[cfg(target_os = "linux")]
            Self::WaylandFast(capturer) => capturer.get_resolution(),
            #[cfg(target_os = "windows")]
            Self::Windows(capturer) => capturer.get_resolution(),
            #[cfg(target_os = "macos")]
            Self::MacOS(capturer) => capturer.get_resolution(),
        }
    }

    fn is_healthy(&self) -> bool {
        match self {
            #[cfg(target_os = "linux")]
            Self::WaylandPortal(capturer) => capturer.is_healthy(),
            #[cfg(target_os = "linux")]
            Self::Wayland(capturer) => capturer.is_healthy(),
            #[cfg(target_os = "linux")]
            Self::X11(capturer) => capturer.is_healthy(),
            #[cfg(target_os = "linux")]
            Self::X11Fast(capturer) => capturer.is_healthy(),
            #[cfg(target_os = "linux")]
            Self::WaylandFast(capturer) => capturer.is_healthy(),
            #[cfg(target_os = "windows")]
            Self::Windows(capturer) => capturer.is_healthy(),
            #[cfg(target_os = "macos")]
            Self::MacOS(capturer) => capturer.is_healthy(),
        }
    }

    async fn cleanup(&mut self) -> Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            Self::WaylandPortal(capturer) => capturer.cleanup().await,
            #[cfg(target_os = "linux")]
            Self::Wayland(capturer) => capturer.cleanup().await,
            #[cfg(target_os = "linux")]
            Self::X11(capturer) => capturer.cleanup().await,
            #[cfg(target_os = "linux")]
            Self::X11Fast(capturer) => capturer.cleanup().await,
            #[cfg(target_os = "linux")]
            Self::WaylandFast(capturer) => capturer.cleanup().await,
            #[cfg(target_os = "windows")]
            Self::Windows(capturer) => capturer.cleanup().await,
            #[cfg(target_os = "macos")]
            Self::MacOS(capturer) => capturer.cleanup().await,
        }
    }
}

/// Enum to hold different video encoder implementations
pub enum VideoEncoderEnum {
    Software(encoding::SoftwareEncoder),
    H264(h264_encoder::H264Encoder),
    Hevc(hevc_encoder::HevcEncoder),
    #[cfg(feature = "nvenc")]
    NvencH264(nvenc_encoder::NvencEncoder),
    #[cfg(feature = "nvenc")]
    NvencH265(nvenc_encoder::NvencEncoder),
    #[cfg(feature = "nvenc")]
    NvencAV1(nvenc_encoder::NvencEncoder),
    #[cfg(feature = "qsv")]
    Qsv(encoding::QsvEncoder),
    #[cfg(feature = "videotoolbox")]
    VideoToolbox(encoding::VideoToolboxEncoder),
}

#[async_trait]
impl VideoEncoder for VideoEncoderEnum {
    async fn initialize(&mut self, width: u32, height: u32, fps: u32) -> Result<()> {
        match self {
            Self::Software(encoder) => encoder.initialize(width, height, fps).await,
            Self::H264(encoder) => encoder.initialize(width, height, fps).await,
            Self::Hevc(encoder) => encoder.initialize(width, height, fps).await,
            #[cfg(feature = "nvenc")]
            Self::NvencH264(encoder) => encoder.initialize(width, height, fps).await,
            #[cfg(feature = "nvenc")]
            Self::NvencH265(encoder) => encoder.initialize(width, height, fps).await,
            #[cfg(feature = "nvenc")]
            Self::NvencAV1(encoder) => encoder.initialize(width, height, fps).await,
            #[cfg(feature = "qsv")]
            Self::Qsv(encoder) => encoder.initialize(width, height, fps).await,
            #[cfg(feature = "videotoolbox")]
            Self::VideoToolbox(encoder) => encoder.initialize(width, height, fps).await,
        }
    }

    async fn encode_frame(&mut self, frame: &Frame) -> Result<Vec<u8>> {
        match self {
            Self::Software(encoder) => encoder.encode_frame(frame).await,
            Self::H264(encoder) => encoder.encode_frame(frame).await,
            Self::Hevc(encoder) => encoder.encode_frame(frame).await,
            #[cfg(feature = "nvenc")]
            Self::NvencH264(encoder) => encoder.encode_frame(frame).await,
            #[cfg(feature = "nvenc")]
            Self::NvencH265(encoder) => encoder.encode_frame(frame).await,
            #[cfg(feature = "nvenc")]
            Self::NvencAV1(encoder) => encoder.encode_frame(frame).await,
            #[cfg(feature = "qsv")]
            Self::Qsv(encoder) => encoder.encode_frame(frame).await,
            #[cfg(feature = "videotoolbox")]
            Self::VideoToolbox(encoder) => encoder.encode_frame(frame).await,
        }
    }

    fn get_encoder_info(&self) -> EncoderInfo {
        match self {
            Self::Software(encoder) => encoder.get_encoder_info(),
            Self::H264(encoder) => encoder.get_encoder_info(),
            Self::Hevc(encoder) => encoder.get_encoder_info(),
            #[cfg(feature = "nvenc")]
            Self::NvencH264(encoder) => encoder.get_encoder_info(),
            #[cfg(feature = "nvenc")]
            Self::NvencH265(encoder) => encoder.get_encoder_info(),
            #[cfg(feature = "nvenc")]
            Self::NvencAV1(encoder) => encoder.get_encoder_info(),
            #[cfg(feature = "qsv")]
            Self::Qsv(encoder) => encoder.get_encoder_info(),
            #[cfg(feature = "videotoolbox")]
            Self::VideoToolbox(encoder) => encoder.get_encoder_info(),
        }
    }

    fn is_healthy(&self) -> bool {
        match self {
            Self::Software(encoder) => encoder.is_healthy(),
            Self::H264(encoder) => encoder.is_healthy(),
            Self::Hevc(encoder) => encoder.is_healthy(),
            #[cfg(feature = "nvenc")]
            Self::NvencH264(encoder) => encoder.is_healthy(),
            #[cfg(feature = "nvenc")]
            Self::NvencH265(encoder) => encoder.is_healthy(),
            #[cfg(feature = "nvenc")]
            Self::NvencAV1(encoder) => encoder.is_healthy(),
            #[cfg(feature = "qsv")]
            Self::Qsv(encoder) => encoder.is_healthy(),
            #[cfg(feature = "videotoolbox")]
            Self::VideoToolbox(encoder) => encoder.is_healthy(),
        }
    }

    async fn cleanup(&mut self) -> Result<()> {
        match self {
            Self::Software(encoder) => encoder.cleanup().await,
            Self::H264(encoder) => encoder.cleanup().await,
            Self::Hevc(encoder) => encoder.cleanup().await,
            #[cfg(feature = "nvenc")]
            Self::NvencH264(encoder) => encoder.cleanup().await,
            #[cfg(feature = "nvenc")]
            Self::NvencH265(encoder) => encoder.cleanup().await,
            #[cfg(feature = "nvenc")]
            Self::NvencAV1(encoder) => encoder.cleanup().await,
            #[cfg(feature = "qsv")]
            Self::Qsv(encoder) => encoder.cleanup().await,
            #[cfg(feature = "videotoolbox")]
            Self::VideoToolbox(encoder) => encoder.cleanup().await,
        }
    }
}

/// Platform-specific screen capture implementation
#[async_trait::async_trait]
pub trait ScreenCapturer: Send + Sync {
    /// Initialize the capturer
    async fn initialize(&mut self) -> Result<()>;
    
    /// Capture a single frame
    async fn capture_frame(&mut self) -> Result<Frame>;
    
    /// Get available display information
    fn get_display_info(&self) -> Vec<DisplayInfo>;
    
    /// Set which display to capture (for multi-monitor)
    fn select_display(&mut self, display_id: u32) -> Result<()>;
    
    /// Set capture region
    fn set_capture_region(&mut self, x: i32, y: i32, width: u32, height: u32) -> Result<()>;
    
    /// Get current capture resolution
    fn get_resolution(&self) -> (u32, u32);
    
    /// Check if capturer is healthy
    fn is_healthy(&self) -> bool;
    
    /// Cleanup resources
    async fn cleanup(&mut self) -> Result<()>;
}

/// Video encoder trait for hardware-accelerated encoding
#[async_trait::async_trait]
pub trait VideoEncoder: Send + Sync {
    /// Initialize encoder with settings
    async fn initialize(&mut self, width: u32, height: u32, fps: u32) -> Result<()>;
    
    /// Encode a frame
    async fn encode_frame(&mut self, frame: &Frame) -> Result<Vec<u8>>;
    
    /// Get encoder info
    fn get_encoder_info(&self) -> EncoderInfo;
    
    /// Check if encoder is healthy
    fn is_healthy(&self) -> bool;
    
    /// Cleanup encoder resources
    async fn cleanup(&mut self) -> Result<()> {
        // Default implementation does nothing
        Ok(())
    }
}

/// Represents a captured screen frame
#[derive(Debug, Clone)]
pub struct Frame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
    pub stride: u32,
    pub timestamp: u64,
}

/// Pixel format for frame data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PixelFormat {
    RGBA,
    BGRA,
    RGB,
    BGR,
    YUV420,
    NV12,
}

/// Display/monitor information
#[derive(Debug, Clone)]
pub struct DisplayInfo {
    pub id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub is_primary: bool,
}

/// Video encoder information
#[derive(Debug, Clone)]
pub struct EncoderInfo {
    pub name: String,
    pub hardware_accelerated: bool,
    pub supported_formats: Vec<PixelFormat>,
    pub max_resolution: (u32, u32),
}

impl ScreenCapture {
    /// Create new screen capture instance
    pub async fn new(session_type: SessionType) -> Result<Self> {
        let capturer = Self::create_platform_capturer().await?;
        
        let mut screen_capture = Self {
            capturer: Arc::new(Mutex::new(capturer)),
            encoder: Arc::new(RwLock::new(None)),
            is_streaming: Arc::new(RwLock::new(false)),
            session_type,
            capture_task_handle: Arc::new(Mutex::new(None)),
        };
        
        screen_capture.initialize().await?;
        
        Ok(screen_capture)
    }

    /// Create platform-specific capturer
    async fn create_platform_capturer() -> Result<ScreenCapturerEnum> {
        #[cfg(target_os = "linux")]
        {
            // Check for capture method preference
            let use_portal = std::env::var("GHOSTLINK_USE_PORTAL")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or(true); // Default to Portal-based capture (Wayland-native)

            let use_fast_capture = std::env::var("GHOSTLINK_FAST_CAPTURE")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or(true);

            // Try Wayland first, fall back to X11
            if std::env::var("WAYLAND_DISPLAY").is_ok() {
                // Wayland-native: Use Portal + PipeWire (preferred)
                if use_portal {
                    info!("Using Wayland Portal + PipeWire capture (native, 60fps capable)");
                    match wayland::WaylandPortalCapturer::new().await {
                        Ok(capturer) => return Ok(ScreenCapturerEnum::WaylandPortal(capturer)),
                        Err(e) => {
                            warn!("Portal capture failed, falling back: {}", e);
                        }
                    }
                }

                // Fallback to legacy Wayland methods
                info!("Using legacy Wayland screen capture");
                if use_fast_capture {
                    Ok(ScreenCapturerEnum::WaylandFast(wayland_fast::WaylandFastCapturer::new().await?))
                } else {
                    Ok(ScreenCapturerEnum::Wayland(wayland::capturer::WaylandCapturer::new().await?))
                }
            } else {
                info!("Using X11 screen capture (60fps capable)");
                if use_fast_capture {
                    Ok(ScreenCapturerEnum::X11Fast(x11_fast::X11FastCapturer::new().await?))
                } else {
                    Ok(ScreenCapturerEnum::X11(x11::X11Capturer::new().await?))
                }
            }
        }
        
        #[cfg(target_os = "windows")]
        {
            info!("Using Windows DXGI screen capture");
            Ok(ScreenCapturerEnum::Windows(windows::DxgiCapturer::new().await?))
        }
        
        #[cfg(target_os = "macos")]
        {
            info!("Using macOS Core Graphics screen capture");
            Ok(ScreenCapturerEnum::MacOS(macos::CoreGraphicsCapturer::new().await?))
        }
        
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            Err(CaptureError::UnsupportedPlatform {
                platform: std::env::consts::OS.to_string(),
            }.into())
        }
    }

    /// Initialize screen capture
    async fn initialize(&mut self) -> Result<()> {
        info!("Initializing screen capture");
        
        {
            let mut capturer_guard = self.capturer.lock().await;
            capturer_guard.initialize().await?;
        }
        
        // Initialize encoder
        let (width, height) = {
            let capturer_guard = self.capturer.lock().await;
            capturer_guard.get_resolution()
        };
        let mut encoder = encoding::create_best_encoder().await?;
        encoder.initialize(width, height, 30).await?;
        
        let mut encoder_guard = self.encoder.write().await;
        *encoder_guard = Some(encoder);
        
        info!("Screen capture initialized: {}x{}", width, height);
        Ok(())
    }

    /// Start streaming screen capture
    pub async fn start_streaming(&self) -> Result<()> {
        let mut streaming_guard = self.is_streaming.write().await;
        
        if *streaming_guard {
            warn!("Screen capture already streaming");
            return Ok(());
        }
        
        *streaming_guard = true;
        
        // Start capture loop
        self.start_capture_loop().await?;
        
        info!("Screen capture streaming started");
        Ok(())
    }

    /// Start the capture loop in background
    async fn start_capture_loop(&self) -> Result<()> {
        let capturer = Arc::clone(&self.capturer);
        let encoder = Arc::clone(&self.encoder);
        let is_streaming = Arc::clone(&self.is_streaming);
        
        // Spawn capture loop task
        let handle = tokio::spawn(async move {
            info!("Capture loop started");
            let mut capture_interval = interval(Duration::from_millis(33)); // ~30 FPS
            
            while *is_streaming.read().await {
                capture_interval.tick().await;
                
                // Capture frame
                let frame_result = {
                    let mut capturer_guard = capturer.lock().await;
                    capturer_guard.capture_frame().await
                };
                
                match frame_result {
                    Ok(frame) => {
                        debug!("Captured frame: {}x{}", frame.width, frame.height);
                        
                        // Encode frame if encoder is available
                        if let Some(encoder) = encoder.write().await.as_mut() {
                            match encoder.encode_frame(&frame).await {
                                Ok(encoded_data) => {
                                    debug!("Frame encoded: {} bytes", encoded_data.len());
                                    // Send encoded data via websocket/relay
                                    if let Err(e) = Self::send_frame_to_relay(encoded_data).await {
                                        error!("Failed to send frame to relay: {}", e);
                                    }
                                },
                                Err(e) => {
                                    error!("Failed to encode frame: {}", e);
                                }
                            }
                        }
                    },
                    Err(e) => {
                        error!("Failed to capture frame: {}", e);
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
            
            info!("Capture loop stopped");
        });
        
        // Store task handle for cleanup
        let mut task_handle = self.capture_task_handle.lock().await;
        *task_handle = Some(handle);
        
        Ok(())
    }

    /// Stop streaming screen capture
    pub async fn stop_streaming(&self) -> Result<()> {
        {
            let mut streaming_guard = self.is_streaming.write().await;
            *streaming_guard = false;
        }
        
        // Wait for capture task to finish
        let mut task_handle = self.capture_task_handle.lock().await;
        if let Some(handle) = task_handle.take() {
            if let Err(e) = handle.await {
                warn!("Capture task ended with error: {}", e);
            }
        }
        
        info!("Screen capture streaming stopped");
        Ok(())
    }

    /// Get available displays
    pub async fn get_displays(&self) -> Result<Vec<DisplayInfo>> {
        let capturer_guard = self.capturer.lock().await;
        Ok(capturer_guard.get_display_info())
    }

    /// Set capture display (for multi-monitor)
    pub async fn set_display(&mut self, display_id: u32) -> Result<()> {
        info!("Setting capture display to: {}", display_id);
        let mut capturer_guard = self.capturer.lock().await;
        capturer_guard.select_display(display_id)
    }

    /// Get current resolution
    pub async fn get_resolution(&self) -> (u32, u32) {
        let capturer_guard = self.capturer.lock().await;
        capturer_guard.get_resolution()
    }

    /// Check if capture is healthy
    pub async fn is_healthy(&self) -> bool {
        let capturer_guard = self.capturer.lock().await;
        capturer_guard.is_healthy()
    }

    /// Get encoder information
    pub async fn get_encoder_info(&self) -> Option<EncoderInfo> {
        let encoder_guard = self.encoder.read().await;
        encoder_guard.as_ref().map(|e| e.get_encoder_info())
    }

    /// Send frame data to relay server
    async fn send_frame_to_relay(frame_data: Vec<u8>) -> Result<()> {
        // TODO: Integrate with actual WebSocket relay client
        // For now, this is a placeholder that logs the frame size
        trace!("Sending frame to relay: {} bytes", frame_data.len());
        
        // In a real implementation, this would:
        // 1. Get the WebSocket connection to relay server
        // 2. Create a FrameMessage with proper headers
        // 3. Send via WebSocket
        // 4. Handle connection failures and reconnection
        
        Ok(())
    }
}

impl ScreenCapturerEnum {
    /// Initialize the capturer
    pub async fn initialize(&mut self) -> Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::WaylandPortal(capturer) => capturer.initialize().await,
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::Wayland(capturer) => capturer.initialize().await,
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::X11(capturer) => capturer.initialize().await,
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::X11Fast(capturer) => capturer.initialize().await,
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::WaylandFast(capturer) => capturer.initialize().await,
            #[cfg(target_os = "windows")]
            ScreenCapturerEnum::Windows(capturer) => capturer.initialize().await,
            #[cfg(target_os = "macos")]
            ScreenCapturerEnum::MacOS(capturer) => capturer.initialize().await,
        }
    }

    /// Capture a single frame
    pub async fn capture_frame(&mut self) -> Result<Frame> {
        match self {
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::WaylandPortal(capturer) => capturer.capture_frame().await,
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::Wayland(capturer) => capturer.capture_frame().await,
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::X11(capturer) => capturer.capture_frame().await,
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::X11Fast(capturer) => capturer.capture_frame().await,
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::WaylandFast(capturer) => capturer.capture_frame().await,
            #[cfg(target_os = "windows")]
            ScreenCapturerEnum::Windows(capturer) => capturer.capture_frame().await,
            #[cfg(target_os = "macos")]
            ScreenCapturerEnum::MacOS(capturer) => capturer.capture_frame().await,
        }
    }

    /// Get available display information
    pub fn get_display_info(&self) -> Vec<DisplayInfo> {
        match self {
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::WaylandPortal(capturer) => capturer.get_display_info(),
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::Wayland(capturer) => capturer.get_display_info(),
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::X11(capturer) => capturer.get_display_info(),
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::X11Fast(capturer) => capturer.get_display_info(),
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::WaylandFast(capturer) => capturer.get_display_info(),
            #[cfg(target_os = "windows")]
            ScreenCapturerEnum::Windows(capturer) => capturer.get_display_info(),
            #[cfg(target_os = "macos")]
            ScreenCapturerEnum::MacOS(capturer) => capturer.get_display_info(),
        }
    }

    /// Set which display to capture (for multi-monitor)
    pub fn select_display(&mut self, display_id: u32) -> Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::WaylandPortal(capturer) => capturer.select_display(display_id),
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::Wayland(capturer) => capturer.select_display(display_id),
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::X11(capturer) => capturer.select_display(display_id),
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::X11Fast(capturer) => capturer.select_display(display_id),
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::WaylandFast(capturer) => capturer.select_display(display_id),
            #[cfg(target_os = "windows")]
            ScreenCapturerEnum::Windows(capturer) => capturer.select_display(display_id),
            #[cfg(target_os = "macos")]
            ScreenCapturerEnum::MacOS(capturer) => capturer.select_display(display_id),
        }
    }

    /// Set capture region
    pub fn set_capture_region(&mut self, x: i32, y: i32, width: u32, height: u32) -> Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::WaylandPortal(capturer) => capturer.set_capture_region(x, y, width, height),
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::Wayland(capturer) => capturer.set_capture_region(x, y, width, height),
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::X11(capturer) => capturer.set_capture_region(x, y, width, height),
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::X11Fast(capturer) => capturer.set_capture_region(x, y, width, height),
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::WaylandFast(capturer) => capturer.set_capture_region(x, y, width, height),
            #[cfg(target_os = "windows")]
            ScreenCapturerEnum::Windows(capturer) => capturer.set_capture_region(x, y, width, height),
            #[cfg(target_os = "macos")]
            ScreenCapturerEnum::MacOS(capturer) => capturer.set_capture_region(x, y, width, height),
        }
    }

    /// Get current capture resolution
    pub fn get_resolution(&self) -> (u32, u32) {
        match self {
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::WaylandPortal(capturer) => capturer.get_resolution(),
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::Wayland(capturer) => capturer.get_resolution(),
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::X11(capturer) => capturer.get_resolution(),
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::X11Fast(capturer) => capturer.get_resolution(),
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::WaylandFast(capturer) => capturer.get_resolution(),
            #[cfg(target_os = "windows")]
            ScreenCapturerEnum::Windows(capturer) => capturer.get_resolution(),
            #[cfg(target_os = "macos")]
            ScreenCapturerEnum::MacOS(capturer) => capturer.get_resolution(),
        }
    }

    /// Check if capturer is healthy
    pub fn is_healthy(&self) -> bool {
        match self {
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::WaylandPortal(capturer) => capturer.is_healthy(),
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::Wayland(capturer) => capturer.is_healthy(),
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::X11(capturer) => capturer.is_healthy(),
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::X11Fast(capturer) => capturer.is_healthy(),
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::WaylandFast(capturer) => capturer.is_healthy(),
            #[cfg(target_os = "windows")]
            ScreenCapturerEnum::Windows(capturer) => capturer.is_healthy(),
            #[cfg(target_os = "macos")]
            ScreenCapturerEnum::MacOS(capturer) => capturer.is_healthy(),
        }
    }
    
    /// Cleanup capturer resources
    pub async fn cleanup(&mut self) -> Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::WaylandPortal(capturer) => capturer.cleanup().await,
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::Wayland(capturer) => capturer.cleanup().await,
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::X11(capturer) => capturer.cleanup().await,
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::X11Fast(capturer) => capturer.cleanup().await,
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::WaylandFast(capturer) => capturer.cleanup().await,
            #[cfg(target_os = "windows")]
            ScreenCapturerEnum::Windows(capturer) => capturer.cleanup().await,
            #[cfg(target_os = "macos")]
            ScreenCapturerEnum::MacOS(capturer) => capturer.cleanup().await,
        }
    }
}

impl VideoEncoderEnum {
    /// Initialize encoder with settings
    pub async fn initialize(&mut self, width: u32, height: u32, fps: u32) -> Result<()> {
        match self {
            VideoEncoderEnum::Software(encoder) => encoder.initialize(width, height, fps).await,
            VideoEncoderEnum::H264(encoder) => encoder.initialize(width, height, fps).await,
            VideoEncoderEnum::Hevc(encoder) => encoder.initialize(width, height, fps).await,
            #[cfg(feature = "nvenc")]
            VideoEncoderEnum::NvencH264(encoder) => encoder.initialize(width, height, fps).await,
            #[cfg(feature = "nvenc")]
            VideoEncoderEnum::NvencH265(encoder) => encoder.initialize(width, height, fps).await,
            #[cfg(feature = "nvenc")]
            VideoEncoderEnum::NvencAV1(encoder) => encoder.initialize(width, height, fps).await,
            #[cfg(feature = "qsv")]
            VideoEncoderEnum::Qsv(encoder) => encoder.initialize(width, height, fps).await,
            #[cfg(feature = "videotoolbox")]
            VideoEncoderEnum::VideoToolbox(encoder) => encoder.initialize(width, height, fps).await,
        }
    }
    
    /// Encode a frame
    pub async fn encode_frame(&mut self, frame: &Frame) -> Result<Vec<u8>> {
        match self {
            VideoEncoderEnum::Software(encoder) => encoder.encode_frame(frame).await,
            VideoEncoderEnum::H264(encoder) => encoder.encode_frame(frame).await,
            VideoEncoderEnum::Hevc(encoder) => encoder.encode_frame(frame).await,
            #[cfg(feature = "nvenc")]
            VideoEncoderEnum::NvencH264(encoder) => encoder.encode_frame(frame).await,
            #[cfg(feature = "nvenc")]
            VideoEncoderEnum::NvencH265(encoder) => encoder.encode_frame(frame).await,
            #[cfg(feature = "nvenc")]
            VideoEncoderEnum::NvencAV1(encoder) => encoder.encode_frame(frame).await,
            #[cfg(feature = "qsv")]
            VideoEncoderEnum::Qsv(encoder) => encoder.encode_frame(frame).await,
            #[cfg(feature = "videotoolbox")]
            VideoEncoderEnum::VideoToolbox(encoder) => encoder.encode_frame(frame).await,
        }
    }
    
    /// Get encoder info
    pub fn get_encoder_info(&self) -> EncoderInfo {
        match self {
            VideoEncoderEnum::Software(encoder) => encoder.get_encoder_info(),
            VideoEncoderEnum::H264(encoder) => encoder.get_encoder_info(),
            VideoEncoderEnum::Hevc(encoder) => encoder.get_encoder_info(),
            #[cfg(feature = "nvenc")]
            VideoEncoderEnum::NvencH264(encoder) => encoder.get_encoder_info(),
            #[cfg(feature = "nvenc")]
            VideoEncoderEnum::NvencH265(encoder) => encoder.get_encoder_info(),
            #[cfg(feature = "nvenc")]
            VideoEncoderEnum::NvencAV1(encoder) => encoder.get_encoder_info(),
            #[cfg(feature = "qsv")]
            VideoEncoderEnum::Qsv(encoder) => encoder.get_encoder_info(),
            #[cfg(feature = "videotoolbox")]
            VideoEncoderEnum::VideoToolbox(encoder) => encoder.get_encoder_info(),
        }
    }
    
    /// Check if encoder is healthy
    pub fn is_healthy(&self) -> bool {
        match self {
            VideoEncoderEnum::Software(encoder) => encoder.is_healthy(),
            VideoEncoderEnum::H264(encoder) => encoder.is_healthy(),
            VideoEncoderEnum::Hevc(encoder) => encoder.is_healthy(),
            #[cfg(feature = "nvenc")]
            VideoEncoderEnum::NvencH264(encoder) => encoder.is_healthy(),
            #[cfg(feature = "nvenc")]
            VideoEncoderEnum::NvencH265(encoder) => encoder.is_healthy(),
            #[cfg(feature = "nvenc")]
            VideoEncoderEnum::NvencAV1(encoder) => encoder.is_healthy(),
            #[cfg(feature = "qsv")]
            VideoEncoderEnum::Qsv(encoder) => encoder.is_healthy(),
            #[cfg(feature = "videotoolbox")]
            VideoEncoderEnum::VideoToolbox(encoder) => encoder.is_healthy(),
        }
    }
}
