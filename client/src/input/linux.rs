use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs::File;
use std::os::unix::io::AsRawFd;
use tracing::{debug, error, info, warn};

use super::{InputHandler, KeyCode, MouseButton};

/// Wayland input handler using evdev and uinput
pub struct WaylandInputHandler {
    virtual_mouse: Option<File>,
    virtual_keyboard: Option<File>,
    input_blocked: bool,
    original_devices: Vec<String>,
}

impl WaylandInputHandler {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            virtual_mouse: None,
            virtual_keyboard: None,
            input_blocked: false,
            original_devices: Vec::new(),
        })
    }

    /// Create virtual input devices using uinput
    async fn create_virtual_devices(&mut self) -> Result<()> {
        // This would require the `evdev` and `uinput` crates
        // For now, we'll use a simpler approach with xdotool when available
        
        info!("Creating virtual input devices for Wayland");
        
        // TODO: Implement proper uinput virtual device creation
        // This requires root permissions or udev rules
        
        Ok(())
    }

    /// Use wtype for text input on Wayland
    async fn wayland_text_input(&self, text: &str) -> Result<()> {
        use std::process::Command;
        
        let output = Command::new("wtype")
            .arg(text)
            .output()
            .context("Failed to execute wtype")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("wtype failed: {}", stderr));
        }
        
        Ok(())
    }

    /// Use ydotool for input simulation on Wayland
    async fn ydotool_input(&self, args: &[&str]) -> Result<()> {
        use std::process::Command;
        
        let output = Command::new("ydotool")
            .args(args)
            .output()
            .context("Failed to execute ydotool")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("ydotool failed: {}", stderr));
        }
        
        Ok(())
    }

    /// Block input by grabbing input devices
    async fn block_wayland_input(&mut self) -> Result<()> {
        // This is complex on Wayland and typically requires compositor support
        // For now, we'll implement a basic approach
        
        warn!("Input blocking on Wayland is limited without compositor support");
        
        // TODO: Implement proper input blocking
        // This might require:
        // 1. Creating an overlay window to capture all input
        // 2. Using compositor-specific protocols
        // 3. Temporarily disabling input devices via udev
        
        self.input_blocked = true;
        Ok(())
    }

    /// Convert mouse button to ydotool format
    fn mouse_button_to_ydotool(&self, button: MouseButton) -> &'static str {
        match button {
            MouseButton::Left => "0x40001", // BTN_LEFT
            MouseButton::Right => "0x40002", // BTN_RIGHT
            MouseButton::Middle => "0x40004", // BTN_MIDDLE
            MouseButton::X1 => "0x40008", // BTN_SIDE
            MouseButton::X2 => "0x40010", // BTN_EXTRA
        }
    }

    /// Convert key code to Linux key code
    fn keycode_to_linux(&self, key: KeyCode) -> Option<u32> {
        // Linux input event codes (from linux/input-event-codes.h)
        match key {
            KeyCode::A => Some(30),
            KeyCode::B => Some(48),
            KeyCode::C => Some(46),
            KeyCode::D => Some(32),
            KeyCode::E => Some(18),
            KeyCode::F => Some(33),
            KeyCode::G => Some(34),
            KeyCode::H => Some(35),
            KeyCode::I => Some(23),
            KeyCode::J => Some(36),
            KeyCode::K => Some(37),
            KeyCode::L => Some(38),
            KeyCode::M => Some(50),
            KeyCode::N => Some(49),
            KeyCode::O => Some(24),
            KeyCode::P => Some(25),
            KeyCode::Q => Some(16),
            KeyCode::R => Some(19),
            KeyCode::S => Some(31),
            KeyCode::T => Some(20),
            KeyCode::U => Some(22),
            KeyCode::V => Some(47),
            KeyCode::W => Some(17),
            KeyCode::X => Some(45),
            KeyCode::Y => Some(21),
            KeyCode::Z => Some(44),
            
            KeyCode::Key0 => Some(11),
            KeyCode::Key1 => Some(2),
            KeyCode::Key2 => Some(3),
            KeyCode::Key3 => Some(4),
            KeyCode::Key4 => Some(5),
            KeyCode::Key5 => Some(6),
            KeyCode::Key6 => Some(7),
            KeyCode::Key7 => Some(8),
            KeyCode::Key8 => Some(9),
            KeyCode::Key9 => Some(10),
            
            KeyCode::Space => Some(57),
            KeyCode::Enter => Some(28),
            KeyCode::Tab => Some(15),
            KeyCode::Backspace => Some(14),
            KeyCode::Delete => Some(111),
            KeyCode::Escape => Some(1),
            
            KeyCode::Shift => Some(42), // Left shift
            KeyCode::Ctrl => Some(29),  // Left ctrl
            KeyCode::Alt => Some(56),   // Left alt
            KeyCode::Super => Some(125), // Left super
            
            KeyCode::Up => Some(103),
            KeyCode::Down => Some(108),
            KeyCode::Left => Some(105),
            KeyCode::Right => Some(106),
            
            KeyCode::F1 => Some(59),
            KeyCode::F2 => Some(60),
            KeyCode::F3 => Some(61),
            KeyCode::F4 => Some(62),
            KeyCode::F5 => Some(63),
            KeyCode::F6 => Some(64),
            KeyCode::F7 => Some(65),
            KeyCode::F8 => Some(66),
            KeyCode::F9 => Some(67),
            KeyCode::F10 => Some(68),
            KeyCode::F11 => Some(87),
            KeyCode::F12 => Some(88),
            
            KeyCode::Raw(code) => Some(code),
            
            _ => {
                warn!("Unhandled key code: {:?}", key);
                None
            }
        }
    }
}

