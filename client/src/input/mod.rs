use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::session::SessionType;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

/// Cross-platform input control
pub struct InputController {
    controller: InputHandlerEnum,
    is_input_blocked: Arc<RwLock<bool>>,
    session_type: SessionType,
}

/// Enum to hold different input handler implementations
pub enum InputHandlerEnum {
    #[cfg(target_os = "linux")]
    WaylandInput(linux::WaylandInputHandler),
    #[cfg(target_os = "linux")]
    X11Input(linux::X11InputHandler),
    #[cfg(target_os = "windows")]
    WindowsInput(windows::WindowsInputHandler),
    #[cfg(target_os = "macos")]
    MacInput(macos::MacInputHandler),
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    Placeholder,
}

impl InputHandlerEnum {
    /// Initialize the input handler
    pub async fn initialize(&mut self) -> Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            InputHandlerEnum::WaylandInput(handler) => handler.initialize().await,
            #[cfg(target_os = "linux")]
            InputHandlerEnum::X11Input(handler) => handler.initialize().await,
            #[cfg(target_os = "windows")]
            InputHandlerEnum::WindowsInput(handler) => handler.initialize().await,
            #[cfg(target_os = "macos")]
            InputHandlerEnum::MacInput(handler) => handler.initialize().await,
            #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
            InputHandlerEnum::Placeholder => Ok(()),
        }
    }
    
    /// Handle mouse movement
    pub async fn handle_mouse_move(&self, x: i32, y: i32) -> Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            InputHandlerEnum::WaylandInput(handler) => handler.handle_mouse_move(x, y).await,
            #[cfg(target_os = "linux")]
            InputHandlerEnum::X11Input(handler) => handler.handle_mouse_move(x, y).await,
            #[cfg(target_os = "windows")]
            InputHandlerEnum::WindowsInput(handler) => handler.handle_mouse_move(x, y).await,
            #[cfg(target_os = "macos")]
            InputHandlerEnum::MacInput(handler) => handler.handle_mouse_move(x, y).await,
            #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
            InputHandlerEnum::Placeholder => Ok(()),
        }
    }
    
    /// Handle mouse button press/release
    pub async fn handle_mouse_button(&self, button: MouseButton, pressed: bool) -> Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            InputHandlerEnum::WaylandInput(handler) => handler.handle_mouse_button(button, pressed).await,
            #[cfg(target_os = "linux")]
            InputHandlerEnum::X11Input(handler) => handler.handle_mouse_button(button, pressed).await,
            #[cfg(target_os = "windows")]
            InputHandlerEnum::WindowsInput(handler) => handler.handle_mouse_button(button, pressed).await,
            #[cfg(target_os = "macos")]
            InputHandlerEnum::MacInput(handler) => handler.handle_mouse_button(button, pressed).await,
            #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
            InputHandlerEnum::Placeholder => Ok(()),
        }
    }
    
    /// Handle mouse scroll
    pub async fn handle_mouse_scroll(&self, delta_x: i32, delta_y: i32) -> Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            InputHandlerEnum::WaylandInput(handler) => handler.handle_mouse_scroll(delta_x, delta_y).await,
            #[cfg(target_os = "linux")]
            InputHandlerEnum::X11Input(handler) => handler.handle_mouse_scroll(delta_x, delta_y).await,
            #[cfg(target_os = "windows")]
            InputHandlerEnum::WindowsInput(handler) => handler.handle_mouse_scroll(delta_x, delta_y).await,
            #[cfg(target_os = "macos")]
            InputHandlerEnum::MacInput(handler) => handler.handle_mouse_scroll(delta_x, delta_y).await,
            #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
            InputHandlerEnum::Placeholder => Ok(()),
        }
    }
    
    /// Handle keyboard key press/release
    pub async fn handle_key_event(&self, key: KeyCode, pressed: bool) -> Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            InputHandlerEnum::WaylandInput(handler) => handler.handle_key_event(key, pressed).await,
            #[cfg(target_os = "linux")]
            InputHandlerEnum::X11Input(handler) => handler.handle_key_event(key, pressed).await,
            #[cfg(target_os = "windows")]
            InputHandlerEnum::WindowsInput(handler) => handler.handle_key_event(key, pressed).await,
            #[cfg(target_os = "macos")]
            InputHandlerEnum::MacInput(handler) => handler.handle_key_event(key, pressed).await,
            #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
            InputHandlerEnum::Placeholder => Ok(()),
        }
    }
    
    /// Handle text input
    pub async fn handle_text_input(&self, text: &str) -> Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            InputHandlerEnum::WaylandInput(handler) => handler.handle_text_input(text).await,
            #[cfg(target_os = "linux")]
            InputHandlerEnum::X11Input(handler) => handler.handle_text_input(text).await,
            #[cfg(target_os = "windows")]
            InputHandlerEnum::WindowsInput(handler) => handler.handle_text_input(text).await,
            #[cfg(target_os = "macos")]
            InputHandlerEnum::MacInput(handler) => handler.handle_text_input(text).await,
            #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
            InputHandlerEnum::Placeholder => Ok(()),
        }
    }
    
    /// Block user input (for screen blanking scenarios)
    pub async fn block_user_input(&self) -> Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            InputHandlerEnum::WaylandInput(handler) => handler.block_user_input().await,
            #[cfg(target_os = "linux")]
            InputHandlerEnum::X11Input(handler) => handler.block_user_input().await,
            #[cfg(target_os = "windows")]
            InputHandlerEnum::WindowsInput(handler) => handler.block_user_input().await,
            #[cfg(target_os = "macos")]
            InputHandlerEnum::MacInput(handler) => handler.block_user_input().await,
            #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
            InputHandlerEnum::Placeholder => Ok(()),
        }
    }
    
    /// Unblock user input
    pub async fn unblock_user_input(&self) -> Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            InputHandlerEnum::WaylandInput(handler) => handler.unblock_user_input().await,
            #[cfg(target_os = "linux")]
            InputHandlerEnum::X11Input(handler) => handler.unblock_user_input().await,
            #[cfg(target_os = "windows")]
            InputHandlerEnum::WindowsInput(handler) => handler.unblock_user_input().await,
            #[cfg(target_os = "macos")]
            InputHandlerEnum::MacInput(handler) => handler.unblock_user_input().await,
            #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
            InputHandlerEnum::Placeholder => Ok(()),
        }
    }
    
    /// Check if input is currently blocked
    pub fn is_input_blocked(&self) -> bool {
        match self {
            #[cfg(target_os = "linux")]
            InputHandlerEnum::WaylandInput(handler) => handler.is_input_blocked(),
            #[cfg(target_os = "linux")]
            InputHandlerEnum::X11Input(handler) => handler.is_input_blocked(),
            #[cfg(target_os = "windows")]
            InputHandlerEnum::WindowsInput(handler) => handler.is_input_blocked(),
            #[cfg(target_os = "macos")]
            InputHandlerEnum::MacInput(handler) => handler.is_input_blocked(),
            #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
            InputHandlerEnum::Placeholder => false,
        }
    }
    
    /// Check if handler is healthy
    pub fn is_healthy(&self) -> bool {
        match self {
            #[cfg(target_os = "linux")]
            InputHandlerEnum::WaylandInput(handler) => handler.is_healthy(),
            #[cfg(target_os = "linux")]
            InputHandlerEnum::X11Input(handler) => handler.is_healthy(),
            #[cfg(target_os = "windows")]
            InputHandlerEnum::WindowsInput(handler) => handler.is_healthy(),
            #[cfg(target_os = "macos")]
            InputHandlerEnum::MacInput(handler) => handler.is_healthy(),
            #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
            InputHandlerEnum::Placeholder => true,
        }
    }
    
    /// Cleanup resources
    pub async fn cleanup(&mut self) -> Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            InputHandlerEnum::WaylandInput(handler) => handler.cleanup().await,
            #[cfg(target_os = "linux")]
            InputHandlerEnum::X11Input(handler) => handler.cleanup().await,
            #[cfg(target_os = "windows")]
            InputHandlerEnum::WindowsInput(handler) => handler.cleanup().await,
            #[cfg(target_os = "macos")]
            InputHandlerEnum::MacInput(handler) => handler.cleanup().await,
            #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
            InputHandlerEnum::Placeholder => Ok(()),
        }
    }
}

