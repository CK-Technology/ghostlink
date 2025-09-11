use crate::{
    capture::{
        frame_protocol::{FrameMessage, VideoCodec, QualityLevel, FrameStats},
        encoder_factory::{EncoderFactory, EncoderPreference},
        VideoEncoderEnum, VideoEncoder, ScreenCapturerEnum, ScreenCapturer, Frame,
    },
    connection::{RelayConnection, RelayMessage},
    error::{GhostLinkError, Result},
};

use std::{
    sync::{Arc, atomic::{AtomicU32, AtomicBool, Ordering}},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{
    sync::{RwLock, Mutex},
    time::{interval, sleep},
    task::JoinHandle,
};
use tracing::{debug, error, info, warn, trace};
use parking_lot::RwLock as ParkingRwLock;

const TARGET_FPS: u32 = 60;
const FRAME_TIME_MS: u64 = 1000 / TARGET_FPS as u64;
const KEYFRAME_INTERVAL_SECONDS: u64 = 2;
const ADAPTIVE_QUALITY_WINDOW: usize = 30; // Frames to average for quality adaptation
const MAX_FRAME_SIZE: usize = 2 * 1024 * 1024; // 2MB max frame size

/// High-performance frame streaming service with adaptive quality
pub struct FrameStreamingService {
    /// Session identifier
    session_id: [u8; 8],
    /// Connection to relay server
    connection: Arc<RelayConnection>,
    /// Screen capturer
    capturer: Arc<Mutex<ScreenCapturerEnum>>,
    /// Video encoder
    encoder: Arc<RwLock<Option<VideoEncoderEnum>>>,
    /// Current encoder preference
    encoder_preference: Arc<RwLock<EncoderPreference>>,
    /// Frame sequence counter
    sequence_counter: AtomicU32,
    /// Streaming state
    is_streaming: AtomicBool,
    /// Quality level
    current_quality: Arc<ParkingRwLock<QualityLevel>>,
    /// Frame statistics
    stats: Arc<ParkingRwLock<FrameStats>>,
    /// Streaming task handle
    stream_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    /// Last keyframe time
    last_keyframe: Arc<Mutex<Instant>>,
    /// Target bitrate (for adaptive quality)
    target_bitrate: Arc<RwLock<u32>>,
    /// Recent frame sizes for adaptive quality
    recent_frame_sizes: Arc<Mutex<Vec<usize>>>,
}

impl FrameStreamingService {
    /// Create new frame streaming service
    pub async fn new(
        session_id: [u8; 8],
        connection: Arc<RelayConnection>,
        capturer: ScreenCapturerEnum,
        preference: EncoderPreference,
    ) -> Result<Self> {
        info!("Creating frame streaming service for session {:?}", session_id);
        
        let capturer = Arc::new(Mutex::new(capturer));
        
        Ok(Self {
            session_id,
            connection,
            capturer,
            encoder: Arc::new(RwLock::new(None)),
            encoder_preference: Arc::new(RwLock::new(preference)),
            sequence_counter: AtomicU32::new(0),
            is_streaming: AtomicBool::new(false),
            current_quality: Arc::new(ParkingRwLock::new(QualityLevel::High)),
            stats: Arc::new(ParkingRwLock::new(FrameStats::default())),
            stream_task: Arc::new(Mutex::new(None)),
            last_keyframe: Arc::new(Mutex::new(Instant::now())),
            target_bitrate: Arc::new(RwLock::new(3000)), // 3 Mbps default
            recent_frame_sizes: Arc::new(Mutex::new(Vec::new())),
        })
    }
    
    /// Initialize encoder for streaming
    pub async fn initialize(&self, width: u32, height: u32) -> Result<()> {
        info!("Initializing frame streaming: {}x{} @ {}fps", width, height, TARGET_FPS);
        
        // Get target bitrate based on quality
        let quality = *self.current_quality.read();
        let bitrate = match quality {
            QualityLevel::Ultra => 8000,   // 8 Mbps
            QualityLevel::High => 5000,    // 5 Mbps  
            QualityLevel::Medium => 3000,  // 3 Mbps
            QualityLevel::Low => 1500,     // 1.5 Mbps
            QualityLevel::Potato => 800,   // 800 Kbps
        };
        
        *self.target_bitrate.write().await = bitrate;
        
        // Create best encoder for streaming
        let preference = *self.encoder_preference.read().await;
        let encoder = EncoderFactory::create_streaming_encoder(bitrate, TARGET_FPS).await?;
        
        // Initialize encoder
        if let VideoEncoderEnum::Software(ref mut sw_encoder) = &encoder {
            sw_encoder.initialize(width, height, TARGET_FPS).await?;
        } else if let VideoEncoderEnum::H264(ref mut h264_encoder) = &encoder {
            h264_encoder.initialize(width, height, TARGET_FPS).await?;
        } else if let VideoEncoderEnum::Hevc(ref mut hevc_encoder) = &encoder {
            hevc_encoder.initialize(width, height, TARGET_FPS).await?;
        } else if let VideoEncoderEnum::NvencH264(ref mut nvenc_encoder) = &encoder {
            nvenc_encoder.initialize(width, height, TARGET_FPS).await?;
        } else if let VideoEncoderEnum::NvencH265(ref mut nvenc_encoder) = &encoder {
            nvenc_encoder.initialize(width, height, TARGET_FPS).await?;
        } else if let VideoEncoderEnum::NvencAV1(ref mut nvenc_encoder) = &encoder {
            nvenc_encoder.initialize(width, height, TARGET_FPS).await?;
        }
        
        // Store encoder
        *self.encoder.write().await = Some(encoder);
        
        info!("Frame streaming initialized with encoder: {:?}", self.get_encoder_info().await);
        Ok(())
    }
    
    /// Start streaming frames
    pub async fn start_streaming(&self) -> Result<()> {
        if self.is_streaming.load(Ordering::Relaxed) {
            warn!("Frame streaming already started");
            return Ok(());
        }
        
        info!("Starting frame streaming for session {:?}", self.session_id);
        self.is_streaming.store(true, Ordering::Relaxed);
        
        // Reset stats and state
        *self.stats.write() = FrameStats::default();
        self.sequence_counter.store(0, Ordering::Relaxed);
        *self.last_keyframe.lock().await = Instant::now();
        
        // Start streaming task
        let task = self.spawn_streaming_task().await;
        *self.stream_task.lock().await = Some(task);
        
        info!("Frame streaming started");
        Ok(())
    }
    
    /// Stop streaming frames
    pub async fn stop_streaming(&self) -> Result<()> {
        if !self.is_streaming.load(Ordering::Relaxed) {
            return Ok(());
        }
        
        info!("Stopping frame streaming for session {:?}", self.session_id);
        self.is_streaming.store(false, Ordering::Relaxed);
        
        // Stop streaming task
        if let Some(task) = self.stream_task.lock().await.take() {
            task.abort();
            let _ = task.await;
        }
        
        // Cleanup encoder
        if let Some(ref mut encoder) = *self.encoder.write().await {
            match encoder {
                VideoEncoderEnum::H264(ref mut enc) => { let _ = enc.cleanup().await; }
                VideoEncoderEnum::Hevc(ref mut enc) => { let _ = enc.cleanup().await; }
                VideoEncoderEnum::NvencH264(ref mut enc) => { let _ = enc.cleanup().await; }
                VideoEncoderEnum::NvencH265(ref mut enc) => { let _ = enc.cleanup().await; }
                VideoEncoderEnum::NvencAV1(ref mut enc) => { let _ = enc.cleanup().await; }
                _ => {}
            }
        }
        
        info!("Frame streaming stopped");
        self.log_final_stats().await;
        Ok(())
    }
    
    /// Spawn the main streaming task
    async fn spawn_streaming_task(&self) -> JoinHandle<()> {
        let session_id = self.session_id;
        let connection = Arc::clone(&self.connection);
        let capturer = Arc::clone(&self.capturer);
        let encoder = Arc::clone(&self.encoder);
        let is_streaming = Arc::clone(&self.is_streaming);
        let sequence_counter = Arc::clone(&self.sequence_counter);
        let current_quality = Arc::clone(&self.current_quality);
        let stats = Arc::clone(&self.stats);
        let last_keyframe = Arc::clone(&self.last_keyframe);
        let recent_frame_sizes = Arc::clone(&self.recent_frame_sizes);
        
        tokio::spawn(async move {
            let mut frame_interval = interval(Duration::from_millis(FRAME_TIME_MS));
            frame_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            info!("Frame streaming task started");
            
            while is_streaming.load(Ordering::Relaxed) {
                frame_interval.tick().await;
                
                let start_time = Instant::now();
                
                // Capture frame
                let frame = match Self::capture_frame(&capturer).await {
                    Ok(frame) => frame,
                    Err(e) => {
                        error!("Frame capture failed: {}", e);
                        sleep(Duration::from_millis(100)).await; // Brief pause on error
                        continue;
                    }
                };
                
                // Encode frame
                let encoded_data = match Self::encode_frame(&encoder, &frame).await {
                    Ok(data) => data,
                    Err(e) => {
                        error!("Frame encoding failed: {}", e);
                        continue;
                    }
                };
                
                if encoded_data.is_empty() {
                    trace!("Encoder returned no data (buffering)");
                    continue;
                }
                
                // Determine if keyframe
                let should_keyframe = {
                    let last_kf = last_keyframe.lock().await;
                    last_kf.elapsed().as_secs() >= KEYFRAME_INTERVAL_SECONDS
                };
                
                if should_keyframe {
                    *last_keyframe.lock().await = Instant::now();
                }
                
                // Get codec from encoder
                let codec = Self::detect_codec(&encoder).await;
                let quality = *current_quality.read();
                
                // Create frame message
                let sequence = sequence_counter.fetch_add(1, Ordering::Relaxed);
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_micros() as u64;
                
                let mut frame_msg = FrameMessage::new(
                    sequence,
                    &session_id,
                    codec,
                    quality,
                    frame.width,
                    frame.height,
                    encoded_data,
                    timestamp,
                    should_keyframe,
                );
                
                // Serialize to binary
                let binary_data = match frame_msg.serialize_binary() {
                    Ok(data) => data,
                    Err(e) => {
                        error!("Frame serialization failed: {}", e);
                        continue;
                    }
                };
                
                // Check frame size for quality adaptation
                Self::update_frame_size_history(&recent_frame_sizes, binary_data.len()).await;
                
                // Send via WebSocket as binary message (more efficient than JSON)
                if let Err(e) = connection.send_binary_frame(binary_data).await {
                    error!("Failed to send frame: {}", e);
                    // Don't break on send error, might be temporary
                }
                
                // Update statistics
                let frame_info = frame_msg.get_info();
                stats.write().record_sent(&frame_info);
                
                let encode_time = start_time.elapsed();
                if encode_time.as_millis() > FRAME_TIME_MS {
                    debug!("Frame processing took {}ms (target: {}ms)", 
                        encode_time.as_millis(), FRAME_TIME_MS);
                }
                
                // Adaptive quality adjustment every 30 frames
                if sequence % ADAPTIVE_QUALITY_WINDOW as u32 == 0 {
                    Self::adapt_quality(&current_quality, &recent_frame_sizes).await;
                }
            }
            
            info!("Frame streaming task ended");
        })
    }
    
    /// Capture a frame from the screen capturer
    async fn capture_frame(capturer: &Arc<Mutex<ScreenCapturerEnum>>) -> Result<Frame> {
        let mut capturer_guard = capturer.lock().await;
        
        match capturer_guard.as_mut() {
            ScreenCapturerEnum::X11Fast(ref mut capturer) => {
                capturer.capture_frame().await
            }
            ScreenCapturerEnum::WaylandFast(ref mut capturer) => {
                capturer.capture_frame().await
            }
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::X11(ref mut capturer) => {
                capturer.capture_frame().await
            }
            #[cfg(target_os = "linux")]
            ScreenCapturerEnum::Wayland(ref mut capturer) => {
                capturer.capture_frame().await
            }
            #[cfg(target_os = "windows")]
            ScreenCapturerEnum::Windows(ref mut capturer) => {
                capturer.capture_frame().await
            }
            #[cfg(target_os = "macos")]
            ScreenCapturerEnum::MacOS(ref mut capturer) => {
                capturer.capture_frame().await
            }
        }
    }
    
    /// Encode a frame with the current encoder
    async fn encode_frame(encoder: &Arc<RwLock<Option<VideoEncoderEnum>>>, frame: &Frame) -> Result<Vec<u8>> {
        let encoder_guard = encoder.read().await;
        let encoder = encoder_guard.as_ref()
            .ok_or_else(|| GhostLinkError::Other("No encoder initialized".to_string()))?;
        
        match encoder {
            VideoEncoderEnum::Software(ref enc) => enc.encode_frame(frame).await,
            VideoEncoderEnum::H264(ref enc) => enc.encode_frame(frame).await,
            VideoEncoderEnum::Hevc(ref enc) => enc.encode_frame(frame).await,
            VideoEncoderEnum::NvencH264(ref enc) => enc.encode_frame(frame).await,
            VideoEncoderEnum::NvencH265(ref enc) => enc.encode_frame(frame).await,
            VideoEncoderEnum::NvencAV1(ref enc) => enc.encode_frame(frame).await,
        }
    }
    
    /// Detect codec from encoder type
    async fn detect_codec(encoder: &Arc<RwLock<Option<VideoEncoderEnum>>>) -> VideoCodec {
        let encoder_guard = encoder.read().await;
        if let Some(ref encoder) = *encoder_guard {
            match encoder {
                VideoEncoderEnum::Software(_) => VideoCodec::Png,
                VideoEncoderEnum::H264(_) => VideoCodec::H264,
                VideoEncoderEnum::Hevc(_) => VideoCodec::H265,
                VideoEncoderEnum::NvencH264(_) => VideoCodec::NvencH264,
                VideoEncoderEnum::NvencH265(_) => VideoCodec::NvencH265,
                VideoEncoderEnum::NvencAV1(_) => VideoCodec::NvencAV1,
            }
        } else {
            VideoCodec::Raw
        }
    }
    
    /// Update frame size history for quality adaptation
    async fn update_frame_size_history(recent_sizes: &Arc<Mutex<Vec<usize>>>, size: usize) {
        let mut sizes = recent_sizes.lock().await;
        sizes.push(size);
        if sizes.len() > ADAPTIVE_QUALITY_WINDOW {
            sizes.drain(0..sizes.len() - ADAPTIVE_QUALITY_WINDOW);
        }
    }
    
    /// Adapt quality based on recent frame sizes
    async fn adapt_quality(current_quality: &Arc<ParkingRwLock<QualityLevel>>, recent_sizes: &Arc<Mutex<Vec<usize>>>) {
        let sizes = recent_sizes.lock().await;
        if sizes.is_empty() {
            return;
        }
        
        let average_size = sizes.iter().sum::<usize>() / sizes.len();
        let current = *current_quality.read();
        
        // Adapt based on frame sizes
        let new_quality = if average_size > MAX_FRAME_SIZE {
            // Frames too large, reduce quality
            match current {
                QualityLevel::Ultra => QualityLevel::High,
                QualityLevel::High => QualityLevel::Medium,
                QualityLevel::Medium => QualityLevel::Low,
                QualityLevel::Low => QualityLevel::Potato,
                QualityLevel::Potato => QualityLevel::Potato,
            }
        } else if average_size < MAX_FRAME_SIZE / 4 {
            // Frames small, can increase quality
            match current {
                QualityLevel::Potato => QualityLevel::Low,
                QualityLevel::Low => QualityLevel::Medium,
                QualityLevel::Medium => QualityLevel::High,
                QualityLevel::High => QualityLevel::Ultra,
                QualityLevel::Ultra => QualityLevel::Ultra,
            }
        } else {
            current // No change
        };
        
        if new_quality as u8 != current as u8 {
            info!("Adapting quality: {:?} -> {:?} (avg frame size: {} bytes)", 
                current, new_quality, average_size);
            *current_quality.write() = new_quality;
        }
    }
    
    /// Get current encoder information
    async fn get_encoder_info(&self) -> Option<String> {
        let encoder_guard = self.encoder.read().await;
        encoder_guard.as_ref().map(|encoder| {
            match encoder {
                VideoEncoderEnum::Software(ref enc) => enc.get_encoder_info().name,
                VideoEncoderEnum::H264(ref enc) => enc.get_encoder_info().name,
                VideoEncoderEnum::Hevc(ref enc) => enc.get_encoder_info().name,
                VideoEncoderEnum::NvencH264(ref enc) => enc.get_encoder_info().name,
                VideoEncoderEnum::NvencH265(ref enc) => enc.get_encoder_info().name,
                VideoEncoderEnum::NvencAV1(ref enc) => enc.get_encoder_info().name,
            }
        })
    }
    
    /// Get streaming statistics
    pub fn get_stats(&self) -> FrameStats {
        self.stats.read().clone()
    }
    
    /// Log final statistics
    async fn log_final_stats(&self) {
        let stats = self.stats.read().clone();
        info!("Final streaming stats: {} frames sent, {} bytes, {:.1}% efficiency, {:.0} bytes/frame avg",
            stats.frames_sent,
            stats.bytes_sent,
            stats.get_efficiency() * 100.0,
            stats.get_average_frame_size()
        );
    }
    
    /// Check if streaming is active
    pub fn is_streaming(&self) -> bool {
        self.is_streaming.load(Ordering::Relaxed)
    }
    
    /// Change encoder preference and reinitialize if needed
    pub async fn change_encoder_preference(&self, preference: EncoderPreference) -> Result<()> {
        *self.encoder_preference.write().await = preference;
        
        // If currently streaming, need to reinitialize encoder
        if self.is_streaming() {
            warn!("Changing encoder preference while streaming - this will cause a brief interruption");
            // TODO: Implement smooth encoder transition
        }
        
        Ok(())
    }
    
    /// Change quality level
    pub fn set_quality_level(&self, quality: QualityLevel) {
        let old_quality = *self.current_quality.read();
        *self.current_quality.write() = quality;
        
        if quality as u8 != old_quality as u8 {
            info!("Quality level changed: {:?} -> {:?}", old_quality, quality);
        }
    }
}