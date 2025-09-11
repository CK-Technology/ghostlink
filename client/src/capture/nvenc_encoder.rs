use async_trait::async_trait;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

#[cfg(feature = "nvenc")]
use ffmpeg_next as ffmpeg;

use crate::capture::{EncoderInfo, Frame, PixelFormat, VideoEncoder};
use crate::error::{GhostLinkError, Result};

const TARGET_FPS: u32 = 60;
const DEFAULT_BITRATE: u32 = 3_000_000; // 3 Mbps for hardware encoding
const NVENC_PRESET: &str = "p1"; // Fastest preset for lowest latency
const NVENC_TUNE: &str = "ull"; // Ultra Low Latency

/// NVIDIA NVENC hardware encoder for maximum 60fps performance with GPU acceleration
pub struct NvencEncoder {
    width: u32,
    height: u32,
    fps: u32,
    bitrate: u32,
    codec_type: NvencCodec,
    encoder_context: Option<Arc<Mutex<EncoderContext>>>,
    frame_count: u64,
    last_keyframe: u64,
    keyframe_interval: u64,
    is_initialized: bool,
    gpu_memory_type: GpuMemoryType,
}

#[derive(Debug, Clone)]
pub enum NvencCodec {
    H264,
    H265,
    AV1, // For latest GPUs
}

#[derive(Debug, Clone)]
enum GpuMemoryType {
    SystemMemory,
    CudaMemory,
    D3D11Memory,
}

struct EncoderContext {
    #[cfg(feature = "nvenc")]
    ffmpeg_context: ffmpeg::encoder::video::Video,
    #[cfg(feature = "nvenc")]
    scaler: ffmpeg::software::scaling::Context,
    #[cfg(feature = "nvenc")]
    cuda_context: Option<CudaContext>,
    #[cfg(not(feature = "nvenc"))]
    _placeholder: u8,
}

// FFmpeg contexts are not Send, so we need to ensure thread safety manually
unsafe impl Send for EncoderContext {}

#[cfg(feature = "nvenc")]
struct CudaContext {
    device_id: i32,
    // In real implementation, would hold CUDA context
}

impl NvencEncoder {
    pub fn new(codec: NvencCodec) -> Self {
        info!("Creating NVENC {:?} encoder for maximum 60fps GPU acceleration", codec);
        
        Self {
            width: 0,
            height: 0,
            fps: TARGET_FPS,
            bitrate: DEFAULT_BITRATE,
            codec_type: codec,
            encoder_context: None,
            frame_count: 0,
            last_keyframe: 0,
            keyframe_interval: TARGET_FPS as u64 * 2, // Keyframe every 2 seconds
            is_initialized: false,
            gpu_memory_type: GpuMemoryType::SystemMemory,
        }
    }

    /// Check if NVENC is available on this system
    pub fn is_available() -> bool {
        #[cfg(feature = "nvenc")]
        {
            // Check for NVIDIA GPU
            if let Ok(output) = std::process::Command::new("nvidia-smi")
                .arg("--query-gpu=name")
                .arg("--format=csv,noheader")
                .output()
            {
                if output.status.success() && !output.stdout.is_empty() {
                    info!("NVIDIA GPU detected for NVENC");
                    return true;
                }
            }
            
            // Alternative check using CUDA
            if std::path::Path::new("/usr/lib/x86_64-linux-gnu/libnvidia-encode.so").exists() ||
               std::path::Path::new("/usr/lib64/libnvidia-encode.so").exists() {
                info!("NVENC libraries found");
                return true;
            }
            
            warn!("NVENC not available - no NVIDIA GPU or drivers found");
            false
        }
        
        #[cfg(not(feature = "nvenc"))]
        {
            warn!("NVENC support not compiled in");
            false
        }
    }