/// Platform-specific input handling trait
#[async_trait::async_trait]
pub trait InputHandler: Send + Sync {
    /// Initialize the input handler
    async fn initialize(&mut self) -> Result<()>;
    
    /// Handle mouse movement
    async fn handle_mouse_move(&self, x: i32, y: i32) -> Result<()>;
    
    /// Handle mouse button press/release
    async fn handle_mouse_button(&self, button: MouseButton, pressed: bool) -> Result<()>;
    
    /// Handle mouse scroll
    async fn handle_mouse_scroll(&self, delta_x: i32, delta_y: i32) -> Result<()>;
    
    /// Handle keyboard key press/release
    async fn handle_key_event(&self, key: KeyCode, pressed: bool) -> Result<()>;
    
    /// Handle text input
    async fn handle_text_input(&self, text: &str) -> Result<()>;
    
    /// Block user input (for screen blanking scenarios)
    async fn block_user_input(&self) -> Result<()>;
    
    /// Unblock user input
    async fn unblock_user_input(&self) -> Result<()>;
    
    /// Check if input is currently blocked
    fn is_input_blocked(&self) -> bool;
    
    /// Check if handler is healthy
    fn is_healthy(&self) -> bool;
    
    /// Cleanup resources
    async fn cleanup(&mut self) -> Result<()>;
}

