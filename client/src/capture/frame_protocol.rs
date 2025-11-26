use crate::error::{GhostLinkError, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, trace};

const FRAME_HEADER_MAGIC: u32 = 0x47464D45; // "GFME" - GhostLink Frame Message
const PROTOCOL_VERSION: u16 = 1;

/// Video frame format supported by the protocol
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum VideoCodec {
    /// Raw uncompressed frame (fallback)
    Raw,
    /// PNG compressed frame (lossless)
    Png,
    /// JPEG compressed frame (fast, lossy)
    Jpeg,
    /// H.264 compressed frame
    H264,
    /// H.265/HEVC compressed frame
    H265,
    /// NVIDIA NVENC H.264
    NvencH264,
    /// NVIDIA NVENC H.265
    NvencH265,
    /// NVIDIA NVENC AV1 (latest GPUs)
    NvencAV1,
}

/// Frame quality level for adaptive streaming
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum QualityLevel {
    /// Maximum quality (lossless or near-lossless)
    Ultra,
    /// High quality for local networks
    High,
    /// Balanced quality/bandwidth
    Medium,
    /// Low quality for slow connections
    Low,
    /// Minimum quality for very slow connections
    Potato,
}

/// Binary frame header (fixed size for efficient parsing)
#[repr(packed)]
#[derive(Debug, Clone, Copy)]
pub struct FrameHeader {
    /// Magic number for validation
    magic: u32,
    /// Protocol version
    version: u16,
    /// Frame sequence number
    sequence: u32,
    /// Session ID (8 bytes for UUID-like format)
    session_id: [u8; 8],
    /// Video codec used
    codec: u8,
    /// Quality level
    quality: u8,
    /// Frame width
    width: u32,
    /// Frame height  
    height: u32,
    /// Encoded frame data size
    data_size: u32,
    /// Frame timestamp (microseconds)
    timestamp: u64,
    /// Flags (keyframe, etc.)
    flags: u16,
    /// CRC32 checksum of data
    checksum: u32,
    /// Reserved for future use
    reserved: [u8; 8],
}

/// Frame flags
pub const FLAG_KEYFRAME: u16 = 0x0001;
pub const FLAG_DELTA: u16 = 0x0002;
pub const FLAG_COMPRESSED: u16 = 0x0004;
pub const FLAG_ERROR_CORRECTION: u16 = 0x0008;

impl FrameHeader {
    /// Create a new frame header
    pub fn new(
        sequence: u32,
        session_id: &[u8; 8],
        codec: VideoCodec,
        quality: QualityLevel,
        width: u32,
        height: u32,
        data_size: u32,
        timestamp: u64,
        is_keyframe: bool,
    ) -> Self {
        let mut flags = if is_keyframe { FLAG_KEYFRAME } else { FLAG_DELTA };

        // Mark compressed codecs
        match codec {
            VideoCodec::Jpeg | VideoCodec::H264 | VideoCodec::H265 |
            VideoCodec::NvencH264 | VideoCodec::NvencH265 | VideoCodec::NvencAV1 => {
                flags |= FLAG_COMPRESSED;
            }
            _ => {}
        }
        
        Self {
            magic: FRAME_HEADER_MAGIC,
            version: PROTOCOL_VERSION,
            sequence,
            session_id: *session_id,
            codec: codec as u8,
            quality: quality as u8,
            width,
            height,
            data_size,
            timestamp,
            flags,
            checksum: 0, // Set by serialize_binary
            reserved: [0; 8],
        }
    }
    
    /// Get header size in bytes
    pub const fn size() -> usize {
        std::mem::size_of::<FrameHeader>()
    }
    
    /// Validate header magic and version
    pub fn validate(&self) -> Result<()> {
        let magic = self.magic;
        if magic != FRAME_HEADER_MAGIC {
            return Err(GhostLinkError::Protocol(
                format!("Invalid frame header magic: 0x{:08X}", magic)
            ));
        }
        
        let version = self.version;
        if version != PROTOCOL_VERSION {
            return Err(GhostLinkError::Protocol(
                format!("Unsupported protocol version: {}", version)
            ));
        }
        
        Ok(())
    }
    
    /// Get codec from header
    pub fn get_codec(&self) -> Result<VideoCodec> {
        match self.codec {
            0 => Ok(VideoCodec::Raw),
            1 => Ok(VideoCodec::Png),
            2 => Ok(VideoCodec::Jpeg),
            3 => Ok(VideoCodec::H264),
            4 => Ok(VideoCodec::H265),
            5 => Ok(VideoCodec::NvencH264),
            6 => Ok(VideoCodec::NvencH265),
            7 => Ok(VideoCodec::NvencAV1),
            _ => Err(GhostLinkError::Protocol(
                format!("Unknown codec: {}", self.codec)
            )),
        }
    }
    