#[async_trait::async_trait]
impl InputHandler for WaylandInputHandler {
    async fn initialize(&mut self) -> Result<()> {
        info!("Initializing Wayland input handler");
        
        // Check if we're actually on Wayland
        if std::env::var("WAYLAND_DISPLAY").is_err() {
            return Err(anyhow::anyhow!("Not running on Wayland"));
        }
        
        // Check for required tools
        let tools = ["ydotool", "wtype"];
        for tool in &tools {
            if std::process::Command::new(tool)
                .arg("--help")
                .output()
                .is_err() {
                warn!("Tool '{}' not found - some input features may not work", tool);
            }
        }
        
        // Create virtual devices if we have permissions
        if let Err(e) = self.create_virtual_devices().await {
            warn!("Could not create virtual devices: {} - falling back to external tools", e);
        }
        
        info!("Wayland input handler initialized");
        Ok(())
    }

    async fn handle_mouse_move(&self, x: i32, y: i32) -> Result<()> {
        if self.input_blocked {
            return Ok(());
        }
        
        debug!("Mouse move: ({}, {})", x, y);
        
        // Use ydotool for mouse movement
        self.ydotool_input(&["mousemove", "--absolute", &x.to_string(), &y.to_string()]).await?;
        
        Ok(())
    }

    async fn handle_mouse_button(&self, button: MouseButton, pressed: bool) -> Result<()> {
        if self.input_blocked {
            return Ok(());
        }
        
        debug!("Mouse button {:?}: {}", button, if pressed { "down" } else { "up" });
        
        let button_code = self.mouse_button_to_ydotool(button);
        let action = if pressed { "1" } else { "0" };
        
        self.ydotool_input(&["click", button_code, action]).await?;
        
        Ok(())
    }

    async fn handle_mouse_scroll(&self, delta_x: i32, delta_y: i32) -> Result<()> {
        if self.input_blocked {
            return Ok(());
        }
        
        debug!("Mouse scroll: ({}, {})", delta_x, delta_y);
        
        // ydotool scroll (vertical)
        if delta_y != 0 {
            let direction = if delta_y > 0 { "4" } else { "5" }; // Scroll up/down
            for _ in 0..delta_y.abs() {
                self.ydotool_input(&["click", direction]).await?;
            }
        }
        
        // Horizontal scroll (if supported)
        if delta_x != 0 {
            let direction = if delta_x > 0 { "6" } else { "7" };
            for _ in 0..delta_x.abs() {
                self.ydotool_input(&["click", direction]).await?;
            }
        }
        
        Ok(())
    }

    async fn handle_key_event(&self, key: KeyCode, pressed: bool) -> Result<()> {
        if self.input_blocked {
            return Ok(());
        }
        
        debug!("Key {:?}: {}", key, if pressed { "down" } else { "up" });
        
        if let Some(linux_key) = self.keycode_to_linux(key) {
            let action = if pressed { "1" } else { "0" };
            self.ydotool_input(&["key", &format!("{}:{}", linux_key, action)]).await?;
        } else {
            warn!("Could not map key {:?} to Linux key code", key);
        }
        
        Ok(())
    }

    async fn handle_text_input(&self, text: &str) -> Result<()> {
        if self.input_blocked {
            return Ok(());
        }
        
        debug!("Text input: '{}'", text);
        
        // Use wtype for text input on Wayland
        self.wayland_text_input(text).await?;
        
        Ok(())
    }

    async fn block_user_input(&self) -> Result<()> {
        info!("Blocking user input on Wayland");
        
        // This is challenging on Wayland without compositor cooperation
        // We would need to:
        // 1. Create a fullscreen overlay window
        // 2. Grab all input devices
        // 3. Use compositor-specific protocols
        
        warn!("Input blocking on Wayland requires compositor support");
        Ok(())
    }

    async fn unblock_user_input(&self) -> Result<()> {
        info!("Unblocking user input on Wayland");
        Ok(())
    }

