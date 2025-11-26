//! Wayland-native screen capture using XDG Desktop Portal + PipeWire
//!
//! This module provides high-performance screen capture for Wayland compositors
//! using the Portal API for permission handling and PipeWire/GStreamer for
//! actual frame capture.
//!
//! Architecture:
//! 1. XDG Desktop Portal (DBus) - Handles permissions, creates session
//! 2. PipeWire - Provides the actual video stream
//! 3. GStreamer - Pipeline for efficient frame extraction
//!
//! Reference: RustDesk implementation in archive/rustdesk/libs/scrap/src/wayland/

#![allow(dead_code)]

pub mod portal;
pub mod pipewire;
pub mod capturer;

pub use capturer::WaylandPortalCapturer;

use std::sync::atomic::{AtomicBool, Ordering};

lazy_static::lazy_static! {
    /// Global flag to track if GStreamer has been initialized
    static ref GST_INITIALIZED: AtomicBool = AtomicBool::new(false);
}

/// Initialize GStreamer (only once)
pub fn init_gstreamer() -> crate::error::Result<()> {
    if !GST_INITIALIZED.swap(true, Ordering::SeqCst) {
        gstreamer::init().map_err(|e| {
            crate::error::GhostLinkError::Capture(crate::error::CaptureError::InitializationFailed {
                reason: format!("GStreamer init failed: {}", e),
            })
        })?;
        tracing::info!("GStreamer initialized successfully");
    }
    Ok(())
}

/// Detect which Wayland compositor is running
pub fn detect_compositor() -> CompositorType {
    if std::env::var("SWAYSOCK").is_ok() {
        return CompositorType::Sway;
    }
    if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
        return CompositorType::Hyprland;
    }
    if std::env::var("WAYFIRE_SOCKET").is_ok() {
        return CompositorType::Wayfire;
    }
    if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
        let desktop_lower = desktop.to_lowercase();
        if desktop_lower.contains("gnome") {
            return CompositorType::Gnome;
        }
        if desktop_lower.contains("kde") || desktop_lower.contains("plasma") {
            return CompositorType::KdePlasma;
        }
    }
    CompositorType::Unknown
}

/// Known Wayland compositors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositorType {
    Sway,
    Hyprland,
    Wayfire,
    Gnome,
    KdePlasma,
    Unknown,
}

impl CompositorType {
    /// Check if this compositor typically provides position info in Portal responses
    pub fn provides_position_info(&self) -> bool {
        matches!(self, CompositorType::Gnome)
    }
}
