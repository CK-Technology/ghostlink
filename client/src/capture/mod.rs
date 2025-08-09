use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::session::SessionType;

#[cfg(target_os = "linux")]
pub mod wayland;
#[cfg(target_os = "linux")]
pub mod x11;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

pub mod encoding;

/// Cross-platform screen capture abstraction
pub struct ScreenCapture {
    capturer: ScreenCapturerEnum,
    encoder: Arc<RwLock<Option<VideoEncoderEnum>>>,
    is_streaming: Arc<RwLock<bool>>,
    session_type: SessionType,
}

/// Enum to hold different screen capturer implementations
pub enum ScreenCapturerEnum {
    #[cfg(target_os = "linux")]
    Wayland(wayland::WaylandCapturer),
    #[cfg(target_os = "linux")]
    X11(x11::X11Capturer),
    #[cfg(target_os = "windows")]
    Windows(windows::DxgiCapturer),
    #[cfg(target_os = "macos")]
    MacOS(macos::CoreGraphicsCapturer),
}

/// Enum to hold different video encoder implementations
pub enum VideoEncoderEnum {
    Software(encoding::SoftwareEncoder),
    #[cfg(feature = "nvenc")]
    Nvenc(encoding::NvencEncoder),
    #[cfg(feature = "qsv")]
    Qsv(encoding::QsvEncoder),
    #[cfg(feature = "videotoolbox")]
    VideoToolbox(encoding::VideoToolboxEncoder),
}

/// Platform-specific screen capture implementation
#[async_trait::async_trait]
pub trait ScreenCapturer: Send + Sync {
    /// Initialize the capturer
    async fn initialize(&mut self) -> Result<()>;
    
    /// Capture a single frame
    async fn capture_frame(&self) -> Result<Frame>;
    
    /// Get available displays/monitors
    async fn get_displays(&self) -> Result<Vec<Display>>;
    
    /// Set which display to capture (for multi-monitor)
    async fn set_display(&mut self, display_id: u32) -> Result<()>;
    
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
    async fn encode_frame(&self, frame: &Frame) -> Result<Vec<u8>>;
    
    /// Get encoder info
    fn get_encoder_info(&self) -> EncoderInfo;
    
    /// Check if encoder is healthy
    fn is_healthy(&self) -> bool;
}

/// Represents a captured screen frame
#[derive(Debug, Clone)]
pub struct Frame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
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
pub struct Display {
    pub id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub is_primary: bool,
    pub scale_factor: f32,
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
            capturer,
            encoder: Arc::new(RwLock::new(None)),
            is_streaming: Arc::new(RwLock::new(false)),
            session_type,
        };
        
        screen_capture.initialize().await?;
        
        Ok(screen_capture)
    }

    /// Create platform-specific capturer
    async fn create_platform_capturer() -> Result<ScreenCapturerEnum> {
        #[cfg(target_os = "linux")]
        {
            // Try Wayland first, fall back to X11
            if std::env::var("WAYLAND_DISPLAY").is_ok() {
                info!("Using Wayland screen capture");
                Ok(ScreenCapturerEnum::Wayland(wayland::WaylandCapturer::new().await?))
            } else {
                info!("Using X11 screen capture");
                Ok(ScreenCapturerEnum::X11(x11::X11Capturer::new().await?))
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
            Err(anyhow::anyhow!("Unsupported platform for screen capture"))
        }
    }

    /// Initialize screen capture
    async fn initialize(&mut self) -> Result<()> {
        info!("Initializing screen capture");
        
        self.capturer.initialize().await?;
        
        // Initialize encoder
        let (width, height) = self.capturer.get_resolution();
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
        // We need to move the capturer into the task, so we'll need to restructure this
        // For now, let's comment this out and implement a different approach
        
        let is_streaming = Arc::clone(&self.is_streaming);
        let encoder = Arc::clone(&self.encoder);
        
        // TODO: Implement a better approach where the capturer can be shared
        // or where the capture loop is managed differently
        
        info!("Capture loop start requested - implementation needed");
        
        // Set streaming to true for now
        *is_streaming.write().await = true;
        
        Ok(())
    }

    /// Stop streaming screen capture
    pub async fn stop_streaming(&self) -> Result<()> {
        let mut streaming_guard = self.is_streaming.write().await;
        *streaming_guard = false;
        
        info!("Screen capture streaming stopped");
        Ok(())
    }

    /// Get available displays
    pub async fn get_displays(&self) -> Result<Vec<Display>> {
        self.capturer.get_displays().await
    }

    /// Set capture display (for multi-monitor)
    pub async fn set_display(&mut self, display_id: u32) -> Result<()> {
        info!("Setting capture display to: {}", display_id);
        self.capturer.set_display(display_id).await
    }

    /// Get current resolution
    pub fn get_resolution(&self) -> (u32, u32) {
        self.capturer.get_resolution()
    }

    /// Check if capture is healthy
    pub fn is_healthy(&self) -> bool {
        self.capturer.is_healthy()
    }

    /// Get encoder information
    pub async fn get_encoder_info(&self) -> Option<EncoderInfo> {
        let encoder_guard = self.encoder.read().await;
        encoder_guard.as_ref().map(|e| e.get_encoder_info())
    }
}

