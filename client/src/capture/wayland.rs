use anyhow::{Context, Result};
use std::collections::HashMap;
use std::process::Command;
use tracing::{debug, error, info, warn};

use super::{Display, Frame, PixelFormat, ScreenCapturer};

/// Wayland screen capture using wlr-screencopy protocol
pub struct WaylandCapturer {
    displays: Vec<Display>,
    current_display: u32,
    is_initialized: bool,
}

impl WaylandCapturer {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            displays: Vec::new(),
            current_display: 0,
            is_initialized: false,
        })
    }

    /// Get Wayland compositor info
    fn get_compositor_info(&self) -> Result<String> {
        let compositor = std::env::var("XDG_CURRENT_DESKTOP")
            .or_else(|_| std::env::var("WAYLAND_DISPLAY"))
            .unwrap_or_else(|_| "unknown".to_string());
        
        Ok(compositor)
    }

    /// Check if wlr-screencopy is available
    fn check_screencopy_available(&self) -> Result<bool> {
        // Try to run wl-copy to test if wl-clipboard tools are available
        let output = Command::new("wl-copy")
            .arg("--version")
            .output();
        
        match output {
            Ok(output) => Ok(output.status.success()),
            Err(_) => {
                // Check if grim is available (alternative screenshot tool)
                let grim_output = Command::new("grim")
                    .arg("--help")
                    .output();
                
                Ok(grim_output.is_ok())
            }
        }
    }

    /// Use grim for Wayland screenshot
    async fn capture_with_grim(&self) -> Result<Frame> {
        let output = Command::new("grim")
            .arg("-t")
            .arg("ppm") // Use PPM format for easier parsing
            .arg("-") // Output to stdout
            .output()
            .context("Failed to execute grim")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("grim failed: {}", stderr));
        }

        self.parse_ppm_data(&output.stdout)
    }

    /// Parse PPM image data
    fn parse_ppm_data(&self, data: &[u8]) -> Result<Frame> {
        // PPM format: P6\nwidth height\n255\n[binary RGB data]
        let header_end = data.windows(2).position(|w| w == b"\n\n" || w == b"\r\n")
            .context("Could not find PPM header end")?;
        
        let header = std::str::from_utf8(&data[..header_end])
            .context("Invalid PPM header")?;
        
        let lines: Vec<&str> = header.lines().collect();
        if lines.len() < 3 || lines[0] != "P6" {
            return Err(anyhow::anyhow!("Invalid PPM format"));
        }
        
        let dimensions: Vec<&str> = lines[1].split_whitespace().collect();
        if dimensions.len() != 2 {
            return Err(anyhow::anyhow!("Invalid PPM dimensions"));
        }
        
        let width: u32 = dimensions[0].parse().context("Invalid width")?;
        let height: u32 = dimensions[1].parse().context("Invalid height")?;
        
        // Skip header and get RGB data
        let rgb_data = &data[header_end + 2..];
        
        // Convert RGB to RGBA
        let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);
        for chunk in rgb_data.chunks(3) {
            if chunk.len() == 3 {
                rgba_data.push(chunk[0]); // R
                rgba_data.push(chunk[1]); // G
                rgba_data.push(chunk[2]); // B
                rgba_data.push(255);      // A
            }
        }
        
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        Ok(Frame {
            data: rgba_data,
            width,
            height,
            format: PixelFormat::RGBA,
            timestamp,
        })
    }

    /// Get display information using wlr-randr
    async fn detect_displays(&mut self) -> Result<()> {
        // Try wlr-randr first
        if let Ok(output) = Command::new("wlr-randr").output() {
            if output.status.success() {
                return self.parse_wlr_randr_output(&output.stdout);
            }
        }
        
        // Fallback: try swaymsg (for Sway compositor)
        if let Ok(output) = Command::new("swaymsg")
            .args(&["-t", "get_outputs"])
            .output() {
            if output.status.success() {
                return self.parse_sway_outputs(&output.stdout);
            }
        }
        
        // Final fallback: create a default display
        warn!("Could not detect Wayland displays, using default");
        self.displays.push(Display {
            id: 0,
            name: "Default".to_string(),
            width: 1920,
            height: 1080,
            x: 0,
            y: 0,
            is_primary: true,
            scale_factor: 1.0,
        });
        
        Ok(())
    }

    /// Parse wlr-randr output to get display info
    fn parse_wlr_randr_output(&mut self, output: &[u8]) -> Result<()> {
        let output_str = String::from_utf8_lossy(output);
        let mut displays = Vec::new();
        let mut current_display: Option<Display> = None;
        let mut display_id = 0;
        
        for line in output_str.lines() {
            let line = line.trim();
            
            if line.is_empty() {
                if let Some(display) = current_display.take() {
                    displays.push(display);
                }
                continue;
            }
            
            // Look for display name (usually starts the block)
            if !line.starts_with(" ") && line.contains("(") {
                if let Some(display) = current_display.take() {
                    displays.push(display);
                }
                
                let name = line.split('(').next().unwrap_or("Unknown").trim();
                current_display = Some(Display {
                    id: display_id,
                    name: name.to_string(),
                    width: 1920,
                    height: 1080,
                    x: 0,
                    y: 0,
                    is_primary: display_id == 0,
                    scale_factor: 1.0,
                });
                display_id += 1;
            }
            
            // Parse current mode line (contains resolution)
            if line.contains("current") && current_display.is_some() {
                if let Some(ref mut display) = current_display {
                    // Extract resolution from something like "1920x1080@60.000000Hz"
                    if let Some(res_part) = line.split_whitespace().next() {
                        if let Some((w, h)) = res_part.split_once('x') {
                            if let (Ok(width), Ok(height)) = (w.parse::<u32>(), h.split('@').next().unwrap_or("1080").parse::<u32>()) {
                                display.width = width;
                                display.height = height;
                            }
                        }
                    }
                }
            }
        }
        
        if let Some(display) = current_display {
            displays.push(display);
        }
        
        self.displays = displays;
        info!("Detected {} Wayland displays", self.displays.len());
        
        Ok(())
    }

    /// Parse Sway compositor output info
    fn parse_sway_outputs(&mut self, output: &[u8]) -> Result<()> {
        let output_str = String::from_utf8_lossy(output);
        
        // Parse JSON output from swaymsg
        if let Ok(outputs) = serde_json::from_str::<serde_json::Value>(&output_str) {
            if let Some(outputs_array) = outputs.as_array() {
                let mut displays = Vec::new();
                
                for (id, output) in outputs_array.iter().enumerate() {
                    if let (Some(name), Some(rect)) = (output.get("name"), output.get("rect")) {
                        let display = Display {
                            id: id as u32,
                            name: name.as_str().unwrap_or("Unknown").to_string(),
                            width: rect.get("width").and_then(|v| v.as_u64()).unwrap_or(1920) as u32,
                            height: rect.get("height").and_then(|v| v.as_u64()).unwrap_or(1080) as u32,
                            x: rect.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                            y: rect.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                            is_primary: output.get("primary").and_then(|v| v.as_bool()).unwrap_or(id == 0),
                            scale_factor: output.get("scale").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                        };
                        displays.push(display);
                    }
                }
                
                self.displays = displays;
                info!("Detected {} Sway displays", self.displays.len());
                return Ok(());
            }
        }
        
        Err(anyhow::anyhow!("Failed to parse Sway output information"))
    }
}

