//! PipeWire stream capture using GStreamer
//!
//! Creates a GStreamer pipeline to capture frames from PipeWire streams
//! obtained via the XDG Desktop Portal.

use std::os::unix::io::AsRawFd;

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app::AppSink;

use tracing::{debug, trace, warn};

use crate::error::{Result, GhostLinkError, CaptureError};
use super::portal::StreamInfo;

/// Pixel format from PipeWire/GStreamer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeWirePixelFormat {
    BGRx,
    RGBx,
    BGRA,
    RGBA,
}

impl PipeWirePixelFormat {
    /// Bytes per pixel
    pub fn bpp(&self) -> usize {
        4 // All formats are 4 bytes per pixel
    }

    /// Convert to our internal pixel format
    pub fn to_capture_format(&self) -> crate::capture::PixelFormat {
        match self {
            PipeWirePixelFormat::BGRx | PipeWirePixelFormat::BGRA => crate::capture::PixelFormat::BGRA,
            PipeWirePixelFormat::RGBx | PipeWirePixelFormat::RGBA => crate::capture::PixelFormat::RGBA,
        }
    }
}

/// PipeWire stream wrapper
pub struct PipeWireStream {
    pub path: u64,
    pub position: (i32, i32),
    pub size: (u32, u32),
}

impl From<&StreamInfo> for PipeWireStream {
    fn from(info: &StreamInfo) -> Self {
        Self {
            path: info.path,
            position: info.position,
            size: info.size,
        }
    }
}

/// GStreamer-based PipeWire recorder
pub struct PipeWireRecorder {
    pipeline: gst::Pipeline,
    appsink: AppSink,
    width: u32,
    height: u32,
    pixel_format: PipeWirePixelFormat,
    // Cached buffer for frame comparison (skip unchanged frames)
    last_frame_hash: u64,
}

impl PipeWireRecorder {
    /// Create a new PipeWire recorder for the given stream
    pub fn new(fd: &impl AsRawFd, stream: &PipeWireStream) -> Result<Self> {
        // Ensure GStreamer is initialized
        super::init_gstreamer()?;

        let pipeline = gst::Pipeline::new();

        // Create pipewiresrc element
        let src = gst::ElementFactory::make("pipewiresrc")
            .build()
            .map_err(|e| GhostLinkError::Capture(CaptureError::InitializationFailed {
                reason: format!("Failed to create pipewiresrc: {}", e),
            }))?;

        // Configure pipewiresrc
        src.set_property("fd", fd.as_raw_fd());
        src.set_property("path", format!("{}", stream.path));
        src.set_property("keepalive-time", 1000i32);
        // Critical: set always-copy to avoid PipeWire destruction deadlock
        // See: https://gitlab.freedesktop.org/pipewire/pipewire/-/issues/982
        src.set_property("always-copy", true);

        // Create appsink for frame extraction
        let sink = gst::ElementFactory::make("appsink")
            .build()
            .map_err(|e| GhostLinkError::Capture(CaptureError::InitializationFailed {
                reason: format!("Failed to create appsink: {}", e),
            }))?;

        // Configure appsink
        sink.set_property("drop", true); // Drop old frames if we can't keep up
        sink.set_property("max-buffers", 1u32); // Only keep latest frame

        // Add elements to pipeline and link
        pipeline.add_many([&src, &sink]).map_err(|e| {
            GhostLinkError::Capture(CaptureError::InitializationFailed {
                reason: format!("Failed to add elements to pipeline: {}", e),
            })
        })?;

        src.link(&sink).map_err(|e| {
            GhostLinkError::Capture(CaptureError::InitializationFailed {
                reason: format!("Failed to link pipeline elements: {}", e),
            })
        })?;

        // Cast sink to AppSink
        let appsink = sink.dynamic_cast::<AppSink>().map_err(|_| {
            GhostLinkError::Capture(CaptureError::InitializationFailed {
                reason: "Failed to cast to AppSink".into(),
            })
        })?;

        // Set accepted caps (BGRx or RGBx for efficiency)
        let caps = gst::Caps::builder("video/x-raw")
            .field("format", gst::List::new(["BGRx", "RGBx", "BGRA", "RGBA"]))
            .build();
        appsink.set_caps(Some(&caps));

        // Start the pipeline
        debug!("Starting GStreamer pipeline for stream {}", stream.path);
        pipeline.set_state(gst::State::Playing).map_err(|e| {
            GhostLinkError::Capture(CaptureError::InitializationFailed {
                reason: format!("Failed to start pipeline: {}", e),
            })
        })?;

        // Wait for pipeline to reach playing state
        let (result, state, _pending) = pipeline.state(gst::ClockTime::from_mseconds(2000));
        match result {
            Ok(_) if state == gst::State::Playing => {
                debug!("Pipeline is now in PLAYING state");
            }
            _ => {
                warn!("Pipeline state change incomplete, proceeding anyway");
            }
        }

        // Small delay to let things settle (empirically helps stability)
        std::thread::sleep(std::time::Duration::from_millis(100));

        Ok(Self {
            pipeline,
            appsink,
            width: stream.size.0,
            height: stream.size.1,
            pixel_format: PipeWirePixelFormat::BGRx, // Will be updated on first frame
            last_frame_hash: 0,
        })
    }

