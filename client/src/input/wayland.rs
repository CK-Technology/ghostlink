use async_trait::async_trait;
use tracing::{debug, error, info, warn};
use std::process::Command;
use crate::error::{Result, InputError, GhostLinkError};
use super::{InputController, KeyboardInput, MouseInput, MouseButton, KeyCode};

/// Wayland input controller using various compositor-specific tools
pub struct WaylandInputController {
    compositor: String,
    use_ydotool: bool,
    use_wlrctl: bool,
    is_initialized: bool,
}

impl WaylandInputController {
    pub async fn new() -> Result<Self> {
        info!("Initializing Wayland input controller");
        
        // Detect compositor type
        let compositor = super::super::capture::wayland::detect_compositor();
        info!("Detected Wayland compositor: {}", compositor);
        
        // Check available input tools
        let use_ydotool = Self::check_ydotool_available();
        let use_wlrctl = Self::check_wlrctl_available();
        
        if !use_ydotool && !use_wlrctl {
            warn!("No Wayland input tools available. Input control will be limited.");
            warn!("Consider installing ydotool or wlrctl for full functionality.");
        }
        
        Ok(Self {
            compositor,
            use_ydotool,
            use_wlrctl,
            is_initialized: false,
        })
    }
    
    /// Check if ydotool is available (universal Wayland input tool)
    fn check_ydotool_available() -> bool {
        // Check if ydotool is installed
        let result = Command::new("which")
            .arg("ydotool")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        
        if result {
            // Check if ydotoold daemon is running
            let daemon_check = Command::new("pgrep")
                .arg("ydotoold")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            
            if !daemon_check {
                warn!("ydotool is installed but ydotoold daemon is not running");
                warn!("Start it with: sudo ydotoold");
            }
            
            return daemon_check;
        }
        
        false
    }
    
    /// Check if wlrctl is available (for wlroots-based compositors)
    fn check_wlrctl_available() -> bool {
        Command::new("which")
            .arg("wlrctl")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    
    /// Send mouse movement using ydotool
    async fn move_mouse_ydotool(&self, x: i32, y: i32, absolute: bool) -> Result<()> {
        let args = if absolute {
            vec!["mousemove", "--absolute", &x.to_string(), &y.to_string()]
        } else {
            vec!["mousemove", &x.to_string(), &y.to_string()]
        };
        
        let output = Command::new("ydotool")
            .args(&args)
            .output()
            .map_err(|e| GhostLinkError::Input(InputError::InputBlocked))?;
        
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(GhostLinkError::Input(InputError::InputBlocked));
        }
        
        Ok(())
    }
    
    /// Send mouse movement using wlrctl
    async fn move_mouse_wlrctl(&self, x: i32, y: i32, absolute: bool) -> Result<()> {
        let args = if absolute {
            vec!["pointer", "move", &x.to_string(), &y.to_string()]
        } else {
            // wlrctl doesn't support relative movement directly
            // We'd need to get current position first
            return self.move_mouse_ydotool(x, y, absolute).await;
        };
        
        let output = Command::new("wlrctl")
            .args(&args)
            .output()
            .map_err(|e| GhostLinkError::Input(InputError::InputBlocked))?;
        
        if !output.status.success() {
            return Err(GhostLinkError::Input(InputError::InputBlocked));
        }
        
        Ok(())
    }
    
    /// Send mouse click using ydotool
    async fn click_mouse_ydotool(&self, button: MouseButton) -> Result<()> {
        let button_code = match button {
            MouseButton::Left => "1",
            MouseButton::Right => "2",
            MouseButton::Middle => "3",
        };
        
        let output = Command::new("ydotool")
            .args(&["click", button_code])
            .output()
            .map_err(|e| GhostLinkError::Input(InputError::InputBlocked))?;
        
        if !output.status.success() {
            return Err(GhostLinkError::Input(InputError::InputBlocked));
        }
        
        Ok(())
    }
    
