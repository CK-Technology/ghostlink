use async_trait::async_trait;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};
use x11rb::connection::Connection;
use x11rb::protocol::damage::{self, ConnectionExt as DamageExt};
use x11rb::protocol::shm::ConnectionExt as ShmExt;
use x11rb::protocol::xfixes::ConnectionExt as XfixesExt;
use x11rb::protocol::xproto::{self, ConnectionExt as _};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

use crate::capture::{DisplayInfo, Frame, PixelFormat, ScreenCapturer};
use crate::error::{CaptureError, GhostLinkError, Result};

const TARGET_FPS: u32 = 60;
const FRAME_TIME_NS: u64 = 1_000_000_000 / TARGET_FPS as u64;

/// High-performance X11 screen capturer with XDamage for 60fps
pub struct X11FastCapturer {
    connection: Arc<RustConnection>,
    screen: usize,
    root: xproto::Window,
    width: u32,
    height: u32,
    damage: Option<damage::Damage>,
    shm_available: bool,
    last_frame_time: Instant,
    frame_buffer: Arc<Mutex<Vec<u8>>>,
    damage_regions: Arc<Mutex<Vec<DamageRegion>>>,
    is_initialized: bool,
}

#[derive(Debug, Clone)]
struct DamageRegion {
    x: i16,
    y: i16,
    width: u16,
    height: u16,
}

impl X11FastCapturer {
    pub async fn new() -> Result<Self> {
        info!("Initializing high-performance X11 screen capturer for 60fps");
        
        // Connect to X11
        let (connection, screen) = RustConnection::connect(None)
            .map_err(|e| GhostLinkError::Capture(CaptureError::ConnectionFailed {
                reason: format!("Failed to connect to X11: {}", e),
            }))?;
        
        let connection = Arc::new(connection);
        let setup = connection.setup();
        let screen_info = &setup.roots[screen];
        
        let root = screen_info.root;
        let width = screen_info.width_in_pixels as u32;
        let height = screen_info.height_in_pixels as u32;
        
        info!("X11 display: {}x{} @ screen {}", width, height, screen);
        
        // Check for SHM extension (required for high performance)
        let shm_available = Self::check_shm_available(&connection)?;
        if !shm_available {
            warn!("MIT-SHM not available, performance will be limited");
        }
        
        Ok(Self {
            connection,
            screen,
            root,
            width,
            height,
            damage: None,
            shm_available,
            last_frame_time: Instant::now(),
            frame_buffer: Arc::new(Mutex::new(vec![0u8; (width * height * 4) as usize])),
            damage_regions: Arc::new(Mutex::new(Vec::new())),
            is_initialized: false,
        })
    }
    
    /// Check if MIT-SHM extension is available
    fn check_shm_available(conn: &RustConnection) -> Result<bool> {
        match conn.shm_query_version() {
            Ok(cookie) => {
                match cookie.reply() {
                    Ok(reply) => {
                        info!("MIT-SHM version: {}.{}", reply.major_version, reply.minor_version);
                        Ok(true)
                    }
                    Err(_) => Ok(false)
                }
            }
            Err(_) => Ok(false)
        }
    }
    
    /// Initialize XDamage for tracking screen updates
    async fn init_damage(&mut self) -> Result<()> {
        // Check for XDamage extension
        match self.connection.damage_query_version(1, 1) {
            Ok(cookie) => {
                let reply = cookie.reply()
                    .map_err(|e| GhostLinkError::Capture(CaptureError::InitializationFailed {
                        reason: format!("XDamage query failed: {}", e),
                    }))?;
                
                info!("XDamage version: {}.{}", reply.major_version, reply.minor_version);
                
                // Create damage object for the root window
                let damage_id = self.connection.generate_id()?;
                self.connection.damage_create(
                    damage_id,
                    self.root,
                    damage::ReportLevel::RAW_RECTANGLES,
                )?;
                
                // Damage events are automatically selected when damage object is created
                
                self.damage = Some(damage_id);
                
                info!("XDamage initialized for incremental screen updates");
                Ok(())
            }
            Err(e) => {
                warn!("XDamage not available: {}, falling back to full screen capture", e);
                Ok(())
            }
        }
    }
    
    /// Capture using MIT-SHM for maximum performance
    async fn capture_shm(&self) -> Result<Vec<u8>> {
        // For now, fallback to regular capture
        // TODO: Implement shared memory capture
        self.capture_regular().await
    }
    
