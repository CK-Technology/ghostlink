use crate::{
    error::{GhostLinkError, Result},
    capture::{Frame, frame_protocol::{FrameMessage, VideoCodec}},
    input::input_protocol::InputEvent,
};

use std::{
    collections::VecDeque,
    fs::{File, OpenOptions},
    io::{BufWriter, Write, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    sync::{RwLock, mpsc},
    task::JoinHandle,
    time::{interval, sleep},
};
use tracing::{debug, error, info, trace, warn};
use serde::{Deserialize, Serialize};
use parking_lot::RwLock as ParkingRwLock;

const RECORDING_BUFFER_SIZE: usize = 1000;  // Buffer up to 1000 events before writing
const RECORDING_FLUSH_INTERVAL_MS: u64 = 1000; // Flush to disk every second
const MAX_RECORDING_SIZE_MB: u64 = 10240; // 10GB max recording size
const RECORDING_FILE_EXTENSION: &str = ".glr"; // GhostLink Recording

/// Comprehensive session recording system for compliance and audit trails
pub struct SessionRecorder {
    /// Session identifier
    session_id: String,
    /// Recording configuration
    config: RecordingConfig,
    /// Recording file path
    recording_path: PathBuf,
    /// Video frame writer
    video_writer: Arc<RwLock<Option<VideoRecordingWriter>>>,
    /// Input event writer  
    input_writer: Arc<RwLock<Option<InputRecordingWriter>>>,
    /// Recording metadata
    metadata: Arc<RwLock<RecordingMetadata>>,
    /// Recording state
    is_recording: AtomicBool,
    /// Recording start time
    start_time: Arc<RwLock<Option<SystemTime>>>,
    /// Recording statistics
    stats: Arc<ParkingRwLock<RecordingStats>>,
    /// Event buffer for batching writes
    event_buffer: Arc<RwLock<VecDeque<RecordingEvent>>>,
    /// Flush task handle
    flush_task: Arc<RwLock<Option<JoinHandle<()>>>>,
    /// Recording size tracker
    recording_size_bytes: AtomicU64,
}

/// Recording configuration options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingConfig {
    /// Enable video recording
    pub record_video: bool,
    /// Enable input event recording
    pub record_input: bool,
    /// Enable audio recording (future)
    pub record_audio: bool,
    /// Video quality for recording (1-10)
    pub video_quality: u8,
    /// Frame rate for recording (fps)
    pub frame_rate: u32,
    /// Compression level (1-9)
    pub compression_level: u8,
    /// Maximum recording duration (seconds)
    pub max_duration_seconds: Option<u64>,
    /// Maximum recording size (bytes)
    pub max_size_bytes: Option<u64>,
    /// Recording directory
    pub recording_directory: PathBuf,
    /// Auto-save interval (seconds)
    pub auto_save_interval: u64,
    /// Include system information
    pub include_system_info: bool,
    /// Include network information
    pub include_network_info: bool,
    /// Encryption enabled
    pub encrypt_recording: bool,
    /// Encryption key (for encrypted recordings)
    pub encryption_key: Option<Vec<u8>>,
}

impl Default for RecordingConfig {
    fn default() -> Self {
        Self {
            record_video: true,
            record_input: true,
            record_audio: false, // Not implemented yet
            video_quality: 7,
            frame_rate: 30,
            compression_level: 6,
            max_duration_seconds: Some(3600 * 4), // 4 hours
            max_size_bytes: Some(MAX_RECORDING_SIZE_MB * 1024 * 1024),
            recording_directory: PathBuf::from("./recordings"),
            auto_save_interval: 60, // 1 minute
            include_system_info: true,
            include_network_info: true,
            encrypt_recording: false,
            encryption_key: None,
        }
    }
}

/// Recording metadata stored with each session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingMetadata {
    /// Session ID
    pub session_id: String,
    /// Recording start time
    pub start_time: u64,
    /// Recording end time
    pub end_time: Option<u64>,
    /// Duration in seconds
    pub duration_seconds: f64,
    /// Operator information
    pub operator: OperatorInfo,
    /// Target system information
    pub target_system: SystemInfo,
    /// Recording statistics
    pub stats: RecordingStats,
    /// File paths
    pub files: RecordingFiles,
    /// Recording configuration used
    pub config: RecordingConfig,
    /// Compliance information
    pub compliance: ComplianceInfo,
}