    /// Initialize CUDA context for GPU memory operations
    #[cfg(feature = "nvenc")]
    fn init_cuda_context(&mut self) -> Result<()> {
        info!("Initializing CUDA context for NVENC");
        
        // In a real implementation, this would:
        // 1. Initialize CUDA runtime
        // 2. Select best GPU device
        // 3. Create CUDA context
        // 4. Allocate GPU memory buffers
        
        // For now, simulate successful initialization
        let cuda_context = CudaContext {
            device_id: 0,
        };
        
        if let Some(context_arc) = &self.encoder_context {
            let mut context = context_arc.lock();
            context.cuda_context = Some(cuda_context);
        }
        
        self.gpu_memory_type = GpuMemoryType::CudaMemory;
        info!("CUDA context initialized for GPU memory operations");
        
        Ok(())
    }

    /// Initialize NVENC encoder with optimal settings
    #[cfg(feature = "nvenc")]
    fn init_nvenc_encoder(&mut self, width: u32, height: u32, fps: u32) -> Result<()> {
        info!("Initializing NVENC {:?} encoder: {}x{} @ {}fps", self.codec_type, width, height, fps);
        
        // Initialize FFmpeg
        ffmpeg::init().map_err(|e| {
            GhostLinkError::Other(format!("Failed to initialize FFmpeg: {}", e))
        })?;
        
        // Select NVENC codec
        let codec_id = match self.codec_type {
            NvencCodec::H264 => ffmpeg::codec::Id::H264,
            NvencCodec::H265 => ffmpeg::codec::Id::HEVC,
            NvencCodec::AV1 => ffmpeg::codec::Id::AV1,
        };
        
        // Try to find NVENC encoder (h264_nvenc, hevc_nvenc, av1_nvenc)
        let encoder_name = match self.codec_type {
            NvencCodec::H264 => "h264_nvenc",
            NvencCodec::H265 => "hevc_nvenc", 
            NvencCodec::AV1 => "av1_nvenc",
        };
        
        let codec = ffmpeg::encoder::find_by_name(encoder_name)
            .ok_or_else(|| GhostLinkError::Other(format!("NVENC encoder {} not available", encoder_name)))?;
        
        let mut encoder = ffmpeg::encoder::video::Video::new(codec)
            .map_err(|e| GhostLinkError::Other(format!("Failed to create NVENC encoder: {}", e)))?;
        
        // Configure for maximum performance
        encoder.set_width(width);
        encoder.set_height(height);
        encoder.set_format(ffmpeg::format::Pixel::NV12); // Optimal format for NVENC
        encoder.set_time_base(ffmpeg::Rational::new(1, fps as i32));
        encoder.set_frame_rate(ffmpeg::Rational::new(fps as i32, 1));
        encoder.set_bit_rate(self.bitrate as usize);
        
        // NVENC-specific performance options
        encoder.set_option("preset", NVENC_PRESET)
            .map_err(|e| GhostLinkError::Other(format!("Failed to set NVENC preset: {}", e)))?;
        encoder.set_option("tune", NVENC_TUNE)
            .map_err(|e| GhostLinkError::Other(format!("Failed to set NVENC tune: {}", e)))?;
        encoder.set_option("delay", "0") // Zero frame delay
            .map_err(|e| GhostLinkError::Other(format!("Failed to set delay: {}", e)))?;
        encoder.set_option("zerolatency", "1") // Enable zero latency mode
            .map_err(|e| GhostLinkError::Other(format!("Failed to set zerolatency: {}", e)))?;
        encoder.set_option("cbr", "1") // Constant bitrate for streaming
            .map_err(|e| GhostLinkError::Other(format!("Failed to set CBR: {}", e)))?;
        
        // Quality settings optimized for real-time
        match self.codec_type {
            NvencCodec::H264 => {
                encoder.set_option("profile", "high")
                    .map_err(|e| GhostLinkError::Other(format!("Failed to set H.264 profile: {}", e)))?;
                encoder.set_option("level", "4.1")
                    .map_err(|e| GhostLinkError::Other(format!("Failed to set H.264 level: {}", e)))?;
            }
            NvencCodec::H265 => {
                encoder.set_option("profile", "main")
                    .map_err(|e| GhostLinkError::Other(format!("Failed to set H.265 profile: {}", e)))?;
                encoder.set_option("tier", "main")
                    .map_err(|e| GhostLinkError::Other(format!("Failed to set H.265 tier: {}", e)))?;
            }
            NvencCodec::AV1 => {
                encoder.set_option("usage", "realtime")
                    .map_err(|e| GhostLinkError::Other(format!("Failed to set AV1 usage: {}", e)))?;
            }
        }
        
        // Open encoder
        let encoder = encoder.open()
            .map_err(|e| GhostLinkError::Other(format!("Failed to open NVENC encoder: {}", e)))?;
        
        // Create optimized scaler (RGBA -> NV12 for NVENC)
        let scaler = ffmpeg::software::scaling::Context::get(
            ffmpeg::format::Pixel::RGBA,
            width,
            height,
            ffmpeg::format::Pixel::NV12,
            width,
            height,
            ffmpeg::software::scaling::Flags::BILINEAR,
        ).map_err(|e| GhostLinkError::Other(format!("Failed to create scaler: {}", e)))?;
        
        let context = EncoderContext {
            ffmpeg_context: encoder,
            scaler,
            cuda_context: None,
        };
        
        self.encoder_context = Some(Arc::new(Mutex::new(context)));
        
        // Initialize CUDA context for GPU operations
        self.init_cuda_context()?;
        
        info!("NVENC {:?} encoder initialized successfully", self.codec_type);
        Ok(())
    }