    /// Capture a frame from the PipeWire stream
    ///
    /// Returns None if no new frame is available or if frame is unchanged
    pub fn capture_frame(&mut self, timeout_ms: u64) -> Result<Option<CapturedFrame>> {
        let sample = match self.appsink.try_pull_sample(gst::ClockTime::from_mseconds(timeout_ms)) {
            Some(s) => s,
            None => return Ok(None), // No frame available within timeout
        };

        // Get frame info from caps
        let caps = sample.caps().ok_or_else(|| {
            GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: "No caps on sample".into(),
            })
        })?;

        let structure = caps.structure(0).ok_or_else(|| {
            GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: "No structure in caps".into(),
            })
        })?;

        // Extract width and height
        let width: i32 = structure.get("width").map_err(|e| {
            GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: format!("Failed to get width: {}", e),
            })
        })?;
        let height: i32 = structure.get("height").map_err(|e| {
            GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: format!("Failed to get height: {}", e),
            })
        })?;

        self.width = width as u32;
        self.height = height as u32;

        // Extract pixel format
        let format_str: String = structure.get("format").map_err(|e| {
            GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: format!("Failed to get format: {}", e),
            })
        })?;

        self.pixel_format = match format_str.as_str() {
            "BGRx" => PipeWirePixelFormat::BGRx,
            "RGBx" => PipeWirePixelFormat::RGBx,
            "BGRA" => PipeWirePixelFormat::BGRA,
            "RGBA" => PipeWirePixelFormat::RGBA,
            other => {
                warn!("Unknown pixel format: {}, assuming BGRx", other);
                PipeWirePixelFormat::BGRx
            }
        };

        // Get the buffer
        let buffer = sample.buffer_owned().ok_or_else(|| {
            GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: "No buffer in sample".into(),
            })
        })?;

        // Check for crop metadata (for window captures)
        let crop = buffer
            .meta::<gstreamer_video::VideoCropMeta>()
            .map(|m| m.rect());

        // Map buffer for reading
        let mapped = buffer.into_mapped_buffer_readable().map_err(|_| {
            GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: "Failed to map buffer".into(),
            })
        })?;

        let data = mapped.as_slice();

        // Quick hash check to skip unchanged frames
        let frame_hash = Self::hash_frame(data);
        if frame_hash == self.last_frame_hash {
            trace!("Frame unchanged, skipping");
            return Ok(None);
        }
        self.last_frame_hash = frame_hash;

        // Validate buffer size
        let expected_size = (self.width * self.height * 4) as usize;
        if data.len() != expected_size {
            warn!(
                "Buffer size mismatch: got {}, expected {} ({}x{}x4)",
                data.len(), expected_size, self.width, self.height
            );
            return Ok(None);
        }

        // Handle crop if present
        let (final_data, final_width, final_height) = if let Some((x, y, w, h)) = crop {
            if x == 0 && y == 0 && w == self.width && h == self.height {
                // No actual cropping needed
                (data.to_vec(), self.width, self.height)
            } else {
                // Need to extract cropped region
                let cropped = Self::extract_crop(
                    data,
                    self.width as usize,
                    x as usize,
                    y as usize,
                    w as usize,
                    h as usize,
                );
                (cropped, w, h)
            }
        } else {
            (data.to_vec(), self.width, self.height)
        };

        Ok(Some(CapturedFrame {
            data: final_data,
            width: final_width,
            height: final_height,
            pixel_format: self.pixel_format,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        }))
    }

    /// Get current resolution
    pub fn resolution(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Quick hash of frame data for change detection
    fn hash_frame(data: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Sample every 1000th byte for speed
        let mut hasher = DefaultHasher::new();
        for (i, byte) in data.iter().enumerate() {
            if i % 1000 == 0 {
                byte.hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    /// Extract a cropped region from frame data
    fn extract_crop(
        data: &[u8],
        stride_width: usize,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
    ) -> Vec<u8> {
        let bpp = 4; // Bytes per pixel
        let mut cropped = Vec::with_capacity(width * height * bpp);

        for row in y..(y + height) {
            let start = (row * stride_width + x) * bpp;
            let end = start + (width * bpp);
            if end <= data.len() {
                cropped.extend_from_slice(&data[start..end]);
            }
        }

        cropped
    }
}

impl Drop for PipeWireRecorder {
    fn drop(&mut self) {
        debug!("Stopping PipeWire recorder pipeline");
        if let Err(e) = self.pipeline.set_state(gst::State::Null) {
            warn!("Failed to stop pipeline: {}", e);
        }
        // Wait for state change to complete
        let _ = self.pipeline.state(gst::ClockTime::from_mseconds(2000));
    }
}

/// A captured frame from PipeWire
#[derive(Debug, Clone)]
pub struct CapturedFrame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub pixel_format: PipeWirePixelFormat,
    pub timestamp: u64,
}

impl CapturedFrame {
    /// Convert to the generic Frame type
    pub fn to_frame(&self) -> crate::capture::Frame {
        crate::capture::Frame {
            data: self.data.clone(),
            width: self.width,
            height: self.height,
            stride: self.width * 4,
            pixel_format: self.pixel_format.to_capture_format(),
            timestamp: self.timestamp,
        }
    }
}