    fn is_input_blocked(&self) -> bool {
        self.input_blocked
    }

    fn is_healthy(&self) -> bool {
        true // Basic health check - could be more sophisticated
    }

    async fn cleanup(&mut self) -> Result<()> {
        info!("Cleaning up Wayland input handler");
        
        if self.input_blocked {
            let _ = self.unblock_user_input().await;
        }
        
        // Close virtual devices
        self.virtual_mouse = None;
        self.virtual_keyboard = None;
        
        Ok(())
    }
}

/// X11 input handler (fallback for X11 sessions)
pub struct X11InputHandler {
    input_blocked: bool,
}

impl X11InputHandler {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            input_blocked: false,
        })
    }

    /// Use xdotool for X11 input simulation
    async fn xdotool_input(&self, args: &[&str]) -> Result<()> {
        use std::process::Command;
        
        let output = Command::new("xdotool")
            .args(args)
            .output()
            .context("Failed to execute xdotool")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("xdotool failed: {}", stderr));
        }
        
        Ok(())
    }
}

#[async_trait::async_trait]
impl InputHandler for X11InputHandler {
    async fn initialize(&mut self) -> Result<()> {
        info!("Initializing X11 input handler");
        
        // Check if xdotool is available
        if std::process::Command::new("xdotool")
            .arg("--version")
            .output()
            .is_err() {
            return Err(anyhow::anyhow!("xdotool not found - required for X11 input control"));
        }
        
        info!("X11 input handler initialized");
        Ok(())
    }

    async fn handle_mouse_move(&self, x: i32, y: i32) -> Result<()> {
        if self.input_blocked {
            return Ok(());
        }
        
        self.xdotool_input(&["mousemove", &x.to_string(), &y.to_string()]).await
    }

    async fn handle_mouse_button(&self, button: MouseButton, pressed: bool) -> Result<()> {
        if self.input_blocked {
            return Ok(());
        }
        
        let button_num = match button {
            MouseButton::Left => "1",
            MouseButton::Middle => "2",
            MouseButton::Right => "3",
            MouseButton::X1 => "8",
            MouseButton::X2 => "9",
        };
        
        if pressed {
            self.xdotool_input(&["mousedown", button_num]).await
        } else {
            self.xdotool_input(&["mouseup", button_num]).await
        }
    }

    async fn handle_mouse_scroll(&self, _delta_x: i32, delta_y: i32) -> Result<()> {
        if self.input_blocked {
            return Ok(());
        }
        
        if delta_y > 0 {
            for _ in 0..delta_y {
                self.xdotool_input(&["click", "4"]).await?; // Scroll up
            }
        } else if delta_y < 0 {
            for _ in 0..(-delta_y) {
                self.xdotool_input(&["click", "5"]).await?; // Scroll down
            }
        }
        
        Ok(())
    }

    async fn handle_key_event(&self, key: KeyCode, pressed: bool) -> Result<()> {
        if self.input_blocked {
            return Ok(());
        }
        
        // Convert to xdotool key names
        let key_name = match key {
            KeyCode::Space => "space",
            KeyCode::Enter => "Return",
            KeyCode::Tab => "Tab",
            KeyCode::Backspace => "BackSpace",
            KeyCode::Delete => "Delete",
            KeyCode::Escape => "Escape",
            KeyCode::Shift => "shift",
            KeyCode::Ctrl => "ctrl",
            KeyCode::Alt => "alt",
            KeyCode::Super => "Super_L",
            KeyCode::Up => "Up",
            KeyCode::Down => "Down",
            KeyCode::Left => "Left",
            KeyCode::Right => "Right",
            // Add more mappings as needed
            _ => {
                warn!("Unhandled key for X11: {:?}", key);
                return Ok(());
            }
        };
        
        if pressed {
            self.xdotool_input(&["keydown", key_name]).await
        } else {
            self.xdotool_input(&["keyup", key_name]).await
        }
    }

    async fn handle_text_input(&self, text: &str) -> Result<()> {
        if self.input_blocked {
            return Ok(());
        }
        
        self.xdotool_input(&["type", text]).await
    }

    async fn block_user_input(&self) -> Result<()> {
        info!("Blocking user input on X11");
        // TODO: Implement X11 input blocking using xinput
        Ok(())
    }

    async fn unblock_user_input(&self) -> Result<()> {
        info!("Unblocking user input on X11");
        // TODO: Implement X11 input unblocking
        Ok(())
    }

    fn is_input_blocked(&self) -> bool {
        self.input_blocked
    }

    fn is_healthy(&self) -> bool {
        true
    }

    async fn cleanup(&mut self) -> Result<()> {
        info!("Cleaning up X11 input handler");
        
        if self.input_blocked {
            let _ = self.unblock_user_input().await;
        }
        
        Ok(())
    }
}
