use tracing::{debug, info, warn};

use crate::capture::{
    VideoEncoderEnum, 
    h264_encoder::H264Encoder,
    hevc_encoder::HevcEncoder,
    nvenc_encoder::{NvencEncoder, NvencCodec},
    encoding::SoftwareEncoder,
};
use crate::error::{GhostLinkError, Result};

/// Encoder preferences for different use cases
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderPreference {
    /// Maximum performance (lowest latency, highest quality)
    MaxPerformance,
    /// Balanced performance and bandwidth
    Balanced,
    /// Minimum bandwidth (highest compression)
    MinBandwidth,
    /// Maximum compatibility (works on all systems)
    MaxCompatibility,
}

/// Encoder selection criteria
pub struct EncoderFactory;

impl EncoderFactory {
    /// Create the best available encoder based on system capabilities and preferences
    pub async fn create_best_encoder(
        preference: EncoderPreference,
        target_fps: u32,
    ) -> Result<VideoEncoderEnum> {
        info!("Selecting best encoder for {:?} at {}fps", preference, target_fps);
        
        // Check system capabilities
        let has_nvidia = Self::has_nvidia_gpu();
        let has_ffmpeg = Self::has_ffmpeg_support();
        
        info!("System capabilities: NVIDIA GPU = {}, FFmpeg = {}", has_nvidia, has_ffmpeg);
        
        match preference {
            EncoderPreference::MaxPerformance => {
                Self::create_performance_encoder(has_nvidia, has_ffmpeg, target_fps).await
            }
            EncoderPreference::Balanced => {
                Self::create_balanced_encoder(has_nvidia, has_ffmpeg, target_fps).await
            }
            EncoderPreference::MinBandwidth => {
                Self::create_bandwidth_efficient_encoder(has_nvidia, has_ffmpeg, target_fps).await
            }
            EncoderPreference::MaxCompatibility => {
                Self::create_compatible_encoder(has_ffmpeg).await
            }
        }
    }
    
    /// Create encoder optimized for maximum performance (lowest latency)
    async fn create_performance_encoder(
        has_nvidia: bool, 
        has_ffmpeg: bool,
        target_fps: u32,
    ) -> Result<VideoEncoderEnum> {
        // Priority: NVENC H.264 > H.264 Software > Fallback
        
        if has_nvidia && target_fps >= 60 {
            #[cfg(feature = "nvenc")]
            {
                info!("Using NVENC H.264 for maximum 60fps performance");
                let encoder = NvencEncoder::new(NvencCodec::H264);
                return Ok(VideoEncoderEnum::NvencH264(encoder));
            }
        }
        
        if has_ffmpeg {
            info!("Using software H.264 encoder for high performance");
            let encoder = H264Encoder::new();
            return Ok(VideoEncoderEnum::H264(encoder));
        }
        
        // Fallback to basic software encoder
        warn!("Falling back to basic software encoder");
        let encoder = SoftwareEncoder::new().await?;
        Ok(VideoEncoderEnum::Software(encoder))
    }
    
    /// Create encoder with balanced performance and bandwidth
    async fn create_balanced_encoder(
        has_nvidia: bool,
        has_ffmpeg: bool,
        target_fps: u32,
    ) -> Result<VideoEncoderEnum> {
        // Priority: NVENC H.265 > H.265 Software > NVENC H.264 > H.264 Software
        
        if has_nvidia {
            #[cfg(feature = "nvenc")]
            {
                if target_fps >= 60 {
                    info!("Using NVENC H.265 for balanced performance and compression");
                    let encoder = NvencEncoder::new(NvencCodec::H265);
                    return Ok(VideoEncoderEnum::NvencH265(encoder));
                } else {
                    info!("Using NVENC H.264 for balanced performance");
                    let encoder = NvencEncoder::new(NvencCodec::H264);
                    return Ok(VideoEncoderEnum::NvencH264(encoder));
                }
            }
        }
        
        if has_ffmpeg {
            info!("Using software H.265 encoder for balanced compression");
            let encoder = HevcEncoder::new();
            return Ok(VideoEncoderEnum::Hevc(encoder));
        }
        
        // Fallback
        warn!("Falling back to basic software encoder");
        let encoder = SoftwareEncoder::new().await?;
        Ok(VideoEncoderEnum::Software(encoder))
    }
    
    /// Create encoder optimized for minimum bandwidth
    async fn create_bandwidth_efficient_encoder(
        has_nvidia: bool,
        has_ffmpeg: bool,
        _target_fps: u32,
    ) -> Result<VideoEncoderEnum> {
        // Priority: NVENC AV1 > NVENC H.265 > H.265 Software > H.264
        
        if has_nvidia {
            #[cfg(feature = "nvenc")]
            {
                // Try AV1 for latest GPUs (best compression)
                if Self::supports_nvenc_av1() {
                    info!("Using NVENC AV1 for maximum compression efficiency");
                    let encoder = NvencEncoder::new(NvencCodec::AV1);
                    return Ok(VideoEncoderEnum::NvencAV1(encoder));
                }
                
                info!("Using NVENC H.265 for high compression efficiency");
                let encoder = NvencEncoder::new(NvencCodec::H265);
                return Ok(VideoEncoderEnum::NvencH265(encoder));
            }
        }
        
        if has_ffmpeg {
            info!("Using software H.265 encoder for bandwidth efficiency");
            let encoder = HevcEncoder::new();
            return Ok(VideoEncoderEnum::Hevc(encoder));
        }
        
        // Fallback
        warn!("Falling back to basic software encoder");
        let encoder = SoftwareEncoder::new().await?;
        Ok(VideoEncoderEnum::Software(encoder))
    }
    