    /// Send mouse click using wlrctl
    async fn click_mouse_wlrctl(&self, button: MouseButton) -> Result<()> {
        let button_name = match button {
            MouseButton::Left => "BTN_LEFT",
            MouseButton::Right => "BTN_RIGHT",
            MouseButton::Middle => "BTN_MIDDLE",
        };
        
        // Click is press + release
        let press_output = Command::new("wlrctl")
            .args(&["pointer", "click", button_name])
            .output()
            .map_err(|e| GhostLinkError::Input(InputError::InputBlocked))?;
        
        if !press_output.status.success() {
            return Err(GhostLinkError::Input(InputError::InputBlocked));
        }
        
        Ok(())
    }
    
    /// Send key press using ydotool
    async fn send_key_ydotool(&self, key: KeyCode, press: bool) -> Result<()> {
        let key_name = self.keycode_to_ydotool_name(key);
        
        let action = if press { "key" } else { "key" }; // ydotool handles both with same command
        
        let output = Command::new("ydotool")
            .args(&[action, &key_name])
            .output()
            .map_err(|e| GhostLinkError::Input(InputError::InputBlocked))?;
        
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            warn!("ydotool key failed: {}", error);
            return Err(GhostLinkError::Input(InputError::KeyMappingFailed {
                key: key_name,
            }));
        }
        
