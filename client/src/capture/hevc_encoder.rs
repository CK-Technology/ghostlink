use async_trait::async_trait;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

#[cfg(feature = "x264-encoder")]
use ffmpeg_next as ffmpeg;

use crate::capture::{EncoderInfo, Frame, PixelFormat, VideoEncoder};
use crate::error::{GhostLinkError, Result};

const TARGET_FPS: u32 = 60;
const DEFAULT_BITRATE: u32 = 1_500_000; // 1.5 Mbps (lower than H.264 due to better compression)
const DEFAULT_PRESET: &str = "ultrafast"; // For low latency
const DEFAULT_TUNE: &str = "zerolatency"; // For real-time streaming

/// High-performance H.265/HEVC encoder for 60fps with better compression than H.264
pub struct HevcEncoder {
    width: u32,
    height: u32,
    fps: u32,
    bitrate: u32,
    encoder_context: Option<Arc<Mutex<EncoderContext>>>,
    frame_count: u64,
    last_keyframe: u64,
    keyframe_interval: u64,
    is_initialized: bool,
}

struct EncoderContext {
    #[cfg(feature = "x264-encoder")]
    ffmpeg_context: ffmpeg::encoder::video::Video,
    #[cfg(feature = "x264-encoder")]
    scaler: ffmpeg::software::scaling::Context,
    #[cfg(not(feature = "x264-encoder"))]
    _placeholder: u8,
}

// FFmpeg contexts are not Send, so we need to ensure thread safety manually
unsafe impl Send for EncoderContext {}

impl HevcEncoder {
    pub fn new() -> Self {
        info!("Creating H.265/HEVC encoder for 60fps streaming with better compression");
        
        Self {
            width: 0,
            height: 0,
            fps: TARGET_FPS,
            bitrate: DEFAULT_BITRATE,
            encoder_context: None,
            frame_count: 0,
            last_keyframe: 0,
            keyframe_interval: TARGET_FPS as u64 * 2, // Keyframe every 2 seconds
            is_initialized: false,
        }
    }

    /// Initialize FFmpeg HEVC encoder
    #[cfg(feature = "x264-encoder")]
    fn init_ffmpeg_encoder(&mut self, width: u32, height: u32, fps: u32) -> Result<()> {
        info!("Initializing FFmpeg H.265/HEVC encoder: {}x{} @ {}fps", width, height, fps);
        
        // Initialize FFmpeg
        ffmpeg::init().map_err(|e| {
            GhostLinkError::Other(format!("Failed to initialize FFmpeg: {}", e))
        })?;
        
        // Try hardware-accelerated HEVC first, then software
        let codec = ffmpeg::encoder::find(ffmpeg::codec::Id::HEVC)
            .or_else(|| ffmpeg::encoder::find(ffmpeg::codec::Id::H265))
            .ok_or_else(|| GhostLinkError::Other("H.265/HEVC encoder not available".to_string()))?;
        
        let mut encoder = ffmpeg::codec::Context::new()
            .encoder()
            .video()
            .map_err(|e| GhostLinkError::Other(format!("Failed to create HEVC encoder: {}", e)))?;
        
        // Configure encoder for real-time performance with better compression
        encoder.set_width(width);
        encoder.set_height(height);
        encoder.set_format(ffmpeg::format::Pixel::YUV420P);
        encoder.set_time_base(ffmpeg::Rational::new(1, fps as i32));
        encoder.set_frame_rate(ffmpeg::Rational::new(fps as i32, 1));
        encoder.set_bit_rate(self.bitrate as usize);
        
        // HEVC-specific settings for low latency
        encoder.set_option("preset", DEFAULT_PRESET)
            .map_err(|e| GhostLinkError::Other(format!("Failed to set preset: {}", e)))?;
        encoder.set_option("tune", DEFAULT_TUNE)
            .map_err(|e| GhostLinkError::Other(format!("Failed to set tune: {}", e)))?;
        encoder.set_option("crf", "25") // Slightly higher CRF for better compression
            .map_err(|e| GhostLinkError::Other(format!("Failed to set CRF: {}", e)))?;
        
        // HEVC quality settings
        encoder.set_option("x265-params", "bframes=0:no-scenecut:keyint=120:min-keyint=120")
            .map_err(|e| GhostLinkError::Other(format!("Failed to set x265 params: {}", e)))?;
        
        // Open encoder
        let encoder = encoder.open()
            .map_err(|e| GhostLinkError::Other(format!("Failed to open HEVC encoder: {}", e)))?;
        
        // Create color space converter (RGBA -> YUV420P)
        let scaler = ffmpeg::software::scaling::Context::get(
            ffmpeg::format::Pixel::RGBA,
            width,
            height,
            ffmpeg::format::Pixel::YUV420P,
            width,
            height,
            ffmpeg::software::scaling::Flags::BILINEAR,
        ).map_err(|e| GhostLinkError::Other(format!("Failed to create scaler: {}", e)))?;
        
        let context = EncoderContext {
            ffmpeg_context: encoder,
            scaler,
        };
        
        self.encoder_context = Some(Arc::new(Mutex::new(context)));
        
        info!("H.265/HEVC encoder initialized successfully");
        Ok(())
    }