    /// Regular X11 capture without SHM
    async fn capture_regular(&self) -> Result<Vec<u8>> {
        let image = self.connection.get_image(
            xproto::ImageFormat::Z_PIXMAP,
            self.root,
            0,
            0,
            self.width as u16,
            self.height as u16,
            !0,
        )?.reply()
        .map_err(|e| GhostLinkError::Capture(CaptureError::CaptureFailed {
            reason: format!("Failed to get X11 image: {}", e),
        }))?;
        
        // Convert BGRA to RGBA
        let mut rgba_data = vec![0u8; (self.width * self.height * 4) as usize];
        for i in (0..image.data.len()).step_by(4) {
            rgba_data[i] = image.data[i + 2];     // R
            rgba_data[i + 1] = image.data[i + 1]; // G
            rgba_data[i + 2] = image.data[i];     // B
            rgba_data[i + 3] = image.data[i + 3]; // A
        }
        
        Ok(rgba_data)
    }
    
    /// Process damage events for incremental updates
    async fn process_damage_events(&self) -> Vec<DamageRegion> {
        let mut regions = Vec::new();
        
        // Poll for damage events without blocking
        while let Ok(Some(event)) = self.connection.poll_for_event() {
            match event {
                Event::DamageNotify(notify) => {
                    regions.push(DamageRegion {
                        x: notify.area.x,
                        y: notify.area.y,
                        width: notify.area.width,
                        height: notify.area.height,
                    });
                }
                _ => {}
            }
        }
        
        // Merge overlapping regions for efficiency
        Self::merge_damage_regions(&mut regions);
        
        regions
    }
    
    /// Merge overlapping damage regions
    fn merge_damage_regions(regions: &mut Vec<DamageRegion>) {
        if regions.len() <= 1 {
            return;
        }
        
        // Simple merge algorithm - can be optimized
        let mut merged = Vec::new();
        for region in regions.drain(..) {
            let mut was_merged = false;
            for existing in &mut merged {
                if Self::regions_overlap(&region, existing) {
                    Self::expand_region(existing, &region);
                    was_merged = true;
                    break;
                }
            }
            if !was_merged {
                merged.push(region);
            }
        }
        *regions = merged;
    }
    
    fn regions_overlap(a: &DamageRegion, b: &DamageRegion) -> bool {
        !(a.x + a.width as i16 <= b.x || 
          b.x + b.width as i16 <= a.x ||
          a.y + a.height as i16 <= b.y ||
          b.y + b.height as i16 <= a.y)
    }
    
    fn expand_region(target: &mut DamageRegion, source: &DamageRegion) {
        let min_x = target.x.min(source.x);
        let min_y = target.y.min(source.y);
        let max_x = (target.x + target.width as i16).max(source.x + source.width as i16);
        let max_y = (target.y + target.height as i16).max(source.y + source.height as i16);
        
        target.x = min_x;
        target.y = min_y;
        target.width = (max_x - min_x) as u16;
        target.height = (max_y - min_y) as u16;
    }
    
    /// Capture only damaged regions for incremental updates
    async fn capture_damage_regions(&self, regions: &[DamageRegion]) -> Result<()> {
        let mut buffer = self.frame_buffer.lock();
        
        for region in regions {
            // Clamp region to screen bounds
            let x = region.x.max(0) as u16;
            let y = region.y.max(0) as u16;
            let width = region.width.min(self.width as u16 - x);
            let height = region.height.min(self.height as u16 - y);
            
            if width == 0 || height == 0 {
                continue;
            }
            
            // Capture the damaged region
            let image = self.connection.get_image(
                xproto::ImageFormat::Z_PIXMAP,
                self.root,
                x as i16,
                y as i16,
                width,
                height,
                !0,
            )?.reply()
            .map_err(|e| GhostLinkError::Capture(CaptureError::CaptureFailed {
                reason: format!("Failed to capture damage region: {}", e),
            }))?;
            
            // Update the frame buffer with the damaged region
            let bytes_per_pixel = 4;
            let _stride = self.width * bytes_per_pixel;
            
            for row in 0..height as usize {
                let src_offset = row * (width as usize) * bytes_per_pixel as usize;
                let dst_offset = ((y as usize + row) * self.width as usize + x as usize) * bytes_per_pixel as usize;
                
                // Convert BGRA to RGBA while copying
                for col in 0..width as usize {
                    let src_idx = src_offset + col * 4;
                    let dst_idx = dst_offset + col * 4;
                    
                    buffer[dst_idx] = image.data[src_idx + 2];     // R
                    buffer[dst_idx + 1] = image.data[src_idx + 1]; // G
                    buffer[dst_idx + 2] = image.data[src_idx];     // B
                    buffer[dst_idx + 3] = image.data[src_idx + 3]; // A
                }
            }
        }
        
        Ok(())
    }
}