/// Mouse button enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

/// Keyboard key codes (simplified for cross-platform use)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyCode {
    // Letters
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    
    // Numbers
    Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9,
    
    // Function keys
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    
    // Modifiers
    Shift, Ctrl, Alt, Super,
    
    // Navigation
    Up, Down, Left, Right,
    Home, End, PageUp, PageDown,
    
    // Special keys
    Space, Enter, Tab, Backspace, Delete, Escape,
    
    // Numpad
    Numpad0, Numpad1, Numpad2, Numpad3, Numpad4,
    Numpad5, Numpad6, Numpad7, Numpad8, Numpad9,
    NumpadEnter, NumpadPlus, NumpadMinus, NumpadMultiply, NumpadDivide,
    
    // Other
    CapsLock, NumLock, ScrollLock,
    PrintScreen, Pause, Insert,
    
    // Raw key code for unmapped keys
    Raw(u32),
}

/// Input event from remote operator
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InputEvent {
    MouseMove { x: i32, y: i32 },
    MouseButton { button: MouseButton, pressed: bool },
    MouseScroll { delta_x: i32, delta_y: i32 },
    KeyEvent { key: KeyCode, pressed: bool },
    TextInput { text: String },
}

impl InputController {
    /// Create new input controller
    pub async fn new(session_type: SessionType) -> Result<Self> {
        let mut controller = Self::create_platform_controller().await?;
        
        let mut input_controller = Self {
            controller,
            is_input_blocked: Arc::new(RwLock::new(false)),
            session_type,
        };
        
        input_controller.initialize().await?;
        
        Ok(input_controller)
    }