#[async_trait::async_trait]
impl ScreenCapturer for WaylandCapturer {
    async fn initialize(&mut self) -> Result<()> {
        info!("Initializing Wayland screen capture");
        
        // Check if we're actually running on Wayland
        if std::env::var("WAYLAND_DISPLAY").is_err() {
            return Err(anyhow::anyhow!("Not running on Wayland"));
        }
        
        let compositor = self.get_compositor_info()?;
        info!("Wayland compositor: {}", compositor);
        
        // Check if screencopy tools are available
        if !self.check_screencopy_available()? {
            return Err(anyhow::anyhow!(
                "No Wayland screencopy tools found. Please install 'grim' or 'wl-clipboard'"
            ));
        }
        
        // Detect available displays
        self.detect_displays().await?;
        
        self.is_initialized = true;
        info!("Wayland capturer initialized successfully");
        
        Ok(())
    }

    async fn capture_frame(&self) -> Result<Frame> {
        if !self.is_initialized {
            return Err(anyhow::anyhow!("Capturer not initialized"));
        }
        
        // For now, use grim for capture
        self.capture_with_grim().await
    }

    async fn get_displays(&self) -> Result<Vec<Display>> {
        Ok(self.displays.clone())
    }

    async fn set_display(&mut self, display_id: u32) -> Result<()> {
        if display_id as usize >= self.displays.len() {
            return Err(anyhow::anyhow!("Invalid display ID: {}", display_id));
        }
        
        self.current_display = display_id;
        info!("Set current display to: {}", display_id);
        
        Ok(())
    }

    fn get_resolution(&self) -> (u32, u32) {
        if let Some(display) = self.displays.get(self.current_display as usize) {
            (display.width, display.height)
        } else {
            (1920, 1080) // Default fallback
        }
    }

    fn is_healthy(&self) -> bool {
        self.is_initialized && !self.displays.is_empty()
    }

    async fn cleanup(&mut self) -> Result<()> {
        info!("Cleaning up Wayland capturer");
        self.is_initialized = false;
        Ok(())
    }
}