#[async_trait]
impl ScreenCapturer for X11FastCapturer {
    async fn initialize(&mut self) -> Result<()> {
        info!("Initializing X11 fast capturer for 60fps performance");
        
        // Initialize XDamage for incremental updates
        self.init_damage().await?;
        
        // Initialize XFixes for cursor capture
        match self.connection.xfixes_query_version(4, 0) {
            Ok(cookie) => {
                if let Ok(reply) = cookie.reply() {
                    info!("XFixes version: {}.{}", reply.major_version, reply.minor_version);
                }
            }
            Err(e) => {
                warn!("XFixes not available: {}", e);
            }
        }
        
        // Capture initial frame
        let initial_frame = if self.shm_available {
            self.capture_shm().await?
        } else {
            self.capture_regular().await?
        };
        
        *self.frame_buffer.lock() = initial_frame;
        
        self.is_initialized = true;
        info!("X11 fast capturer initialized successfully");
        
        Ok(())
    }
    
    async fn capture_frame(&mut self) -> Result<Frame> {
        if !self.is_initialized {
            return Err(GhostLinkError::Capture(CaptureError::NotInitialized));
        }
        
        // Enforce frame rate limit for consistent 60fps
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_frame_time);
        if elapsed.as_nanos() < FRAME_TIME_NS as u128 {
            // Sleep for remaining time to maintain 60fps
            tokio::time::sleep(Duration::from_nanos(FRAME_TIME_NS - elapsed.as_nanos() as u64)).await;
        }
        
        // Process damage events if available
        if self.damage.is_some() {
            let damage_regions = self.process_damage_events().await;
            
            if !damage_regions.is_empty() {
                debug!("Processing {} damage regions", damage_regions.len());
                self.capture_damage_regions(&damage_regions).await?;
            } else {
                // No damage, capture full frame every second as fallback
                if elapsed.as_secs() >= 1 {
                    let full_frame = if self.shm_available {
                        self.capture_shm().await?
                    } else {
                        self.capture_regular().await?
                    };
                    *self.frame_buffer.lock() = full_frame;
                }
            }
            
            // Clear damage
            if let Some(damage_id) = self.damage {
                let _ = self.connection.damage_subtract(damage_id, 0u32, 0u32);
            }
        } else {
            // No damage tracking, capture full frame
            let full_frame = if self.shm_available {
                self.capture_shm().await?
            } else {
                self.capture_regular().await?
            };
            *self.frame_buffer.lock() = full_frame;
        }
        
        self.last_frame_time = now;
        
        // Return the current frame buffer
        let buffer = self.frame_buffer.lock();
        Ok(Frame {
            data: buffer.clone(),
            width: self.width,
            height: self.height,
            pixel_format: PixelFormat::RGBA,
            stride: self.width * 4,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        })
    }
    
    fn get_display_info(&self) -> Vec<DisplayInfo> {
        vec![DisplayInfo {
            id: 0,
            name: format!("X11 Display :{}", self.screen),
            width: self.width,
            height: self.height,
            x: 0,
            y: 0,
            is_primary: true,
        }]
    }
    
    fn select_display(&mut self, _display_id: u32) -> Result<()> {
        // Single display for now
        Ok(())
    }
    
    fn set_capture_region(&mut self, x: i32, y: i32, width: u32, height: u32) -> Result<()> {
        // TODO: Implement region capture
        info!("Region capture set to: {}x{} at ({}, {})", width, height, x, y);
        Ok(())
    }
    
    fn get_resolution(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    
    fn is_healthy(&self) -> bool {
        self.is_initialized && self.connection.setup().roots.len() > 0
    }
    
    async fn cleanup(&mut self) -> Result<()> {
        info!("Cleaning up X11 fast capturer");
        
        // Destroy damage object
        if let Some(damage_id) = self.damage {
            let _ = self.connection.damage_destroy(damage_id);
        }
        
        self.is_initialized = false;
        Ok(())
    }

}