        Ok(())
    }
    
    /// Send key press using wlrctl
    async fn send_key_wlrctl(&self, key: KeyCode, press: bool) -> Result<()> {
        let key_name = self.keycode_to_wlrctl_name(key);
        let action = if press { "press" } else { "release" };
        
        let output = Command::new("wlrctl")
            .args(&["keyboard", action, &key_name])
            .output()
            .map_err(|e| GhostLinkError::Input(InputError::InputBlocked))?;
        
        if !output.status.success() {
            return Err(GhostLinkError::Input(InputError::KeyMappingFailed {
                key: key_name,
            }));
        }
        
        Ok(())
    }
    
    /// Convert KeyCode to ydotool key name
    fn keycode_to_ydotool_name(&self, key: KeyCode) -> String {
        match key {
            KeyCode::A => "a",
            KeyCode::B => "b",
            KeyCode::C => "c",
            KeyCode::D => "d",
            KeyCode::E => "e",
            KeyCode::F => "f",
            KeyCode::G => "g",
            KeyCode::H => "h",
            KeyCode::I => "i",
            KeyCode::J => "j",
            KeyCode::K => "k",
            KeyCode::L => "l",
            KeyCode::M => "m",
            KeyCode::N => "n",
            KeyCode::O => "o",
            KeyCode::P => "p",
            KeyCode::Q => "q",
            KeyCode::R => "r",
            KeyCode::S => "s",
            KeyCode::T => "t",
            KeyCode::U => "u",
            KeyCode::V => "v",
            KeyCode::W => "w",
            KeyCode::X => "x",
            KeyCode::Y => "y",
            KeyCode::Z => "z",
            KeyCode::Num0 => "0",
            KeyCode::Num1 => "1",
            KeyCode::Num2 => "2",
            KeyCode::Num3 => "3",
            KeyCode::Num4 => "4",
            KeyCode::Num5 => "5",
            KeyCode::Num6 => "6",
            KeyCode::Num7 => "7",
            KeyCode::Num8 => "8",
            KeyCode::Num9 => "9",
            KeyCode::Space => "space",
            KeyCode::Enter => "Return",
            KeyCode::Tab => "Tab",
            KeyCode::Escape => "Escape",
            KeyCode::Backspace => "BackSpace",
            KeyCode::Delete => "Delete",
            KeyCode::LeftShift => "shift",
            KeyCode::RightShift => "shift",
            KeyCode::LeftCtrl => "ctrl",
            KeyCode::RightCtrl => "ctrl",
            KeyCode::LeftAlt => "alt",
            KeyCode::RightAlt => "alt",
            KeyCode::LeftMeta => "Super_L",
            KeyCode::RightMeta => "Super_R",
            KeyCode::Up => "Up",
            KeyCode::Down => "Down",
            KeyCode::Left => "Left",
            KeyCode::Right => "Right",
            KeyCode::F1 => "F1",
            KeyCode::F2 => "F2",
            KeyCode::F3 => "F3",
            KeyCode::F4 => "F4",
            KeyCode::F5 => "F5",
            KeyCode::F6 => "F6",
            KeyCode::F7 => "F7",
            KeyCode::F8 => "F8",
            KeyCode::F9 => "F9",
            KeyCode::F10 => "F10",
            KeyCode::F11 => "F11",
            KeyCode::F12 => "F12",
            _ => "unknown",
        }.to_string()
    }
    
    /// Convert KeyCode to wlrctl key name
    fn keycode_to_wlrctl_name(&self, key: KeyCode) -> String {
        match key {
            KeyCode::A => "KEY_A",
            KeyCode::B => "KEY_B",
            KeyCode::C => "KEY_C",
            KeyCode::D => "KEY_D",
            KeyCode::E => "KEY_E",
            KeyCode::F => "KEY_F",
            KeyCode::G => "KEY_G",
            KeyCode::H => "KEY_H",
            KeyCode::I => "KEY_I",
            KeyCode::J => "KEY_J",
            KeyCode::K => "KEY_K",
            KeyCode::L => "KEY_L",
            KeyCode::M => "KEY_M",
            KeyCode::N => "KEY_N",
            KeyCode::O => "KEY_O",
            KeyCode::P => "KEY_P",
            KeyCode::Q => "KEY_Q",
            KeyCode::R => "KEY_R",
            KeyCode::S => "KEY_S",
            KeyCode::T => "KEY_T",
            KeyCode::U => "KEY_U",
            KeyCode::V => "KEY_V",
            KeyCode::W => "KEY_W",
            KeyCode::X => "KEY_X",
            KeyCode::Y => "KEY_Y",
            KeyCode::Z => "KEY_Z",
            KeyCode::Num0 => "KEY_0",
            KeyCode::Num1 => "KEY_1",
            KeyCode::Num2 => "KEY_2",
            KeyCode::Num3 => "KEY_3",
            KeyCode::Num4 => "KEY_4",
            KeyCode::Num5 => "KEY_5",
            KeyCode::Num6 => "KEY_6",
            KeyCode::Num7 => "KEY_7",
            KeyCode::Num8 => "KEY_8",
            KeyCode::Num9 => "KEY_9",
            KeyCode::Space => "KEY_SPACE",
            KeyCode::Enter => "KEY_ENTER",
            KeyCode::Tab => "KEY_TAB",
            KeyCode::Escape => "KEY_ESC",
            KeyCode::Backspace => "KEY_BACKSPACE",
            KeyCode::Delete => "KEY_DELETE",
            KeyCode::LeftShift => "KEY_LEFTSHIFT",
            KeyCode::RightShift => "KEY_RIGHTSHIFT",
            KeyCode::LeftCtrl => "KEY_LEFTCTRL",
            KeyCode::RightCtrl => "KEY_RIGHTCTRL",
            KeyCode::LeftAlt => "KEY_LEFTALT",
            KeyCode::RightAlt => "KEY_RIGHTALT",
            KeyCode::LeftMeta => "KEY_LEFTMETA",
            KeyCode::RightMeta => "KEY_RIGHTMETA",
            KeyCode::Up => "KEY_UP",
            KeyCode::Down => "KEY_DOWN",
            KeyCode::Left => "KEY_LEFT",
            KeyCode::Right => "KEY_RIGHT",
            KeyCode::F1 => "KEY_F1",
            KeyCode::F2 => "KEY_F2",
            KeyCode::F3 => "KEY_F3",
            KeyCode::F4 => "KEY_F4",
            KeyCode::F5 => "KEY_F5",
            KeyCode::F6 => "KEY_F6",
            KeyCode::F7 => "KEY_F7",
            KeyCode::F8 => "KEY_F8",
            KeyCode::F9 => "KEY_F9",
            KeyCode::F10 => "KEY_F10",
            KeyCode::F11 => "KEY_F11",
            KeyCode::F12 => "KEY_F12",
            _ => "KEY_UNKNOWN",
        }.to_string()
    }
}