    /// Get quality from header
    pub fn get_quality(&self) -> Result<QualityLevel> {
        match self.quality {
            0 => Ok(QualityLevel::Ultra),
            1 => Ok(QualityLevel::High),
            2 => Ok(QualityLevel::Medium),
            3 => Ok(QualityLevel::Low),
            4 => Ok(QualityLevel::Potato),
            _ => Err(GhostLinkError::Protocol(
                format!("Unknown quality level: {}", self.quality)
            )),
        }
    }
    
    /// Check if frame is a keyframe
    pub fn is_keyframe(&self) -> bool {
        (self.flags & FLAG_KEYFRAME) != 0
    }
    
    /// Check if frame is compressed
    pub fn is_compressed(&self) -> bool {
        (self.flags & FLAG_COMPRESSED) != 0
    }
}

/// Complete frame message with header and data
#[derive(Debug, Clone)]
pub struct FrameMessage {
    pub header: FrameHeader,
    pub data: Vec<u8>,
}

impl FrameMessage {
    /// Create a new frame message
    pub fn new(
        sequence: u32,
        session_id: &[u8; 8],
        codec: VideoCodec,
        quality: QualityLevel,
        width: u32,
        height: u32,
        data: Vec<u8>,
        timestamp: u64,
        is_keyframe: bool,
    ) -> Self {
        let header = FrameHeader::new(
            sequence,
            session_id,
            codec,
            quality,
            width,
            height,
            data.len() as u32,
            timestamp,
            is_keyframe,
        );
        
        Self { header, data }
    }
    
    /// Serialize frame to binary format for WebSocket transmission
    pub fn serialize_binary(&mut self) -> Result<Vec<u8>> {
        // Calculate CRC32 of data
        self.header.checksum = crc32fast::hash(&self.data);
        
        let total_size = FrameHeader::size() + self.data.len();
        let mut buffer = Vec::with_capacity(total_size);
        
        // Serialize header (unsafe but fast for performance)
        let header_bytes = unsafe {
            std::slice::from_raw_parts(
                &self.header as *const FrameHeader as *const u8,
                FrameHeader::size(),
            )
        };
        buffer.extend_from_slice(header_bytes);
        
        // Append frame data
        buffer.extend_from_slice(&self.data);
        
        let sequence = self.header.sequence;
        let width = self.header.width;
        let height = self.header.height;
        let codec = self.header.get_codec().unwrap_or(VideoCodec::Raw) as u8;
        let quality = self.header.get_quality().unwrap_or(QualityLevel::Medium);
        
        trace!("Serialized frame {} ({} bytes): {}x{} {} {:?}", 
            sequence, buffer.len(), 
            width, height,
            codec, quality);
        
        Ok(buffer)
    }
    
    /// Deserialize frame from binary data
    pub fn deserialize_binary(data: &[u8]) -> Result<Self> {
        if data.len() < FrameHeader::size() {
            return Err(GhostLinkError::Protocol(
                format!("Frame data too small: {} bytes", data.len())
            ));
        }
        
        // Parse header
        let header: FrameHeader = unsafe {
            std::ptr::read(data.as_ptr() as *const FrameHeader)
        };
        
        // Validate header
        header.validate()?;
        
        // Check data size
        let expected_size = FrameHeader::size() + header.data_size as usize;
        if data.len() != expected_size {
            return Err(GhostLinkError::Protocol(
                format!("Frame size mismatch: expected {}, got {}", 
                    expected_size, data.len())
            ));
        }
        
        // Extract frame data
        let frame_data = data[FrameHeader::size()..].to_vec();
        
        // Verify checksum
        let calculated_checksum = crc32fast::hash(&frame_data);
        let expected_checksum = header.checksum;
        if calculated_checksum != expected_checksum {
            return Err(GhostLinkError::Protocol(
                format!("Frame checksum mismatch: expected 0x{:08X}, got 0x{:08X}",
                    expected_checksum, calculated_checksum)
            ));
        }
        
        let sequence = header.sequence;
        let width = header.width;
        let height = header.height;
        let codec = header.codec;
        
        trace!("Deserialized frame {} ({} bytes): {}x{} codec={}", 
            sequence, data.len(), 
            width, height, codec);
        
        Ok(Self {
            header,
            data: frame_data,
        })
    }
    
