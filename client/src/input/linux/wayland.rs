use anyhow::Result;
use std::process::Command;
use tracing::{debug, error, info, warn};

use crate::input::{InputHandler, KeyCode, MouseButton};

/// Wayland input handler using external tools
pub struct WaylandInputHandler {
    is_healthy: bool,
    input_blocked: bool,
}

impl WaylandInputHandler {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            is_healthy: true,
            input_blocked: false,
        })
    }

    /// Check if required tools are available
    fn check_tools() -> Result<()> {
        // Check for ydotool (for input simulation)
        let ydotool_check = Command::new("which")
            .arg("ydotool")
            .output();
        
        if ydotool_check.is_err() || !ydotool_check.unwrap().status.success() {
            return Err(anyhow::anyhow!("ydotool not found. Please install ydotool for Wayland input support"));
        }

        Ok(())
    }

    /// Convert mouse button to ydotool format
    fn mouse_button_to_ydotool(&self, button: MouseButton) -> &str {
        match button {
            MouseButton::Left => "0x110",
            MouseButton::Right => "0x111", 
            MouseButton::Middle => "0x112",
            MouseButton::X1 => "0x113",
            MouseButton::X2 => "0x114",
        }
    }

    /// Convert key code to ydotool format
    fn keycode_to_ydotool(&self, key: KeyCode) -> Option<String> {
        match key {
            // Letters (lowercase)
            KeyCode::A => Some("0x1e".to_string()),
            KeyCode::B => Some("0x30".to_string()),
            KeyCode::C => Some("0x2e".to_string()),
            KeyCode::D => Some("0x20".to_string()),
            KeyCode::E => Some("0x12".to_string()),
            KeyCode::F => Some("0x21".to_string()),
            KeyCode::G => Some("0x22".to_string()),
            KeyCode::H => Some("0x23".to_string()),
            KeyCode::I => Some("0x17".to_string()),
            KeyCode::J => Some("0x24".to_string()),
            KeyCode::K => Some("0x25".to_string()),
            KeyCode::L => Some("0x26".to_string()),
            KeyCode::M => Some("0x32".to_string()),
            KeyCode::N => Some("0x31".to_string()),
            KeyCode::O => Some("0x18".to_string()),
            KeyCode::P => Some("0x19".to_string()),
            KeyCode::Q => Some("0x10".to_string()),
            KeyCode::R => Some("0x13".to_string()),
            KeyCode::S => Some("0x1f".to_string()),
            KeyCode::T => Some("0x14".to_string()),
            KeyCode::U => Some("0x16".to_string()),
            KeyCode::V => Some("0x2f".to_string()),
            KeyCode::W => Some("0x11".to_string()),
            KeyCode::X => Some("0x2d".to_string()),
            KeyCode::Y => Some("0x15".to_string()),
            KeyCode::Z => Some("0x2c".to_string()),
            
            // Numbers
            KeyCode::Key0 => Some("0x0b".to_string()),
            KeyCode::Key1 => Some("0x02".to_string()),
            KeyCode::Key2 => Some("0x03".to_string()),
            KeyCode::Key3 => Some("0x04".to_string()),
            KeyCode::Key4 => Some("0x05".to_string()),
            KeyCode::Key5 => Some("0x06".to_string()),
            KeyCode::Key6 => Some("0x07".to_string()),
            KeyCode::Key7 => Some("0x08".to_string()),
            KeyCode::Key8 => Some("0x09".to_string()),
            KeyCode::Key9 => Some("0x0a".to_string()),
            
            // Special keys
            KeyCode::Space => Some("0x39".to_string()),
            KeyCode::Enter => Some("0x1c".to_string()),
            KeyCode::Tab => Some("0x0f".to_string()),
            KeyCode::Backspace => Some("0x0e".to_string()),
            KeyCode::Delete => Some("0x53".to_string()),
            KeyCode::Escape => Some("0x01".to_string()),
            
            // Arrow keys
            KeyCode::Up => Some("0x67".to_string()),
            KeyCode::Down => Some("0x6c".to_string()),
            KeyCode::Left => Some("0x69".to_string()),
            KeyCode::Right => Some("0x6a".to_string()),
            
            // Modifiers
            KeyCode::Shift => Some("0x2a".to_string()),
            KeyCode::Ctrl => Some("0x1d".to_string()),
            KeyCode::Alt => Some("0x38".to_string()),
            KeyCode::Super => Some("0x7d".to_string()),
            
            // Function keys
            KeyCode::F1 => Some("0x3b".to_string()),
            KeyCode::F2 => Some("0x3c".to_string()),
            KeyCode::F3 => Some("0x3d".to_string()),
            KeyCode::F4 => Some("0x3e".to_string()),
            KeyCode::F5 => Some("0x3f".to_string()),
            KeyCode::F6 => Some("0x40".to_string()),
            KeyCode::F7 => Some("0x41".to_string()),
            KeyCode::F8 => Some("0x42".to_string()),
            KeyCode::F9 => Some("0x43".to_string()),
            KeyCode::F10 => Some("0x44".to_string()),
            KeyCode::F11 => Some("0x57".to_string()),
            KeyCode::F12 => Some("0x58".to_string()),
            
            KeyCode::Raw(code) => Some(format!("0x{:x}", code)),
            _ => {
                warn!("Unmapped key code: {:?}", key);
                None
            }
        }
    }
}

