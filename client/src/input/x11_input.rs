use crate::error::{GhostLinkError, Result};
use std::sync::Arc;
use tracing::{debug, error, info, trace, warn};

use x11rb::{
    connection::Connection,
    protocol::{
        xproto::{
            ButtonPressEvent, ButtonReleaseEvent, KeyPressEvent, KeyReleaseEvent, 
            MotionNotifyEvent, ConnectionExt, SendEventDest
        },
        xtest::ConnectionExt as XTestConnectionExt,
    },
    rust_connection::RustConnection,
};

/// High-performance native X11 input injection for remote control
pub struct X11InputInjector {
    connection: Arc<RustConnection>,
    root_window: u32,
    current_x: i16,
    current_y: i16,
}

/// Mouse button mapping for X11
#[derive(Debug, Clone, Copy)]
pub enum MouseButton {
    Left = 1,
    Middle = 2,
    Right = 3,
    ScrollUp = 4,
    ScrollDown = 5,
    ScrollLeft = 6,
    ScrollRight = 7,
}

/// Keyboard modifier keys
#[derive(Debug, Clone, Copy)]
pub struct KeyModifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

impl Default for KeyModifiers {
    fn default() -> Self {
        Self {
            shift: false,
            ctrl: false,
            alt: false,
            meta: false,
        }
    }
}

impl X11InputInjector {
    /// Create new X11 input injector
    pub fn new() -> Result<Self> {
        info!("Initializing native X11 input injector");
        
        // Connect to X11 display
        let (connection, screen) = x11rb::connect(None)
            .map_err(|e| GhostLinkError::Other(format!("Failed to connect to X11: {}", e)))?;
        
        let connection = Arc::new(connection);
        let root_window = connection.setup().roots[screen].root;
        
        // Check if XTEST extension is available
        let xtest_version = connection.xtest_get_version(2, 2)?;
        info!("XTEST extension available: {}.{}", 
            xtest_version.major_version, xtest_version.minor_version);
        
        Ok(Self {
            connection,
            root_window,
            current_x: 0,
            current_y: 0,
        })
    }
    
    /// Move mouse cursor to absolute coordinates
    pub fn move_mouse_absolute(&mut self, x: i16, y: i16) -> Result<()> {
        trace!("Moving mouse to absolute position: ({}, {})", x, y);
        
        // Use XTEST to move the mouse pointer
        self.connection.xtest_fake_input(
            x11rb::protocol::xproto::MOTION_NOTIFY_EVENT,
            0,  // detail (not used for motion)
            x11rb::CURRENT_TIME,
            self.root_window,
            x,
            y,
            0,  // deviceid (0 = core pointer)
        )?;
        
        self.connection.flush()?;
        
        self.current_x = x;
        self.current_y = y;
        
        Ok(())
    }
    
    /// Move mouse cursor relative to current position
    pub fn move_mouse_relative(&mut self, dx: i16, dy: i16) -> Result<()> {
        let new_x = self.current_x + dx;
        let new_y = self.current_y + dy;
        
        trace!("Moving mouse relatively by ({}, {}) to ({}, {})", dx, dy, new_x, new_y);
        
        self.move_mouse_absolute(new_x, new_y)
    }
    
    /// Press mouse button
    pub fn press_mouse_button(&self, button: MouseButton) -> Result<()> {
        debug!("Pressing mouse button: {:?}", button);
        
        self.connection.xtest_fake_input(
            x11rb::protocol::xproto::BUTTON_PRESS_EVENT,
            button as u8,
            x11rb::CURRENT_TIME,
            self.root_window,
            0, 0, 0,
        )?;
        
        self.connection.flush()?;
        Ok(())
    }
    
    /// Release mouse button
    pub fn release_mouse_button(&self, button: MouseButton) -> Result<()> {
        debug!("Releasing mouse button: {:?}", button);
        
        self.connection.xtest_fake_input(
            x11rb::protocol::xproto::BUTTON_RELEASE_EVENT,
            button as u8,
            x11rb::CURRENT_TIME,
            self.root_window,
            0, 0, 0,
        )?;
        
        self.connection.flush()?;
        Ok(())
    }
    