    /// Get frame info for logging
    pub fn get_info(&self) -> FrameInfo {
        FrameInfo {
            sequence: self.header.sequence,
            codec: self.header.get_codec().unwrap_or(VideoCodec::Raw),
            quality: self.header.get_quality().unwrap_or(QualityLevel::Medium),
            width: self.header.width,
            height: self.header.height,
            data_size: self.data.len(),
            timestamp: self.header.timestamp,
            is_keyframe: self.header.is_keyframe(),
            is_compressed: self.header.is_compressed(),
        }
    }
}

/// Frame information for debugging and monitoring
#[derive(Debug, Clone)]
pub struct FrameInfo {
    pub sequence: u32,
    pub codec: VideoCodec,
    pub quality: QualityLevel,
    pub width: u32,
    pub height: u32,
    pub data_size: usize,
    pub timestamp: u64,
    pub is_keyframe: bool,
    pub is_compressed: bool,
}

/// Frame statistics for monitoring
#[derive(Debug, Clone, Default)]
pub struct FrameStats {
    pub frames_sent: u64,
    pub keyframes_sent: u64,
    pub bytes_sent: u64,
    pub frames_received: u64,
    pub keyframes_received: u64,
    pub bytes_received: u64,
    pub decode_errors: u64,
    pub checksum_errors: u64,
    pub last_sequence: u32,
    pub missed_frames: u64,
}

impl FrameStats {
    /// Record a sent frame
    pub fn record_sent(&mut self, info: &FrameInfo) {
        self.frames_sent += 1;
        self.bytes_sent += info.data_size as u64;
        
        if info.is_keyframe {
            self.keyframes_sent += 1;
        }
        
        if info.sequence != self.last_sequence + 1 && self.last_sequence != 0 {
            let missed = info.sequence - self.last_sequence - 1;
            self.missed_frames += missed as u64;
            debug!("Detected {} missed frames (seq {} -> {})", 
                missed, self.last_sequence, info.sequence);
        }
        
        self.last_sequence = info.sequence;
    }
    
    /// Record a received frame
    pub fn record_received(&mut self, info: &FrameInfo) {
        self.frames_received += 1;
        self.bytes_received += info.data_size as u64;
        
        if info.is_keyframe {
            self.keyframes_received += 1;
        }
    }
    
    /// Record a decode error
    pub fn record_decode_error(&mut self) {
        self.decode_errors += 1;
    }
    
    /// Record a checksum error
    pub fn record_checksum_error(&mut self) {
        self.checksum_errors += 1;
    }
    
    /// Get transmission efficiency (sent vs received)
    pub fn get_efficiency(&self) -> f64 {
        if self.frames_sent == 0 {
            return 0.0;
        }
        
        (self.frames_received as f64) / (self.frames_sent as f64)
    }
    
    /// Get average frame size
    pub fn get_average_frame_size(&self) -> f64 {
        if self.frames_sent == 0 {
            return 0.0;
        }
        
        (self.bytes_sent as f64) / (self.frames_sent as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_frame_serialization() {
        let session_id = [1, 2, 3, 4, 5, 6, 7, 8];
        let test_data = vec![0xFF, 0x00, 0xFF, 0x00];
        
        let mut frame = FrameMessage::new(
            42,
            &session_id,
            VideoCodec::H264,
            QualityLevel::High,
            1920,
            1080,
            test_data.clone(),
            12345678,
            true,
        );
        
        // Serialize
        let binary = frame.serialize_binary().unwrap();
        
        // Deserialize
        let decoded = FrameMessage::deserialize_binary(&binary).unwrap();
        
        // Verify
        assert_eq!(decoded.header.sequence, 42);
        assert_eq!(decoded.header.session_id, session_id);
        assert_eq!(decoded.header.get_codec().unwrap(), VideoCodec::H264);
        assert_eq!(decoded.header.width, 1920);
        assert_eq!(decoded.header.height, 1080);
        assert_eq!(decoded.data, test_data);
        assert_eq!(decoded.header.is_keyframe(), true);
    }
    
    #[test]
    fn test_frame_checksum_validation() {
        let session_id = [1, 2, 3, 4, 5, 6, 7, 8];
        let test_data = vec![0xAA, 0xBB, 0xCC, 0xDD];
        
        let mut frame = FrameMessage::new(
            1,
            &session_id,
            VideoCodec::Png,
            QualityLevel::Medium,
            640,
            480,
            test_data,
            98765432,
            false,
        );
        
        let mut binary = frame.serialize_binary().unwrap();
        
        // Corrupt the data
        binary[binary.len() - 1] = 0x00;
        
        // Should fail checksum validation
        let result = FrameMessage::deserialize_binary(&binary);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("checksum mismatch"));
    }
}