/// Information about the operator performing the session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorInfo {
    /// User ID
    pub user_id: String,
    /// Display name
    pub display_name: String,
    /// Email address
    pub email: String,
    /// Organization
    pub organization: String,
    /// Authentication method used
    pub auth_method: String,
    /// Client IP address
    pub client_ip: String,
    /// User agent
    pub user_agent: String,
}

/// Information about the target system being accessed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    /// Device name/hostname
    pub device_name: String,
    /// Operating system
    pub operating_system: String,
    /// System version
    pub system_version: String,
    /// CPU information
    pub cpu_info: String,
    /// Memory information
    pub memory_info: String,
    /// Network interfaces
    pub network_interfaces: Vec<NetworkInterface>,
    /// Screen resolution
    pub screen_resolution: String,
    /// Agent version
    pub agent_version: String,
    /// System uptime
    pub system_uptime: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    pub ip_address: String,
    pub mac_address: String,
    pub interface_type: String,
}

/// Recording statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecordingStats {
    /// Total frames recorded
    pub frames_recorded: u64,
    /// Total input events recorded
    pub input_events_recorded: u64,
    /// Total bytes written
    pub bytes_written: u64,
    /// Average frame rate
    pub average_fps: f64,
    /// Peak frame rate
    pub peak_fps: f64,
    /// Recording errors
    pub recording_errors: u64,
    /// Compression ratio achieved
    pub compression_ratio: f64,
    /// Recording efficiency
    pub recording_efficiency: f64,
}

/// File paths for different recording components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingFiles {
    /// Main recording file
    pub main_file: PathBuf,
    /// Video data file
    pub video_file: Option<PathBuf>,
    /// Input events file
    pub input_file: Option<PathBuf>,
    /// Audio file (future)
    pub audio_file: Option<PathBuf>,
    /// Metadata file
    pub metadata_file: PathBuf,
    /// Thumbnail image
    pub thumbnail_file: Option<PathBuf>,
}

/// Compliance information for audit trails
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceInfo {
    /// Session purpose/reason
    pub session_purpose: String,
    /// Compliance tags
    pub compliance_tags: Vec<String>,
    /// Data classification
    pub data_classification: String,
    /// Retention policy
    pub retention_days: u32,
    /// Legal hold status
    pub legal_hold: bool,
    /// Digital signature (for integrity)
    pub digital_signature: Option<String>,
    /// Audit log entries
    pub audit_log: Vec<AuditLogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub timestamp: u64,
    pub event_type: String,
    pub description: String,
    pub operator_id: String,
    pub client_ip: String,
}