    /// Create encoder optimized for maximum compatibility
    async fn create_compatible_encoder(has_ffmpeg: bool) -> Result<VideoEncoderEnum> {
        // Use H.264 for maximum compatibility across devices and browsers
        
        if has_ffmpeg {
            info!("Using software H.264 encoder for maximum compatibility");
            let encoder = H264Encoder::new();
            return Ok(VideoEncoderEnum::H264(encoder));
        }
        
        // Fallback to basic encoder
        warn!("Using basic software encoder for compatibility");
        let encoder = SoftwareEncoder::new().await?;
        Ok(VideoEncoderEnum::Software(encoder))
    }
    
    /// Check if NVIDIA GPU is available
    fn has_nvidia_gpu() -> bool {
        #[cfg(feature = "nvenc")]
        {
            NvencEncoder::is_available()
        }
        
        #[cfg(not(feature = "nvenc"))]
        {
            false
        }
    }
    
    /// Check if FFmpeg support is available
    fn has_ffmpeg_support() -> bool {
        #[cfg(feature = "x264-encoder")]
        {
            // In real implementation, would check if FFmpeg libraries are available
            true
        }
        
        #[cfg(not(feature = "x264-encoder"))]
        {
            false
        }
    }
    
    /// Check if NVENC supports AV1 (RTX 40 series and newer)
    fn supports_nvenc_av1() -> bool {
        #[cfg(feature = "nvenc")]
        {
            // In real implementation, would query GPU architecture
            // For now, assume modern GPUs support AV1
            if let Ok(output) = std::process::Command::new("nvidia-smi")
                .arg("--query-gpu=name")
                .arg("--format=csv,noheader,nounits")
                .output()
            {
                if let Ok(gpu_name) = String::from_utf8(output.stdout) {
                    let gpu_name = gpu_name.to_lowercase();
                    // RTX 40 series and newer support AV1 encoding
                    return gpu_name.contains("rtx 40") || 
                           gpu_name.contains("rtx 41") ||
                           gpu_name.contains("rtx 42") ||
                           gpu_name.contains("ada lovelace") ||
                           gpu_name.contains("hopper");
                }
            }
            false
        }
        
        #[cfg(not(feature = "nvenc"))]
        {
            false
        }
    }
    
    /// Get recommended encoder for streaming scenarios
    pub async fn create_streaming_encoder(
        bitrate_kbps: u32,
        target_fps: u32,
    ) -> Result<VideoEncoderEnum> {
        let preference = match bitrate_kbps {
            0..=1000 => EncoderPreference::MinBandwidth,      // Low bandwidth
            1001..=3000 => EncoderPreference::Balanced,       // Medium bandwidth  
            3001..=8000 => EncoderPreference::MaxPerformance, // High bandwidth
            _ => EncoderPreference::MaxPerformance,           // Very high bandwidth
        };
        
        info!("Creating streaming encoder for {}kbps at {}fps", bitrate_kbps, target_fps);
        Self::create_best_encoder(preference, target_fps).await
    }
    
    /// Get recommended encoder for recording scenarios
    pub async fn create_recording_encoder(quality_level: u32) -> Result<VideoEncoderEnum> {
        let preference = match quality_level {
            1..=3 => EncoderPreference::MinBandwidth,    // Low quality
            4..=6 => EncoderPreference::Balanced,        // Medium quality
            7..=10 => EncoderPreference::MaxPerformance, // High quality
            _ => EncoderPreference::Balanced,
        };
        
        info!("Creating recording encoder for quality level {}", quality_level);
        Self::create_best_encoder(preference, 30).await // 30fps default for recording
    }
    
    /// List all available encoders on this system
    pub fn list_available_encoders() -> Vec<String> {
        let mut encoders = Vec::new();
        
        // Always available
        encoders.push("Software (Basic)".to_string());
        
        #[cfg(feature = "x264-encoder")]
        {
            encoders.push("H.264 Software".to_string());
            encoders.push("H.265/HEVC Software".to_string());
        }
        
        #[cfg(feature = "nvenc")]
        if Self::has_nvidia_gpu() {
            encoders.push("NVENC H.264 Hardware".to_string());
            encoders.push("NVENC H.265 Hardware".to_string());
            
            if Self::supports_nvenc_av1() {
                encoders.push("NVENC AV1 Hardware".to_string());
            }
        }
        
        #[cfg(feature = "qsv")]
        {
            encoders.push("Intel QuickSync".to_string());
        }
        
        #[cfg(feature = "videotoolbox")]
        {
            encoders.push("Apple VideoToolbox".to_string());
        }
        
        info!("Available encoders: {:?}", encoders);
        encoders
    }
}