impl ScreenCapturerEnum {
    /// Initialize the capturer
    pub async fn initialize(&mut self) -> Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::Wayland(ref mut capturer) => capturer.initialize().await,
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::X11(ref mut capturer) => capturer.initialize().await,
            #[cfg(target_os = "windows")]
            ScreenCapturerEnum::Windows(ref mut capturer) => capturer.initialize().await,
            #[cfg(target_os = "macos")]
            ScreenCapturerEnum::MacOS(ref mut capturer) => capturer.initialize().await,
        }
    }
    
    /// Capture a single frame
    pub async fn capture_frame(&self) -> Result<Frame> {
        match self {
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::Wayland(capturer) => capturer.capture_frame().await,
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::X11(capturer) => capturer.capture_frame().await,
            #[cfg(target_os = "windows")]
            ScreenCapturerEnum::Windows(capturer) => capturer.capture_frame().await,
            #[cfg(target_os = "macos")]
            ScreenCapturerEnum::MacOS(capturer) => capturer.capture_frame().await,
        }
    }
    
    /// Get available displays/monitors
    pub async fn get_displays(&self) -> Result<Vec<Display>> {
        match self {
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::Wayland(capturer) => capturer.get_displays().await,
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::X11(capturer) => capturer.get_displays().await,
            #[cfg(target_os = "windows")]
            ScreenCapturerEnum::Windows(capturer) => capturer.get_displays().await,
            #[cfg(target_os = "macos")]
            ScreenCapturerEnum::MacOS(capturer) => capturer.get_displays().await,
        }
    }
    
    /// Set which display to capture (for multi-monitor)
    pub async fn set_display(&mut self, display_id: u32) -> Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::Wayland(ref mut capturer) => capturer.set_display(display_id).await,
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::X11(ref mut capturer) => capturer.set_display(display_id).await,
            #[cfg(target_os = "windows")]
            ScreenCapturerEnum::Windows(ref mut capturer) => capturer.set_display(display_id).await,
            #[cfg(target_os = "macos")]
            ScreenCapturerEnum::MacOS(ref mut capturer) => capturer.set_display(display_id).await,
        }
    }
    
    /// Get current capture resolution
    pub fn get_resolution(&self) -> (u32, u32) {
        match self {
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::Wayland(capturer) => capturer.get_resolution(),
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::X11(capturer) => capturer.get_resolution(),
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
            ScreenCapturerEnum::Wayland(capturer) => capturer.is_healthy(),
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::X11(capturer) => capturer.is_healthy(),
            #[cfg(target_os = "windows")]
            ScreenCapturerEnum::Windows(capturer) => capturer.is_healthy(),
            #[cfg(target_os = "macos")]
            ScreenCapturerEnum::MacOS(capturer) => capturer.is_healthy(),
        }
    }
}

impl VideoEncoderEnum {
    /// Initialize encoder with settings
    pub async fn initialize(&mut self, width: u32, height: u32, fps: u32) -> Result<()> {
        match self {
            VideoEncoderEnum::Software(ref mut encoder) => encoder.initialize(width, height, fps).await,
            #[cfg(feature = "nvenc")]
            VideoEncoderEnum::Nvenc(ref mut encoder) => encoder.initialize(width, height, fps).await,
            #[cfg(feature = "qsv")]
            VideoEncoderEnum::Qsv(ref mut encoder) => encoder.initialize(width, height, fps).await,
            #[cfg(feature = "videotoolbox")]
            VideoEncoderEnum::VideoToolbox(ref mut encoder) => encoder.initialize(width, height, fps).await,
        }
    }
    
    /// Encode a frame
    pub async fn encode_frame(&self, frame: &Frame) -> Result<Vec<u8>> {
        match self {
            VideoEncoderEnum::Software(encoder) => encoder.encode_frame(frame).await,
            #[cfg(feature = "nvenc")]
            VideoEncoderEnum::Nvenc(encoder) => encoder.encode_frame(frame).await,
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
            #[cfg(feature = "nvenc")]
            VideoEncoderEnum::Nvenc(encoder) => encoder.get_encoder_info(),
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
            #[cfg(feature = "nvenc")]
            VideoEncoderEnum::Nvenc(encoder) => encoder.is_healthy(),
            #[cfg(feature = "qsv")]
            VideoEncoderEnum::Qsv(encoder) => encoder.is_healthy(),
            #[cfg(feature = "videotoolbox")]
            VideoEncoderEnum::VideoToolbox(encoder) => encoder.is_healthy(),
        }
    }
}