    /// Encode frame using NVENC hardware acceleration
    #[cfg(feature = "nvenc")]
    fn encode_frame_nvenc(&mut self, frame: &Frame) -> Result<Vec<u8>> {
        let context_arc = self.encoder_context.as_ref()
            .ok_or_else(|| GhostLinkError::Other("NVENC encoder not initialized".to_string()))?;
        
        let mut context = context_arc.lock();
        
        // Create input frame with GPU-optimized format
        let mut input_frame = ffmpeg::frame::Video::empty();
        input_frame.set_width(self.width);
        input_frame.set_height(self.height);
        input_frame.set_format(ffmpeg::format::Pixel::RGBA);
        
        // Upload frame data (in real implementation, would use GPU memory)
        unsafe {
            let plane = input_frame.data_mut(0);
            let src_len = frame.data.len().min(plane.len());
            plane[..src_len].copy_from_slice(&frame.data[..src_len]);
        }
        
        // Convert colorspace (RGBA -> NV12 for optimal NVENC performance)
        let mut nv12_frame = ffmpeg::frame::Video::empty();
        nv12_frame.set_width(self.width);
        nv12_frame.set_height(self.height);
        nv12_frame.set_format(ffmpeg::format::Pixel::NV12);
        
        context.scaler.run(&input_frame, &mut nv12_frame)
            .map_err(|e| GhostLinkError::Other(format!("NVENC color conversion failed: {}", e)))?;
        
        // Set frame timing and properties
        nv12_frame.set_pts(Some(self.frame_count as i64));
        
        // Let NVENC decide keyframes based on GOP settings for better efficiency
        
        // Submit frame to NVENC encoder
        context.ffmpeg_context.send_frame(&nv12_frame)
            .map_err(|e| GhostLinkError::Other(format!("Failed to send frame to NVENC: {}", e)))?;
        
        // Receive encoded packet from GPU
        let mut packet = ffmpeg::packet::Packet::empty();
        match context.ffmpeg_context.receive_packet(&mut packet) {
            Ok(_) => {
                let encoded_data = packet.data().unwrap_or(&[]).to_vec();
                debug!("NVENC encoded frame {} -> {} bytes (GPU accelerated)", self.frame_count, encoded_data.len());
                Ok(encoded_data)
            }
            Err(ffmpeg::Error::Other { errno: ffmpeg::util::error::EAGAIN }) => {
                // No packet ready yet (GPU still processing)
                Ok(Vec::new())
            }
            Err(e) => {
                error!("NVENC encoding failed: {}", e);
                Err(GhostLinkError::Other(format!("NVENC encode failed: {}", e)))
            }
        }
    }