/// Individual recording events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecordingEvent {
    /// Video frame with timestamp
    VideoFrame {
        timestamp: u64,
        frame_data: Vec<u8>,
        codec: VideoCodec,
        width: u32,
        height: u32,
        is_keyframe: bool,
    },
    /// Input event with timestamp
    InputEvent {
        timestamp: u64,
        event: InputEvent,
        source_ip: String,
    },
    /// Session event (start, pause, resume, end)
    SessionEvent {
        timestamp: u64,
        event_type: SessionEventType,
        description: String,
    },
    /// System event (resolution change, monitor change, etc.)
    SystemEvent {
        timestamp: u64,
        event_type: String,
        details: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionEventType {
    SessionStart,
    SessionPause,
    SessionResume,
    SessionEnd,
    OperatorJoin,
    OperatorLeave,
    PermissionChange,
    QualityChange,
}

/// Video recording writer for efficient video data storage
pub struct VideoRecordingWriter {
    file: BufWriter<File>,
    frame_count: u64,
    start_time: SystemTime,
    last_keyframe_time: u64,
}

/// Input recording writer for input event storage
pub struct InputRecordingWriter {
    file: BufWriter<File>,
    event_count: u64,
}

impl SessionRecorder {
    /// Create new session recorder
    pub async fn new(session_id: String, config: RecordingConfig) -> Result<Self> {
        info!("Creating session recorder for session: {}", session_id);
        
        // Ensure recording directory exists
        if !config.recording_directory.exists() {
            std::fs::create_dir_all(&config.recording_directory)
                .map_err(|e| GhostLinkError::Other(format!("Failed to create recording directory: {}", e)))?;
        }
        
        // Create recording file path
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let filename = format!("session_{}_{}{}", session_id, timestamp, RECORDING_FILE_EXTENSION);
        let recording_path = config.recording_directory.join(filename);
        
        // Initialize metadata
        let metadata = RecordingMetadata {
            session_id: session_id.clone(),
            start_time: timestamp,
            end_time: None,
            duration_seconds: 0.0,
            operator: OperatorInfo {
                user_id: "unknown".to_string(),
                display_name: "Unknown User".to_string(),
                email: "unknown@example.com".to_string(),
                organization: "Unknown".to_string(),
                auth_method: "unknown".to_string(),
                client_ip: "0.0.0.0".to_string(),
                user_agent: "GhostLink Client".to_string(),
            },
            target_system: Self::gather_system_info().await,
            stats: RecordingStats::default(),
            files: RecordingFiles {
                main_file: recording_path.clone(),
                video_file: None,
                input_file: None,
                audio_file: None,
                metadata_file: recording_path.with_extension("json"),
                thumbnail_file: None,
            },
            config: config.clone(),
            compliance: ComplianceInfo {
                session_purpose: "Remote access session".to_string(),
                compliance_tags: vec!["remote-access".to_string()],
                data_classification: "internal".to_string(),
                retention_days: 90,
                legal_hold: false,
                digital_signature: None,
                audit_log: Vec::new(),
            },
        };
        
        Ok(Self {
            session_id,
            config,
            recording_path,
            video_writer: Arc::new(RwLock::new(None)),
            input_writer: Arc::new(RwLock::new(None)),
            metadata: Arc::new(RwLock::new(metadata)),
            is_recording: AtomicBool::new(false),
            start_time: Arc::new(RwLock::new(None)),
            stats: Arc::new(ParkingRwLock::new(RecordingStats::default())),
            event_buffer: Arc::new(RwLock::new(VecDeque::with_capacity(RECORDING_BUFFER_SIZE))),
            flush_task: Arc::new(RwLock::new(None)),
            recording_size_bytes: AtomicU64::new(0),
        })
    }
    
    /// Start recording
    pub async fn start_recording(&self, operator_info: OperatorInfo) -> Result<()> {
        if self.is_recording.load(Ordering::Relaxed) {
            warn!("Recording already started for session {}", self.session_id);
            return Ok(());
        }
        
        info!("Starting recording for session: {}", self.session_id);
        
        // Update metadata with operator info
        {
            let mut metadata = self.metadata.write().await;
            metadata.operator = operator_info;
            metadata.start_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
        }
        
        // Initialize writers
        if self.config.record_video {
            self.init_video_writer().await?;
        }
        
        if self.config.record_input {
            self.init_input_writer().await?;
        }
        
        // Start recording
        *self.start_time.write().await = Some(SystemTime::now());
        self.is_recording.store(true, Ordering::Relaxed);
        
        // Start flush task
        self.start_flush_task().await;
        
        // Record session start event
        self.record_session_event(SessionEventType::SessionStart, "Recording started").await?;
        
        info!("Recording started successfully for session: {}", self.session_id);
        Ok(())
    }
    
    /// Stop recording
    pub async fn stop_recording(&self) -> Result<()> {
        if !self.is_recording.load(Ordering::Relaxed) {
            return Ok(());
        }
        
        info!("Stopping recording for session: {}", self.session_id);
        
        // Record session end event
        self.record_session_event(SessionEventType::SessionEnd, "Recording stopped").await?;
        
        // Stop recording
        self.is_recording.store(false, Ordering::Relaxed);
        
        // Stop flush task
        if let Some(task) = self.flush_task.write().await.take() {
            task.abort();
            let _ = task.await;
        }
        
        // Final flush
        self.flush_events().await?;
        
        // Close writers
        self.close_writers().await?;
        
        // Update metadata
        self.update_final_metadata().await?;
        
        // Save metadata
        self.save_metadata().await?;
        
        info!("Recording stopped and saved for session: {}", self.session_id);
        Ok(())
    }
    
    /// Record a video frame
    pub async fn record_frame(&self, frame_message: &FrameMessage) -> Result<()> {
        if !self.is_recording.load(Ordering::Relaxed) || !self.config.record_video {
            return Ok(());
        }
        
        // Check recording size limits
        if let Some(max_size) = self.config.max_size_bytes {
            if self.recording_size_bytes.load(Ordering::Relaxed) >= max_size {
                warn!("Recording size limit reached, stopping recording");
                self.stop_recording().await?;
                return Ok(());
            }
        }
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        
        let codec = VideoCodec::H264; // TODO: Get from frame_message
        let event = RecordingEvent::VideoFrame {
            timestamp,
            frame_data: frame_message.data.clone(),
            codec,
            width: frame_message.header.width,
            height: frame_message.header.height,
            is_keyframe: frame_message.header.is_keyframe(),
        };
        
        self.buffer_event(event).await;
        
        // Update stats
        {
            let mut stats = self.stats.write();
            stats.frames_recorded += 1;
            stats.bytes_written += frame_message.data.len() as u64;
        }
        
        trace!("Recorded video frame: {} bytes", frame_message.data.len());
        Ok(())
    }
    
    /// Record an input event
    pub async fn record_input_event(&self, input_event: &InputEvent, source_ip: &str) -> Result<()> {
        if !self.is_recording.load(Ordering::Relaxed) || !self.config.record_input {
            return Ok(());
        }
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        
        let event = RecordingEvent::InputEvent {
            timestamp,
            event: input_event.clone(),
            source_ip: source_ip.to_string(),
        };
        
        self.buffer_event(event).await;
        
        // Update stats
        {
            let mut stats = self.stats.write();
            stats.input_events_recorded += 1;
        }
        
        trace!("Recorded input event: {:?}", input_event.event_type());
        Ok(())
    }
    
    /// Record a session event
    pub async fn record_session_event(&self, event_type: SessionEventType, description: &str) -> Result<()> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        
        let event = RecordingEvent::SessionEvent {
            timestamp,
            event_type,
            description: description.to_string(),
        };
        
        self.buffer_event(event).await;
        
        // Also add to compliance audit log
        {
            let mut metadata = self.metadata.write().await;
            metadata.compliance.audit_log.push(AuditLogEntry {
                timestamp,
                event_type: "session".to_string(),
                description: description.to_string(),
                operator_id: metadata.operator.user_id.clone(),
                client_ip: metadata.operator.client_ip.clone(),
            });
        }
        
        debug!("Recorded session event: {:?} - {}", event_type, description);
        Ok(())
    }
    
    /// Buffer event for batch writing
    async fn buffer_event(&self, event: RecordingEvent) {
        let mut buffer = self.event_buffer.write().await;
        buffer.push_back(event);
        
        // If buffer is full, trigger immediate flush
        if buffer.len() >= RECORDING_BUFFER_SIZE {
            drop(buffer);
            if let Err(e) = self.flush_events().await {
                error!("Failed to flush recording events: {}", e);
            }
        }
    }
    
    /// Start periodic flush task
    async fn start_flush_task(&self) {
        let event_buffer = Arc::clone(&self.event_buffer);
        let video_writer = Arc::clone(&self.video_writer);
        let input_writer = Arc::clone(&self.input_writer);
        let is_recording = Arc::clone(&self.is_recording);
        let recording_size_bytes = Arc::clone(&self.recording_size_bytes);
        
        let task = tokio::spawn(async move {
            let mut flush_interval = interval(Duration::from_millis(RECORDING_FLUSH_INTERVAL_MS));
            flush_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            while is_recording.load(Ordering::Relaxed) {
                flush_interval.tick().await;
                
                if let Err(e) = Self::flush_events_internal(
                    &event_buffer,
                    &video_writer,
                    &input_writer,
                    &recording_size_bytes,
                ).await {
                    error!("Failed to flush recording events: {}", e);
                }
            }
        });
        
        *self.flush_task.write().await = Some(task);
    }
    
    /// Flush buffered events to disk
    async fn flush_events(&self) -> Result<()> {
        Self::flush_events_internal(
            &self.event_buffer,
            &self.video_writer,
            &self.input_writer,
            &self.recording_size_bytes,
        ).await
    }
    
    /// Internal flush implementation
    async fn flush_events_internal(
        event_buffer: &Arc<RwLock<VecDeque<RecordingEvent>>>,
        video_writer: &Arc<RwLock<Option<VideoRecordingWriter>>>,
        input_writer: &Arc<RwLock<Option<InputRecordingWriter>>>,
        recording_size_bytes: &AtomicU64,
    ) -> Result<()> {
        let events: Vec<RecordingEvent> = {
            let mut buffer = event_buffer.write().await;
            let events: Vec<_> = buffer.drain(..).collect();
            events
        };
        
        if events.is_empty() {
            return Ok(());
        }
        
        trace!("Flushing {} recording events to disk", events.len());
        
        let mut bytes_written = 0;
        
        for event in events {
            match event {
                RecordingEvent::VideoFrame { frame_data, .. } => {
                    if let Some(ref mut writer) = *video_writer.write().await {
                        let serialized = bincode::serialize(&event)
                            .map_err(|e| GhostLinkError::Other(format!("Serialization error: {}", e)))?;
                        writer.file.write_all(&serialized)?;
                        writer.frame_count += 1;
                        bytes_written += serialized.len();
                    }
                }
                RecordingEvent::InputEvent { .. } => {
                    if let Some(ref mut writer) = *input_writer.write().await {
                        let serialized = bincode::serialize(&event)
                            .map_err(|e| GhostLinkError::Other(format!("Serialization error: {}", e)))?;
                        writer.file.write_all(&serialized)?;
                        writer.event_count += 1;
                        bytes_written += serialized.len();
                    }
                }
                _ => {
                    // Session and system events can go to either writer
                    if let Some(ref mut writer) = *input_writer.write().await {
                        let serialized = bincode::serialize(&event)
                            .map_err(|e| GhostLinkError::Other(format!("Serialization error: {}", e)))?;
                        writer.file.write_all(&serialized)?;
                        bytes_written += serialized.len();
                    }
                }
            }
        }
        
        // Flush writers
        if let Some(ref mut writer) = *video_writer.write().await {
            writer.file.flush()?;
        }
        if let Some(ref mut writer) = *input_writer.write().await {
            writer.file.flush()?;
        }
        
        recording_size_bytes.fetch_add(bytes_written as u64, Ordering::Relaxed);
        trace!("Flushed {} bytes to recording files", bytes_written);
        
        Ok(())
    }
    
    /// Initialize video recording writer
    async fn init_video_writer(&self) -> Result<()> {
        let video_path = self.recording_path.with_extension("video");
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&video_path)
            .map_err(|e| GhostLinkError::Other(format!("Failed to create video file: {}", e)))?;
        
        let writer = VideoRecordingWriter {
            file: BufWriter::new(file),
            frame_count: 0,
            start_time: SystemTime::now(),
            last_keyframe_time: 0,
        };
        
        *self.video_writer.write().await = Some(writer);
        
        // Update metadata
        {
            let mut metadata = self.metadata.write().await;
            metadata.files.video_file = Some(video_path);
        }
        
        info!("Video recording writer initialized");
        Ok(())
    }
    
    /// Initialize input recording writer
    async fn init_input_writer(&self) -> Result<()> {
        let input_path = self.recording_path.with_extension("input");
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&input_path)
            .map_err(|e| GhostLinkError::Other(format!("Failed to create input file: {}", e)))?;
        
        let writer = InputRecordingWriter {
            file: BufWriter::new(file),
            event_count: 0,
        };
        
        *self.input_writer.write().await = Some(writer);
        
        // Update metadata
        {
            let mut metadata = self.metadata.write().await;
            metadata.files.input_file = Some(input_path);
        }
        
        info!("Input recording writer initialized");
        Ok(())
    }
    
    /// Close all writers
    async fn close_writers(&self) -> Result<()> {
        if let Some(mut writer) = self.video_writer.write().await.take() {
            writer.file.flush()?;
            info!("Video writer closed, {} frames recorded", writer.frame_count);
        }
        
        if let Some(mut writer) = self.input_writer.write().await.take() {
            writer.file.flush()?;
            info!("Input writer closed, {} events recorded", writer.event_count);
        }
        
        Ok(())
    }
    
    /// Update final metadata before saving
    async fn update_final_metadata(&self) -> Result<()> {
        let mut metadata = self.metadata.write().await;
        
        if let Some(start_time) = *self.start_time.read().await {
            let duration = start_time.elapsed().unwrap_or_default();
            metadata.duration_seconds = duration.as_secs_f64();
            metadata.end_time = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            );
        }
        
        metadata.stats = self.stats.read().clone();
        
        // Calculate compression ratio
        if metadata.stats.bytes_written > 0 {
            // Estimate raw frame size and calculate compression
            let estimated_raw_size = metadata.stats.frames_recorded * 1920 * 1080 * 4; // Assume 1080p RGBA
            metadata.stats.compression_ratio = estimated_raw_size as f64 / metadata.stats.bytes_written as f64;
        }
        
        // Calculate average FPS
        if metadata.duration_seconds > 0.0 {
            metadata.stats.average_fps = metadata.stats.frames_recorded as f64 / metadata.duration_seconds;
        }
        
        info!("Final recording metadata updated: {:.1}s, {} frames, {} events",
            metadata.duration_seconds,
            metadata.stats.frames_recorded,
            metadata.stats.input_events_recorded
        );
        
        Ok(())
    }
    
    /// Save recording metadata to file
    async fn save_metadata(&self) -> Result<()> {
        let metadata = self.metadata.read().await;
        let json = serde_json::to_string_pretty(&*metadata)
            .map_err(|e| GhostLinkError::Other(format!("Failed to serialize metadata: {}", e)))?;
        
        std::fs::write(&metadata.files.metadata_file, json)
            .map_err(|e| GhostLinkError::Other(format!("Failed to write metadata file: {}", e)))?;
        
        info!("Recording metadata saved to: {:?}", metadata.files.metadata_file);
        Ok(())
    }
    
    /// Gather system information for metadata
    async fn gather_system_info() -> SystemInfo {
        use sysinfo::System;
        
        let mut sys = System::new_all();
        sys.refresh_all();
        
        SystemInfo {
            device_name: System::host_name().unwrap_or_else(|| "Unknown".to_string()),
            operating_system: System::name().unwrap_or_else(|| "Unknown".to_string()),
            system_version: System::os_version().unwrap_or_else(|| "Unknown".to_string()),
            cpu_info: sys.cpus().first()
                .map(|cpu| format!("{} ({} cores)", cpu.brand(), sys.cpus().len()))
                .unwrap_or_else(|| "Unknown".to_string()),
            memory_info: format!("{} MB total, {} MB available", 
                sys.total_memory() / 1024 / 1024,
                sys.available_memory() / 1024 / 1024),
            network_interfaces: vec![], // TODO: Implement network interface detection
            screen_resolution: "1920x1080".to_string(), // TODO: Get actual resolution
            agent_version: env!("CARGO_PKG_VERSION").to_string(),
            system_uptime: System::uptime(),
        }
    }
    
    /// Get recording statistics
    pub fn get_stats(&self) -> RecordingStats {
        self.stats.read().clone()
    }
    
    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::Relaxed)
    }
    
    /// Get recording metadata
    pub async fn get_metadata(&self) -> RecordingMetadata {
        self.metadata.read().await.clone()
    }
    
    /// Get recording file path
    pub fn get_recording_path(&self) -> &PathBuf {
        &self.recording_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_session_recorder_creation() {
        let config = RecordingConfig::default();
        let recorder = SessionRecorder::new("test_session".to_string(), config).await.unwrap();
        
        assert_eq!(recorder.session_id, "test_session");
        assert!(!recorder.is_recording());
    }
    
    #[tokio::test]
    async fn test_recording_lifecycle() {
        let mut config = RecordingConfig::default();
        config.recording_directory = PathBuf::from("./test_recordings");
        
        let recorder = SessionRecorder::new("test_session".to_string(), config).await.unwrap();
        
        let operator_info = OperatorInfo {
            user_id: "test_user".to_string(),
            display_name: "Test User".to_string(),
            email: "test@example.com".to_string(),
            organization: "Test Org".to_string(),
            auth_method: "test".to_string(),
            client_ip: "127.0.0.1".to_string(),
            user_agent: "test".to_string(),
        };
        
        // Start recording
        recorder.start_recording(operator_info).await.unwrap();
        assert!(recorder.is_recording());
        
        // Stop recording
        recorder.stop_recording().await.unwrap();
        assert!(!recorder.is_recording());
        
        // Cleanup test files
        let _ = std::fs::remove_dir_all("./test_recordings");
    }
}