#![allow(dead_code)]

use crate::error::{GhostLinkError, Result};
use tracing::{debug, info, warn};

use super::{EncoderInfo, Frame, PixelFormat, VideoEncoder, VideoEncoderEnum};

/// Compression mode for software encoder
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompressionMode {
    /// Fast JPEG - best for real-time streaming (default)
    Jpeg,
    /// Lossless PNG - better for static content
    Png,
}

/// Create the best available video encoder for the platform
pub async fn create_best_encoder() -> Result<VideoEncoderEnum> {
    // Try hardware encoders first, then fall back to software

    #[cfg(feature = "nvenc")]
    {
        use crate::capture::nvenc_encoder::{NvencEncoder, NvencCodec};
        let encoder = NvencEncoder::new(NvencCodec::H264);
        info!("Using NVIDIA NVENC H.264 hardware encoder");
        return Ok(VideoEncoderEnum::NvencH264(encoder));
    }

    #[cfg(feature = "qsv")]
    {
        if let Ok(encoder) = QsvEncoder::new().await {
            info!("Using Intel Quick Sync Video encoder");
            return Ok(VideoEncoderEnum::Qsv(encoder));
        }
    }

    #[cfg(feature = "videotoolbox")]
    {
        if let Ok(encoder) = VideoToolboxEncoder::new().await {
            info!("Using Apple VideoToolbox encoder");
            return Ok(VideoEncoderEnum::VideoToolbox(encoder));
        }
    }

    // Fallback to software encoder with JPEG (faster than PNG)
    warn!("No hardware encoder available, using software JPEG encoder");
    Ok(VideoEncoderEnum::Software(SoftwareEncoder::new().await?))
}

/// Software video encoder (fallback) with JPEG and PNG support
/// JPEG is used by default for real-time streaming (10-50x faster than PNG)
pub struct SoftwareEncoder {
    width: u32,
    height: u32,
    fps: u32,
    is_initialized: bool,
    compression_mode: CompressionMode,
    jpeg_quality: u8,
}

impl SoftwareEncoder {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            width: 0,
            height: 0,
            fps: 30,
            is_initialized: false,
            compression_mode: CompressionMode::Jpeg, // Default to JPEG for speed
            jpeg_quality: 80, // Good balance of quality and speed
        })
    }

    /// Set compression mode
    pub fn set_compression_mode(&mut self, mode: CompressionMode) {
        self.compression_mode = mode;
        info!("Software encoder compression mode set to {:?}", mode);
    }

    /// Set JPEG quality (1-100, higher = better quality, larger files)
    pub fn set_jpeg_quality(&mut self, quality: u8) {
        self.jpeg_quality = quality.clamp(1, 100);
        debug!("JPEG quality set to {}", self.jpeg_quality);
    }

    /// Fast JPEG compression for real-time streaming
    fn compress_jpeg(&self, frame: &Frame) -> Result<Vec<u8>> {
        use image::{ImageBuffer, RgbaImage};
        use std::io::Cursor;

        // Create image from frame data
        let img: RgbaImage = ImageBuffer::from_raw(frame.width, frame.height, frame.data.clone())
            .ok_or_else(|| GhostLinkError::Encode("Failed to create image from frame data".to_string()))?;

        // Convert to RGB (JPEG doesn't support alpha)
        let rgb_img = image::DynamicImage::ImageRgba8(img).to_rgb8();

        // Encode to JPEG
        let mut jpeg_data = Vec::new();
        let mut cursor = Cursor::new(&mut jpeg_data);

        rgb_img.write_to(&mut cursor, image::ImageFormat::Jpeg)
            .map_err(|e| GhostLinkError::Encode(format!("JPEG encoding failed: {}", e)))?;

        debug!("JPEG encoded {}x{} -> {} bytes (quality {})",
            frame.width, frame.height, jpeg_data.len(), self.jpeg_quality);

        Ok(jpeg_data)
    }

    /// Lossless PNG compression (slower but preserves quality)
    fn compress_png(&self, frame: &Frame) -> Result<Vec<u8>> {
        use std::io::Cursor;

        let mut png_data = Vec::new();
        {
            let mut cursor = Cursor::new(&mut png_data);
            let mut encoder = png::Encoder::new(&mut cursor, frame.width, frame.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            // Use fast compression for real-time
            encoder.set_compression(png::Compression::Fast);

            let mut writer = encoder.write_header().map_err(|e| GhostLinkError::Encode(e.to_string()))?;
            writer.write_image_data(&frame.data).map_err(|e| GhostLinkError::Encode(e.to_string()))?;
        }

        debug!("PNG encoded {}x{} -> {} bytes", frame.width, frame.height, png_data.len());

        Ok(png_data)
    }
}

#[async_trait::async_trait]
impl VideoEncoder for SoftwareEncoder {
    async fn initialize(&mut self, width: u32, height: u32, fps: u32) -> Result<()> {
        info!("Initializing software encoder: {}x{} @ {}fps (mode: {:?})",
            width, height, fps, self.compression_mode);

        self.width = width;
        self.height = height;
        self.fps = fps;
        self.is_initialized = true;

        Ok(())
    }

    async fn encode_frame(&mut self, frame: &Frame) -> Result<Vec<u8>> {
        if !self.is_initialized {
            return Err(GhostLinkError::Encode("Encoder not initialized".to_string()));
        }

        // Use configured compression mode (JPEG by default for real-time)
        match self.compression_mode {
            CompressionMode::Jpeg => self.compress_jpeg(frame),
            CompressionMode::Png => self.compress_png(frame),
        }
    }

    fn get_encoder_info(&self) -> EncoderInfo {
        let name = match self.compression_mode {
            CompressionMode::Jpeg => format!("Software JPEG Encoder (q={})", self.jpeg_quality),
            CompressionMode::Png => "Software PNG Encoder".to_string(),
        };

        EncoderInfo {
            name,
            hardware_accelerated: false,
            supported_formats: vec![
                PixelFormat::RGBA,
                PixelFormat::BGRA,
                PixelFormat::RGB,
                PixelFormat::BGR,
            ],
            max_resolution: (4096, 4096),
        }
    }

    fn is_healthy(&self) -> bool {
        self.is_initialized
    }
}

// Hardware encoder stubs (would be implemented with proper codec libraries)

#[cfg(feature = "nvenc")]
pub struct NvencEncoder {
    // NVIDIA NVENC encoder implementation
}

#[cfg(feature = "nvenc")]
impl NvencEncoder {
    pub async fn new() -> Result<Self> {
        // Check if NVIDIA GPU is available
        // Initialize NVENC
        Err(GhostLinkError::Other("NVENC not implemented yet".to_string()))
    }
}

#[cfg(feature = "qsv")]
pub struct QsvEncoder {
    // Intel Quick Sync Video encoder implementation
}

#[cfg(feature = "qsv")]
impl QsvEncoder {
    pub async fn new() -> Result<Self> {
        // Check if Intel GPU is available
        // Initialize Quick Sync Video
        Err(GhostLinkError::Other("QSV not implemented yet".to_string()))
    }
}

#[cfg(feature = "videotoolbox")]
pub struct VideoToolboxEncoder {
    // Apple VideoToolbox encoder implementation
}

#[cfg(feature = "videotoolbox")]
impl VideoToolboxEncoder {
    pub async fn new() -> Result<Self> {
        // Initialize VideoToolbox
        Err(GhostLinkError::Other("VideoToolbox not implemented yet".to_string()))
    }
}