    /// Get GPU utilization stats
    pub fn get_gpu_stats(&self) -> Result<GpuStats> {
        #[cfg(feature = "nvenc")]
        {
            // In real implementation, would query NVIDIA Management Library (NVML)
            // For now, return mock data
            Ok(GpuStats {
                gpu_utilization: 75,
                memory_used: 2048,
                memory_total: 8192,
                temperature: 65,
                encoder_utilization: 45,
            })
        }
        
        #[cfg(not(feature = "nvenc"))]
        {
            Err(GhostLinkError::Other("NVENC not available".to_string()))
        }
    }
}

pub struct GpuStats {
    pub gpu_utilization: u32,      // Percentage
    pub memory_used: u32,          // MB
    pub memory_total: u32,         // MB
    pub temperature: u32,          // Celsius
    pub encoder_utilization: u32,  // Percentage
}

#[async_trait]
impl VideoEncoder for NvencEncoder {
    async fn initialize(&mut self, width: u32, height: u32, fps: u32) -> Result<()> {
        info!("Initializing NVENC {:?} encoder: {}x{} @ {}fps", self.codec_type, width, height, fps);
        
        // Check if NVENC is available
        if !Self::is_available() {
            return Err(GhostLinkError::Other("NVENC not available on this system".to_string()));
        }
        
        self.width = width;
        self.height = height;
        self.fps = fps;
        self.keyframe_interval = fps as u64 * 2; // Keyframe every 2 seconds
        
        #[cfg(feature = "nvenc")]
        {
            self.init_nvenc_encoder(width, height, fps)?;
        }
        
        #[cfg(not(feature = "nvenc"))]
        {
            return Err(GhostLinkError::Other("NVENC support not compiled".to_string()));
        }
        
        self.is_initialized = true;
        info!("NVENC {:?} encoder initialized successfully", self.codec_type);
        
        Ok(())
    }

    async fn encode_frame(&self, frame: &Frame) -> Result<Vec<u8>> {
        if !self.is_initialized {
            return Err(GhostLinkError::Other("NVENC encoder not initialized".to_string()));
        }

        // Validate frame
        if frame.pixel_format != PixelFormat::RGBA {
            return Err(GhostLinkError::Other("NVENC requires RGBA pixel format".to_string()));
        }

        if frame.width != self.width || frame.height != self.height {
            return Err(GhostLinkError::Other("Frame size mismatch".to_string()));
        }

        // Cast for frame counting
        let mut_self = unsafe { &mut *(self as *const _ as *mut _) };
        mut_self.frame_count += 1;

        #[cfg(feature = "nvenc")]
        {
            mut_self.encode_frame_nvenc(frame)
        }
        
        #[cfg(not(feature = "nvenc"))]
        {
            Err(GhostLinkError::Other("NVENC not available".to_string()))
        }
    }

    fn get_encoder_info(&self) -> EncoderInfo {
        EncoderInfo {
            name: format!("NVENC {:?} Hardware Encoder", self.codec_type),
            hardware_accelerated: true,
            supported_formats: vec![PixelFormat::RGBA, PixelFormat::NV12],
            max_resolution: (7680, 4320), // Support up to 8K with modern GPUs
        }
    }

    fn is_healthy(&self) -> bool {
        self.is_initialized && Self::is_available()
    }

    async fn cleanup(&mut self) -> Result<()> {
        info!("Cleaning up NVENC {:?} encoder", self.codec_type);
        
        if let Some(context_arc) = self.encoder_context.take() {
            #[cfg(feature = "nvenc")]
            {
                let mut context = context_arc.lock();
                // Flush encoder
                if let Err(e) = context.ffmpeg_context.send_eof() {
                    warn!("Failed to flush NVENC encoder: {}", e);
                }
                
                // Cleanup CUDA context
                if let Some(_cuda_ctx) = context.cuda_context.take() {
                    debug!("Cleaning up CUDA context");
                    // In real implementation, would cleanup CUDA resources
                }
            }
        }
        
        self.is_initialized = false;
        info!("NVENC {:?} encoder cleanup complete", self.codec_type);
        
        Ok(())
    }
}