    /// Create platform-specific input controller
    async fn create_platform_controller() -> Result<InputHandlerEnum> {
        #[cfg(target_os = "linux")]
        {
            if std::env::var("WAYLAND_DISPLAY").is_ok() {
                info!("Using Wayland input control");
                Ok(InputHandlerEnum::WaylandInput(linux::WaylandInputHandler::new().await?))
            } else {
                info!("Using X11 input control");
                Ok(InputHandlerEnum::X11Input(linux::X11InputHandler::new().await?))
            }
        }
        
        #[cfg(target_os = "windows")]
        {
            info!("Using Windows input control");
            Ok(InputHandlerEnum::WindowsInput(windows::WindowsInputHandler::new().await?))
        }
        
        #[cfg(target_os = "macos")]
        {
            info!("Using macOS input control");
            Ok(InputHandlerEnum::MacInput(macos::MacInputHandler::new().await?))
        }
        
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            Ok(InputHandlerEnum::Placeholder)
        }
    }

    /// Initialize input controller
    async fn initialize(&mut self) -> Result<()> {
        info!("Initializing input controller");
        self.controller.initialize().await?;
        info!("Input controller initialized");
        Ok(())
    }

    /// Handle input event from remote operator
    pub async fn handle_event(&self, event_data: &serde_json::Value) -> Result<()> {
        // Check if input is currently blocked
        let blocked_guard = self.is_input_blocked.read().await;
        if *blocked_guard {
            debug!("Input event ignored - input is blocked");
            return Ok(());
        }
        drop(blocked_guard);

        // Parse input event
        let event: InputEvent = serde_json::from_value(event_data.clone())
            .map_err(|e| anyhow::anyhow!("Failed to parse input event: {}", e))?;

        debug!("Handling input event: {:?}", event);

        // Dispatch to appropriate handler
        match event {
            InputEvent::MouseMove { x, y } => {
                self.controller.handle_mouse_move(x, y).await?;
            }
            InputEvent::MouseButton { button, pressed } => {
                self.controller.handle_mouse_button(button, pressed).await?;
            }
            InputEvent::MouseScroll { delta_x, delta_y } => {
                self.controller.handle_mouse_scroll(delta_x, delta_y).await?;
            }
            InputEvent::KeyEvent { key, pressed } => {
                self.controller.handle_key_event(key, pressed).await?;
            }
            InputEvent::TextInput { text } => {
                self.controller.handle_text_input(&text).await?;
            }
        }

        Ok(())
    }

    /// Block user input (for backstage sessions with screen blanking)
    pub async fn block_user_input(&self) -> Result<()> {
        info!("Blocking user input");
        
        self.controller.block_user_input().await?;
        
        let mut blocked_guard = self.is_input_blocked.write().await;
        *blocked_guard = true;
        
        Ok(())
    }

    /// Unblock user input
    pub async fn unblock_user_input(&self) -> Result<()> {
        info!("Unblocking user input");
        
        self.controller.unblock_user_input().await?;
        
        let mut blocked_guard = self.is_input_blocked.write().await;
        *blocked_guard = false;
        
        Ok(())
    }

    /// Check if input is currently blocked
    pub async fn is_input_blocked(&self) -> bool {
        let blocked_guard = self.is_input_blocked.read().await;
        *blocked_guard
    }

    /// Check if controller is healthy
    pub fn is_healthy(&self) -> bool {
        self.controller.is_healthy()
    }
}

impl Drop for InputController {
    fn drop(&mut self) {
        // Cleanup will be handled by async drop when available
        // For now, we rely on controller cleanup in their drop implementations
    }
}

/// Helper function to convert screen coordinates between different resolutions
pub fn scale_coordinates(
    x: i32,
    y: i32,
    from_resolution: (u32, u32),
    to_resolution: (u32, u32),
) -> (i32, i32) {
    let (from_w, from_h) = from_resolution;
    let (to_w, to_h) = to_resolution;
    
    let scale_x = to_w as f32 / from_w as f32;
    let scale_y = to_h as f32 / from_h as f32;
    
    let scaled_x = (x as f32 * scale_x) as i32;
    let scaled_y = (y as f32 * scale_y) as i32;
    
    (scaled_x, scaled_y)
}

/// Helper function to translate key codes between platforms
pub fn translate_key_code(key: KeyCode) -> u32 {
    match key {
        // This would be implemented per-platform
        // For now, return a placeholder
        KeyCode::Raw(code) => code,
        _ => 0, // TODO: Implement proper key code translation
    }
}

// ===== New Native Input System =====

// Native input implementations
#[cfg(target_os = "linux")]
pub mod x11_input;

// Cross-platform input protocol and service
pub mod input_protocol;
pub mod input_service;

// Re-exports for convenience
pub use input_protocol::{
    InputEvent as NewInputEvent, 
    InputStats, 
    MouseButtonType, 
    KeyType, 
    SpecialKeyType, 
    ModifierFlags, 
    ScrollDirectionType
};
pub use input_service::{InputService, InputServiceConfig};

#[cfg(target_os = "linux")]
pub use x11_input::{
    X11InputInjector, 
    MouseButton as X11MouseButton, 
    ScrollDirection, 
    SpecialKey, 
    KeyModifiers
};