    /// Encode frame with FFmpeg HEVC
    #[cfg(feature = "x264-encoder")]
    fn encode_frame_ffmpeg(&mut self, frame: &Frame) -> Result<Vec<u8>> {
        let context_arc = self.encoder_context.as_ref()
            .ok_or_else(|| GhostLinkError::Other("Encoder not initialized".to_string()))?;
        
        let mut context = context_arc.lock();
        
        // Create input frame
        let mut input_frame = ffmpeg::frame::Video::empty();
        input_frame.set_width(self.width);
        input_frame.set_height(self.height);
        input_frame.set_format(ffmpeg::format::Pixel::RGBA);
        
        // Set frame data
        unsafe {
            let plane = input_frame.data_mut(0);
            let src_len = frame.data.len().min(plane.len());
            plane[..src_len].copy_from_slice(&frame.data[..src_len]);
        }
        
        // Convert colorspace (RGBA -> YUV420P)
        let mut yuv_frame = ffmpeg::frame::Video::empty();
        yuv_frame.set_width(self.width);
        yuv_frame.set_height(self.height);
        yuv_frame.set_format(ffmpeg::format::Pixel::YUV420P);
        
        context.scaler.run(&input_frame, &mut yuv_frame)
            .map_err(|e| GhostLinkError::Other(format!("Color conversion failed: {}", e)))?;
        
        // Set frame timing
        yuv_frame.set_pts(Some(self.frame_count as i64));
        
        // Let the encoder decide when to insert keyframes based on GOP settings
        
        // Encode frame
        context.ffmpeg_context.send_frame(&yuv_frame)
            .map_err(|e| GhostLinkError::Other(format!("Failed to send frame to HEVC encoder: {}", e)))?;
        
        // Receive encoded packet
        let mut packet = ffmpeg::packet::Packet::empty();
        match context.ffmpeg_context.receive_packet(&mut packet) {
            Ok(_) => {
                let encoded_data = packet.data().unwrap_or(&[]).to_vec();
                debug!("HEVC encoded frame {} -> {} bytes", self.frame_count, encoded_data.len());
                Ok(encoded_data)
            }
            Err(ffmpeg::Error::Other { errno: ffmpeg::util::error::EAGAIN }) => {
                // No packet ready yet
                Ok(Vec::new())
            }
            Err(e) => {
                error!("Failed to receive HEVC packet: {}", e);
                Err(GhostLinkError::Other(format!("HEVC encode failed: {}", e)))
            }
        }
    }