#[async_trait::async_trait]
impl InputHandler for WaylandInputHandler {
    async fn initialize(&mut self) -> Result<()> {
        info!("Initializing Wayland input handler");
        
        // Check if required tools are available
        if let Err(e) = Self::check_tools() {
            error!("Failed to initialize Wayland input handler: {}", e);
            self.is_healthy = false;
            return Err(e);
        }
        
        self.is_healthy = true;
        info!("Wayland input handler initialized successfully");
        Ok(())
    }

    async fn handle_mouse_move(&self, x: i32, y: i32) -> Result<()> {
        debug!("Moving mouse to ({}, {})", x, y);
        
        let output = Command::new("ydotool")
            .args(&["mousemove", &format!("{}", x), &format!("{}", y)])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to execute ydotool mousemove: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("ydotool mousemove failed: {}", stderr));
        }

        Ok(())
    }

    async fn handle_mouse_button(&self, button: MouseButton, pressed: bool) -> Result<()> {
        debug!("Mouse button {:?} {}", button, if pressed { "pressed" } else { "released" });
        
        let button_code = self.mouse_button_to_ydotool(button);
        let action = if pressed { "1" } else { "0" };
        
        let output = Command::new("ydotool")
            .args(&["click", button_code, action])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to execute ydotool click: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("ydotool click failed: {}", stderr));
        }

        Ok(())
    }

    async fn handle_mouse_scroll(&self, delta_x: i32, delta_y: i32) -> Result<()> {
        debug!("Mouse scroll delta: ({}, {})", delta_x, delta_y);
        
        if delta_y != 0 {
            let scroll_direction = if delta_y > 0 { "4" } else { "5" };
            
            let output = Command::new("ydotool")
                .args(&["click", scroll_direction])
                .output()
                .map_err(|e| anyhow::anyhow!("Failed to execute ydotool scroll: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("ydotool scroll failed: {}", stderr));
            }
        }

        // Note: Horizontal scrolling is more complex with ydotool
        if delta_x != 0 {
            warn!("Horizontal scrolling not fully supported with ydotool");
        }

        Ok(())
    }

    async fn handle_key_event(&self, key: KeyCode, pressed: bool) -> Result<()> {
        debug!("Key {:?} {}", key, if pressed { "pressed" } else { "released" });
        
        if let Some(key_code) = self.keycode_to_ydotool(key) {
            let action = if pressed { "1" } else { "0" };
            
            let output = Command::new("ydotool")
                .args(&["key", &format!("{}:{}", key_code, action)])
                .output()
                .map_err(|e| anyhow::anyhow!("Failed to execute ydotool key: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("ydotool key failed: {}", stderr));
            }
        } else {
            warn!("Cannot map key {:?} to ydotool format", key);
        }

        Ok(())
    }

    async fn handle_text_input(&self, text: &str) -> Result<()> {
        debug!("Typing text: {}", text);
        
        let output = Command::new("ydotool")
            .args(&["type", text])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to execute ydotool type: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("ydotool type failed: {}", stderr));
        }

        Ok(())
    }

    async fn block_user_input(&self) -> Result<()> {
        warn!("Input blocking not implemented for Wayland - this is complex and requires root access");
        // TODO: Implement input blocking using udev rules or similar
        Ok(())
    }

    async fn unblock_user_input(&self) -> Result<()> {
        warn!("Input unblocking not implemented for Wayland");
        Ok(())
    }

    fn is_input_blocked(&self) -> bool {
        self.input_blocked
    }

    fn is_healthy(&self) -> bool {
        self.is_healthy
    }

    async fn cleanup(&mut self) -> Result<()> {
        info!("Cleaning up Wayland input handler");
        // No cleanup needed for external tool approach
        Ok(())
    }
}