#[async_trait]
impl InputController for WaylandInputController {
    async fn initialize(&mut self) -> Result<()> {
        info!("Initializing Wayland input controller for compositor: {}", self.compositor);
        
        // Check if we have at least one input method available
        if !self.use_ydotool && !self.use_wlrctl {
            return Err(GhostLinkError::Input(InputError::ToolMissing {
                tool: "ydotool or wlrctl".to_string(),
            }));
        }
        
        // Additional compositor-specific initialization could go here
        match self.compositor.as_str() {
            "sway" | "hyprland" => {
                info!("wlroots-based compositor detected, wlrctl should work");
            }
            "gnome" | "kde" => {
                info!("Desktop environment detected, portal-based input might be needed");
                warn!("Input support may be limited on {}", self.compositor);
            }
            _ => {
                warn!("Unknown compositor: {}, input support may be limited", self.compositor);
            }
        }
        
        self.is_initialized = true;
        Ok(())
    }
    
    async fn send_mouse_input(&mut self, input: MouseInput) -> Result<()> {
        if !self.is_initialized {
            return Err(GhostLinkError::Input(InputError::NotInitialized));
        }
        
        match input {
            MouseInput::Move { x, y } => {
                debug!("Moving mouse to ({}, {})", x, y);
                if self.use_ydotool {
                    self.move_mouse_ydotool(x, y, true).await
                } else if self.use_wlrctl {
                    self.move_mouse_wlrctl(x, y, true).await
                } else {
                    Err(GhostLinkError::Input(InputError::InputBlocked))
                }
            }
            MouseInput::Click { button, .. } => {
                debug!("Clicking mouse button: {:?}", button);
                if self.use_ydotool {
                    self.click_mouse_ydotool(button).await
                } else if self.use_wlrctl {
                    self.click_mouse_wlrctl(button).await
                } else {
                    Err(GhostLinkError::Input(InputError::InputBlocked))
                }
            }
            MouseInput::Scroll { delta } => {
                debug!("Scrolling by delta: {}", delta);
                if self.use_ydotool {
                    // ydotool scroll command
                    let direction = if delta > 0 { "up" } else { "down" };
                    let amount = delta.abs().to_string();
                    
                    Command::new("ydotool")
                        .args(&["wheel", direction, &amount])
                        .output()
                        .map_err(|_| GhostLinkError::Input(InputError::InputBlocked))?;
                    
                    Ok(())
                } else {
                    warn!("Scroll not implemented for wlrctl");
                    Ok(())
                }
            }
        }
    }
    
    async fn send_keyboard_input(&mut self, input: KeyboardInput) -> Result<()> {
        if !self.is_initialized {
            return Err(GhostLinkError::Input(InputError::NotInitialized));
        }
        
        match input {
            KeyboardInput::KeyPress { key } => {
                debug!("Pressing key: {:?}", key);
                if self.use_ydotool {
                    self.send_key_ydotool(key, true).await
                } else if self.use_wlrctl {
                    self.send_key_wlrctl(key, true).await
                } else {
                    Err(GhostLinkError::Input(InputError::InputBlocked))
                }
            }
            KeyboardInput::KeyRelease { key } => {
                debug!("Releasing key: {:?}", key);
                if self.use_ydotool {
                    // ydotool doesn't distinguish press/release for single keys
                    Ok(())
                } else if self.use_wlrctl {
                    self.send_key_wlrctl(key, false).await
                } else {
                    Err(GhostLinkError::Input(InputError::InputBlocked))
                }
            }
            KeyboardInput::Text { text } => {
                debug!("Typing text: {}", text);
                if self.use_ydotool {
                    Command::new("ydotool")
                        .args(&["type", &text])
                        .output()
                        .map_err(|_| GhostLinkError::Input(InputError::InputBlocked))?;
                    Ok(())
                } else {
                    // Fallback to typing each character
                    warn!("Text input not fully implemented for wlrctl");
                    Ok(())
                }
            }
        }
    }
    
    fn is_healthy(&self) -> bool {
        self.is_initialized && (self.use_ydotool || self.use_wlrctl)
    }
}