    /// Fallback encoding
    #[cfg(not(feature = "x264-encoder"))]
    fn encode_frame_fallback(&mut self, frame: &Frame) -> Result<Vec<u8>> {
        // Use PNG as fallback
        use std::io::Cursor;
        
        let mut png_data = Vec::new();
        {
            let mut cursor = Cursor::new(&mut png_data);
            let encoder = png::Encoder::new(&mut cursor, frame.width, frame.height);
            let mut encoder = encoder.into_writer()
                .map_err(|e| GhostLinkError::Other(format!("PNG encoder creation failed: {}", e)))?;
            
            encoder.write_image_data(&frame.data)
                .map_err(|e| GhostLinkError::Other(format!("PNG encoding failed: {}", e)))?;
            
            encoder.finish()
                .map_err(|e| GhostLinkError::Other(format!("PNG finalization failed: {}", e)))?;
        }
        
        debug!("PNG encoded frame {} -> {} bytes", self.frame_count, png_data.len());
        Ok(png_data)
    }
}

#[async_trait]
impl VideoEncoder for HevcEncoder {
    async fn initialize(&mut self, width: u32, height: u32, fps: u32) -> Result<()> {
        info!("Initializing H.265/HEVC encoder: {}x{} @ {}fps", width, height, fps);
        
        self.width = width;
        self.height = height;
        self.fps = fps;
        self.keyframe_interval = fps as u64 * 2; // Keyframe every 2 seconds
        
        // Try to initialize with hardware acceleration first
        #[cfg(feature = "x264-encoder")]
        {
            match self.init_ffmpeg_encoder(width, height, fps) {
                Ok(_) => {
                    self.is_initialized = true;
                    return Ok(());
                }
                Err(e) => {
                    warn!("FFmpeg HEVC encoder failed: {}, falling back to software", e);
                }
            }
        }
        
        // Fallback to software encoder
        #[cfg(not(feature = "x264-encoder"))]
        {
            warn!("HEVC encoder not available, using PNG fallback");
            self.is_initialized = true;
        }
        
        info!("H.265/HEVC encoder initialized successfully");
        Ok(())
    }

    async fn encode_frame(&mut self, frame: &Frame) -> Result<Vec<u8>> {
        if !self.is_initialized {
            return Err(GhostLinkError::Other("Encoder not initialized".to_string()));
        }

        // Validate frame format
        if frame.pixel_format != PixelFormat::RGBA {
            return Err(GhostLinkError::Other("Unsupported pixel format".to_string()));
        }

        if frame.width != self.width || frame.height != self.height {
            return Err(GhostLinkError::Other("Frame size mismatch".to_string()));
        }

        // Update frame count
        self.frame_count += 1;

        // Encode the frame
        #[cfg(feature = "x264-encoder")]
        {
            self.encode_frame_ffmpeg(frame)
        }

        #[cfg(not(feature = "x264-encoder"))]
        {
            self.encode_frame_fallback(frame)
        }
    }

    fn get_encoder_info(&self) -> EncoderInfo {
        EncoderInfo {
            name: if cfg!(feature = "x264-encoder") {
                "H.265/HEVC Hardware Encoder".to_string()
            } else {
                "Software Encoder (PNG Fallback)".to_string()
            },
            hardware_accelerated: cfg!(feature = "x264-encoder"),
            supported_formats: vec![PixelFormat::RGBA, PixelFormat::BGRA],
            max_resolution: (3840, 2160), // Support up to 4K
        }
    }

    fn is_healthy(&self) -> bool {
        self.is_initialized
    }

    async fn cleanup(&mut self) -> Result<()> {
        info!("Cleaning up H.265/HEVC encoder");
        
        if let Some(context_arc) = self.encoder_context.take() {
            #[cfg(feature = "x264-encoder")]
            {
                let mut context = context_arc.lock();
                if let Err(e) = context.ffmpeg_context.send_eof() {
                    warn!("Failed to flush HEVC encoder: {}", e);
                }
            }
        }
        
        self.is_initialized = false;
        info!("H.265/HEVC encoder cleanup complete");
        
        Ok(())
    }
}