    /// Click mouse button (press and release)
    pub fn click_mouse_button(&self, button: MouseButton) -> Result<()> {
        debug!("Clicking mouse button: {:?}", button);
        
        self.press_mouse_button(button)?;
        
        // Small delay between press and release for proper event handling
        std::thread::sleep(std::time::Duration::from_millis(1));
        
        self.release_mouse_button(button)?;
        Ok(())
    }
    
    /// Double-click mouse button
    pub fn double_click_mouse_button(&self, button: MouseButton) -> Result<()> {
        debug!("Double-clicking mouse button: {:?}", button);
        
        self.click_mouse_button(button)?;
        
        // Standard double-click interval
        std::thread::sleep(std::time::Duration::from_millis(50));
        
        self.click_mouse_button(button)?;
        Ok(())
    }
    
    /// Scroll mouse wheel
    pub fn scroll_mouse(&self, direction: ScrollDirection, clicks: u32) -> Result<()> {
        debug!("Scrolling mouse {:?} {} clicks", direction, clicks);
        
        let button = match direction {
            ScrollDirection::Up => MouseButton::ScrollUp,
            ScrollDirection::Down => MouseButton::ScrollDown,
            ScrollDirection::Left => MouseButton::ScrollLeft,
            ScrollDirection::Right => MouseButton::ScrollRight,
        };
        
        for _ in 0..clicks {
            self.click_mouse_button(button)?;
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        
        Ok(())
    }
    
    /// Press keyboard key by keycode
    pub fn press_key(&self, keycode: u8, modifiers: KeyModifiers) -> Result<()> {
        debug!("Pressing key: {} with modifiers: {:?}", keycode, modifiers);
        
        // Press modifier keys first
        self.apply_modifiers(modifiers, true)?;
        
        // Press the main key
        self.connection.xtest_fake_input(
            x11rb::protocol::xproto::KEY_PRESS_EVENT,
            keycode,
            x11rb::CURRENT_TIME,
            self.root_window,
            0, 0, 0,
        )?;
        
        self.connection.flush()?;
        Ok(())
    }
    
    /// Release keyboard key by keycode
    pub fn release_key(&self, keycode: u8, modifiers: KeyModifiers) -> Result<()> {
        debug!("Releasing key: {} with modifiers: {:?}", keycode, modifiers);
        
        // Release the main key
        self.connection.xtest_fake_input(
            x11rb::protocol::xproto::KEY_RELEASE_EVENT,
            keycode,
            x11rb::CURRENT_TIME,
            self.root_window,
            0, 0, 0,
        )?;
        
        // Release modifier keys
        self.apply_modifiers(modifiers, false)?;
        
        self.connection.flush()?;
        Ok(())
    }
    
    /// Press and release a key (single key press)
    pub fn press_and_release_key(&self, keycode: u8, modifiers: KeyModifiers) -> Result<()> {
        debug!("Pressing and releasing key: {} with modifiers: {:?}", keycode, modifiers);
        
        self.press_key(keycode, modifiers)?;
        
        // Small delay for proper key event handling
        std::thread::sleep(std::time::Duration::from_millis(1));
        
        self.release_key(keycode, modifiers)?;
        Ok(())
    }
    
    /// Type a string by converting to keycodes and sending key events
    pub fn type_string(&self, text: &str) -> Result<()> {
        debug!("Typing string: '{}'", text);
        
        for ch in text.chars() {
            let (keycode, modifiers) = self.char_to_keycode(ch)?;
            self.press_and_release_key(keycode, modifiers)?;
            
            // Small delay between characters for natural typing
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        
        Ok(())
    }
    
    /// Apply or remove modifier keys
    fn apply_modifiers(&self, modifiers: KeyModifiers, press: bool) -> Result<()> {
        let event_type = if press { 
            x11rb::protocol::xproto::KEY_PRESS_EVENT 
        } else { 
            x11rb::protocol::xproto::KEY_RELEASE_EVENT 
        };
        
        // Get keycodes for modifier keys (these are standard across most X11 setups)
        if modifiers.shift {
            self.connection.xtest_fake_input(event_type, 50, x11rb::CURRENT_TIME, self.root_window, 0, 0, 0)?; // Left Shift
        }
        if modifiers.ctrl {
            self.connection.xtest_fake_input(event_type, 37, x11rb::CURRENT_TIME, self.root_window, 0, 0, 0)?; // Left Ctrl
        }
        if modifiers.alt {
            self.connection.xtest_fake_input(event_type, 64, x11rb::CURRENT_TIME, self.root_window, 0, 0, 0)?; // Left Alt
        }
        if modifiers.meta {
            self.connection.xtest_fake_input(event_type, 133, x11rb::CURRENT_TIME, self.root_window, 0, 0, 0)?; // Left Super/Meta
        }
        
        Ok(())
    }
    
    /// Convert a character to keycode and modifiers
    /// This is a simplified mapping - a full implementation would query the keyboard layout
    pub fn char_to_keycode(&self, ch: char) -> Result<(u8, KeyModifiers)> {
        let mut modifiers = KeyModifiers::default();
        
        let keycode = match ch {
            // Letters (lowercase)
            'a'..='z' => 38 + (ch as u8 - b'a'),
            
            // Letters (uppercase)
            'A'..='Z' => {
                modifiers.shift = true;
                38 + (ch as u8 - b'A')
            }
            
            // Numbers
            '0'..='9' => {
                if ch == '0' { 19 } else { 10 + (ch as u8 - b'1') }
            }
            
            // Common symbols
            ' ' => 65,  // Space
            '\n' => 36, // Return
            '\t' => 23, // Tab
            
            // Symbols that require shift
            '!' => { modifiers.shift = true; 10 }, // !
            '@' => { modifiers.shift = true; 11 }, // @
            '#' => { modifiers.shift = true; 12 }, // #
            '$' => { modifiers.shift = true; 13 }, // $
            '%' => { modifiers.shift = true; 14 }, // %
            '^' => { modifiers.shift = true; 15 }, // ^
            '&' => { modifiers.shift = true; 16 }, // &
            '*' => { modifiers.shift = true; 17 }, // *
            '(' => { modifiers.shift = true; 18 }, // (
            ')' => { modifiers.shift = true; 19 }, // )
            
            // Common punctuation
            '.' => 60,  // Period
            ',' => 59,  // Comma
            ';' => 47,  // Semicolon
            ':' => { modifiers.shift = true; 47 }, // Colon
            '\'' => 48, // Apostrophe
            '"' => { modifiers.shift = true; 48 }, // Quote
            '/' => 61,  // Slash
            '?' => { modifiers.shift = true; 61 }, // Question mark
            '\\' => 51, // Backslash
            '|' => { modifiers.shift = true; 51 }, // Pipe
            '=' => 21,  // Equal
            '+' => { modifiers.shift = true; 21 }, // Plus
            '-' => 20,  // Minus
            '_' => { modifiers.shift = true; 20 }, // Underscore
            '[' => 34,  // Left bracket
            '{' => { modifiers.shift = true; 34 }, // Left brace
            ']' => 35,  // Right bracket
            '}' => { modifiers.shift = true; 35 }, // Right brace
            '`' => 49,  // Grave
            '~' => { modifiers.shift = true; 49 }, // Tilde
            
            _ => {
                warn!("Unsupported character for typing: '{}'", ch);
                return Err(GhostLinkError::Other(format!("Unsupported character: {}", ch)));
            }
        };
        
        Ok((keycode, modifiers))
    }
    
    /// Send special key combinations (like Ctrl+C, Alt+Tab, etc.)
    pub fn send_key_combination(&self, keys: &[SpecialKey]) -> Result<()> {
        debug!("Sending key combination: {:?}", keys);
        
        // Press all keys in order
        for key in keys {
            let keycode = key.to_keycode();
            self.connection.xtest_fake_input(
                x11rb::protocol::xproto::KEY_PRESS_EVENT,
                keycode,
                x11rb::CURRENT_TIME,
                self.root_window,
                0, 0, 0,
            )?;
        }
        
        self.connection.flush()?;
        
        // Small delay to ensure keys are registered
        std::thread::sleep(std::time::Duration::from_millis(10));
        
        // Release all keys in reverse order
        for key in keys.iter().rev() {
            let keycode = key.to_keycode();
            self.connection.xtest_fake_input(
                x11rb::protocol::xproto::KEY_RELEASE_EVENT,
                keycode,
                x11rb::CURRENT_TIME,
                self.root_window,
                0, 0, 0,
            )?;
        }
        
        self.connection.flush()?;
        Ok(())
    }
    
    /// Get current mouse position
    pub fn get_mouse_position(&self) -> (i16, i16) {
        (self.current_x, self.current_y)
    }
    
    /// Check if input injection is working by testing XTEST availability
    pub fn test_input_injection(&self) -> Result<bool> {
        info!("Testing X11 input injection capabilities");
        
        // Try to get current pointer position as a test
        match self.connection.query_pointer(self.root_window) {
            Ok(_) => {
                info!("X11 input injection test successful");
                Ok(true)
            }
            Err(e) => {
                error!("X11 input injection test failed: {}", e);
                Ok(false)
            }
        }
    }
    
    /// Get display dimensions for coordinate validation
    pub fn get_display_dimensions(&self) -> Result<(u16, u16)> {
        let geometry = self.connection.get_geometry(self.root_window)?;
        let reply = geometry.reply()?;
        Ok((reply.width, reply.height))
    }
}

/// Scroll direction for mouse wheel events
#[derive(Debug, Clone, Copy)]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Special keys for key combinations
#[derive(Debug, Clone, Copy)]
pub enum SpecialKey {
    Ctrl,
    Alt,
    Shift,
    Meta,
    Tab,
    Enter,
    Escape,
    Space,
    Backspace,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
}

impl SpecialKey {
    /// Convert special key to X11 keycode
    pub fn to_keycode(&self) -> u8 {
        match self {
            SpecialKey::Ctrl => 37,      // Left Ctrl
            SpecialKey::Alt => 64,       // Left Alt
            SpecialKey::Shift => 50,     // Left Shift
            SpecialKey::Meta => 133,     // Left Super/Meta
            SpecialKey::Tab => 23,
            SpecialKey::Enter => 36,
            SpecialKey::Escape => 9,
            SpecialKey::Space => 65,
            SpecialKey::Backspace => 22,
            SpecialKey::Delete => 119,
            SpecialKey::Home => 110,
            SpecialKey::End => 115,
            SpecialKey::PageUp => 112,
            SpecialKey::PageDown => 117,
            SpecialKey::ArrowUp => 111,
            SpecialKey::ArrowDown => 116,
            SpecialKey::ArrowLeft => 113,
            SpecialKey::ArrowRight => 114,
            SpecialKey::F1 => 67,
            SpecialKey::F2 => 68,
            SpecialKey::F3 => 69,
            SpecialKey::F4 => 70,
            SpecialKey::F5 => 71,
            SpecialKey::F6 => 72,
            SpecialKey::F7 => 73,
            SpecialKey::F8 => 74,
            SpecialKey::F9 => 75,
            SpecialKey::F10 => 76,
            SpecialKey::F11 => 95,
            SpecialKey::F12 => 96,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_char_to_keycode() {
        let injector = X11InputInjector::new().unwrap();
        
        // Test basic character conversion
        let (keycode, modifiers) = injector.char_to_keycode('a').unwrap();
        assert_eq!(keycode, 38);
        assert!(!modifiers.shift);
        
        // Test uppercase character (should have shift modifier)
        let (keycode, modifiers) = injector.char_to_keycode('A').unwrap();
        assert_eq!(keycode, 38);
        assert!(modifiers.shift);
        
        // Test number
        let (keycode, _) = injector.char_to_keycode('1').unwrap();
        assert_eq!(keycode, 10);
    }
    
    #[test]
    fn test_special_key_keycodes() {
        assert_eq!(SpecialKey::Ctrl.to_keycode(), 37);
        assert_eq!(SpecialKey::Enter.to_keycode(), 36);
        assert_eq!(SpecialKey::Space.to_keycode(), 65);
    }
}