use crate::error::{GhostLinkError, Result};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, trace, warn};

/// Input event types for remote control
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InputEvent {
    /// Mouse movement to absolute coordinates
    MouseMove {
        x: i16,
        y: i16,
        timestamp: u64,
    },
    
    /// Mouse movement relative to current position
    MouseMoveRelative {
        dx: i16,
        dy: i16,
        timestamp: u64,
    },
    
    /// Mouse button press
    MousePress {
        button: MouseButtonType,
        x: i16,
        y: i16,
        timestamp: u64,
    },
    
    /// Mouse button release
    MouseRelease {
        button: MouseButtonType,
        x: i16,
        y: i16,
        timestamp: u64,
    },
    
    /// Mouse button click (press + release)
    MouseClick {
        button: MouseButtonType,
        x: i16,
        y: i16,
        double_click: bool,
        timestamp: u64,
    },
    
    /// Mouse wheel scroll
    MouseScroll {
        direction: ScrollDirectionType,
        clicks: u32,
        x: i16,
        y: i16,
        timestamp: u64,
    },
    
    /// Key press
    KeyPress {
        key: KeyType,
        modifiers: ModifierFlags,
        timestamp: u64,
    },
    
    /// Key release
    KeyRelease {
        key: KeyType,
        modifiers: ModifierFlags,
        timestamp: u64,
    },
    
    /// Key press and release (single keystroke)
    KeyStroke {
        key: KeyType,
        modifiers: ModifierFlags,
        timestamp: u64,
    },
    
    /// Type text string
    TypeText {
        text: String,
        timestamp: u64,
    },
    
    /// Special key combination (like Ctrl+C, Alt+Tab)
    KeyCombination {
        keys: Vec<SpecialKeyType>,
        timestamp: u64,
    },
    
    /// Clipboard operations
    ClipboardSet {
        text: String,
        timestamp: u64,
    },
    
    ClipboardGet {
        timestamp: u64,
    },
}

/// Mouse button types
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MouseButtonType {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

/// Scroll direction types
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ScrollDirectionType {
    Up,
    Down,
    Left,
    Right,
}

/// Key types - can be character, keycode, or special key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyType {
    /// Unicode character
    Character(char),
    /// Raw keycode
    Keycode(u8),
    /// Special key
    Special(SpecialKeyType),
}

/// Special key types
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SpecialKeyType {
    // Modifier keys
    Ctrl,
    Alt,
    Shift,
    Meta,
    
    // Navigation keys
    Tab,
    Enter,
    Escape,
    Space,
    Backspace,
    Delete,
    
    // Arrow keys
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    
    // Page navigation
    Home,
    End,
    PageUp,
    PageDown,
    
    // Function keys
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    
    // System keys
    PrintScreen,
    ScrollLock,
    Pause,
    Insert,
    
    // Media keys
    VolumeUp,
    VolumeDown,
    VolumeMute,
    MediaPlay,
    MediaPause,
    MediaStop,
    MediaNext,
    MediaPrevious,
}

/// Modifier key flags
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ModifierFlags {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

/// Input statistics for monitoring and debugging
#[derive(Debug, Clone, Default)]
pub struct InputStats {
    pub events_received: u64,
    pub events_processed: u64,
    pub events_failed: u64,
    pub mouse_events: u64,
    pub keyboard_events: u64,
    pub last_event_time: u64,
    pub processing_errors: Vec<String>,
}

impl InputEvent {
    /// Create a new input event with current timestamp
    pub fn new_mouse_move(x: i16, y: i16) -> Self {
        Self::MouseMove {
            x,
            y,
            timestamp: current_timestamp(),
        }
    }
    
    pub fn new_mouse_click(button: MouseButtonType, x: i16, y: i16, double_click: bool) -> Self {
        Self::MouseClick {
            button,
            x,
            y,
            double_click,
            timestamp: current_timestamp(),
        }
    }
    
    pub fn new_key_stroke(key: KeyType, modifiers: ModifierFlags) -> Self {
        Self::KeyStroke {
            key,
            modifiers,
            timestamp: current_timestamp(),
        }
    }
    
    pub fn new_type_text(text: String) -> Self {
        Self::TypeText {
            text,
            timestamp: current_timestamp(),
        }
    }
    
    pub fn new_scroll(direction: ScrollDirectionType, clicks: u32, x: i16, y: i16) -> Self {
        Self::MouseScroll {
            direction,
            clicks,
            x,
            y,
            timestamp: current_timestamp(),
        }
    }
    
    /// Get event timestamp
    pub fn timestamp(&self) -> u64 {
        match self {
            Self::MouseMove { timestamp, .. } => *timestamp,
            Self::MouseMoveRelative { timestamp, .. } => *timestamp,
            Self::MousePress { timestamp, .. } => *timestamp,
            Self::MouseRelease { timestamp, .. } => *timestamp,
            Self::MouseClick { timestamp, .. } => *timestamp,
            Self::MouseScroll { timestamp, .. } => *timestamp,
            Self::KeyPress { timestamp, .. } => *timestamp,
            Self::KeyRelease { timestamp, .. } => *timestamp,
            Self::KeyStroke { timestamp, .. } => *timestamp,
            Self::TypeText { timestamp, .. } => *timestamp,
            Self::KeyCombination { timestamp, .. } => *timestamp,
            Self::ClipboardSet { timestamp, .. } => *timestamp,
            Self::ClipboardGet { timestamp, .. } => *timestamp,
        }
    }
    
    /// Check if this is a mouse event
    pub fn is_mouse_event(&self) -> bool {
        matches!(self, 
            Self::MouseMove { .. } | 
            Self::MouseMoveRelative { .. } |
            Self::MousePress { .. } |
            Self::MouseRelease { .. } |
            Self::MouseClick { .. } |
            Self::MouseScroll { .. }
        )
    }
    
    /// Check if this is a keyboard event
    pub fn is_keyboard_event(&self) -> bool {
        matches!(self,
            Self::KeyPress { .. } |
            Self::KeyRelease { .. } |
            Self::KeyStroke { .. } |
            Self::TypeText { .. } |
            Self::KeyCombination { .. }
        )
    }
    
    /// Get event type as string for logging
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::MouseMove { .. } => "mouse_move",
            Self::MouseMoveRelative { .. } => "mouse_move_relative", 
            Self::MousePress { .. } => "mouse_press",
            Self::MouseRelease { .. } => "mouse_release",
            Self::MouseClick { .. } => "mouse_click",
            Self::MouseScroll { .. } => "mouse_scroll",
            Self::KeyPress { .. } => "key_press",
            Self::KeyRelease { .. } => "key_release",
            Self::KeyStroke { .. } => "key_stroke",
            Self::TypeText { .. } => "type_text",
            Self::KeyCombination { .. } => "key_combination",
            Self::ClipboardSet { .. } => "clipboard_set",
            Self::ClipboardGet { .. } => "clipboard_get",
        }
    }
    
    /// Serialize input event to JSON for transmission
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self)
            .map_err(|e| GhostLinkError::Other(format!("Failed to serialize input event: {}", e)))
    }
    
    /// Deserialize input event from JSON
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json)
            .map_err(|e| GhostLinkError::Other(format!("Failed to deserialize input event: {}", e)))
    }
    
    /// Validate event parameters
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::MouseMove { x, y, .. } | 
            Self::MousePress { x, y, .. } |
            Self::MouseRelease { x, y, .. } |
            Self::MouseClick { x, y, .. } => {
                if *x < 0 || *y < 0 {
                    return Err(GhostLinkError::Other("Invalid mouse coordinates".to_string()));
                }
            }
            Self::MouseMoveRelative { dx, dy, .. } => {
                if dx.abs() > 10000 || dy.abs() > 10000 {
                    return Err(GhostLinkError::Other("Mouse movement too large".to_string()));
                }
            }
            Self::MouseScroll { clicks, .. } => {
                if *clicks > 100 {
                    return Err(GhostLinkError::Other("Too many scroll clicks".to_string()));
                }
            }
            Self::TypeText { text, .. } => {
                if text.len() > 10000 {
                    return Err(GhostLinkError::Other("Text too long".to_string()));
                }
            }
            Self::KeyCombination { keys, .. } => {
                if keys.len() > 10 {
                    return Err(GhostLinkError::Other("Too many keys in combination".to_string()));
                }
            }
            _ => {}
        }
        
        Ok(())
    }
}

impl InputStats {
    /// Record a received event
    pub fn record_event(&mut self, event: &InputEvent) {
        self.events_received += 1;
        self.last_event_time = event.timestamp();
        
        if event.is_mouse_event() {
            self.mouse_events += 1;
        } else if event.is_keyboard_event() {
            self.keyboard_events += 1;
        }
    }
    
    /// Record successful processing
    pub fn record_success(&mut self) {
        self.events_processed += 1;
    }
    
    /// Record processing failure
    pub fn record_failure(&mut self, error: String) {
        self.events_failed += 1;
        self.processing_errors.push(error);
        
        // Keep only last 10 errors to prevent memory growth
        if self.processing_errors.len() > 10 {
            self.processing_errors.drain(0..self.processing_errors.len() - 10);
        }
    }
    
    /// Get success rate as percentage
    pub fn success_rate(&self) -> f64 {
        if self.events_received == 0 {
            return 100.0;
        }
        
        (self.events_processed as f64 / self.events_received as f64) * 100.0
    }
    
    /// Get events per second based on time window
    pub fn events_per_second(&self, window_seconds: u64) -> f64 {
        if window_seconds == 0 {
            return 0.0;
        }
        
        self.events_received as f64 / window_seconds as f64
    }
}

/// Convert from protocol types to X11 input types
impl From<MouseButtonType> for crate::input::x11_input::MouseButton {
    fn from(button: MouseButtonType) -> Self {
        match button {
            MouseButtonType::Left => Self::Left,
            MouseButtonType::Right => Self::Right,
            MouseButtonType::Middle => Self::Middle,
            MouseButtonType::X1 => Self::ScrollUp,    // Map extra buttons to scroll for simplicity
            MouseButtonType::X2 => Self::ScrollDown,
        }
    }
}

impl From<ScrollDirectionType> for crate::input::x11_input::ScrollDirection {
    fn from(direction: ScrollDirectionType) -> Self {
        match direction {
            ScrollDirectionType::Up => Self::Up,
            ScrollDirectionType::Down => Self::Down,
            ScrollDirectionType::Left => Self::Left,
            ScrollDirectionType::Right => Self::Right,
        }
    }
}

impl From<ModifierFlags> for crate::input::x11_input::KeyModifiers {
    fn from(flags: ModifierFlags) -> Self {
        Self {
            shift: flags.shift,
            ctrl: flags.ctrl,
            alt: flags.alt,
            meta: flags.meta,
        }
    }
}

impl From<SpecialKeyType> for crate::input::x11_input::SpecialKey {
    fn from(key: SpecialKeyType) -> Self {
        match key {
            SpecialKeyType::Ctrl => Self::Ctrl,
            SpecialKeyType::Alt => Self::Alt,
            SpecialKeyType::Shift => Self::Shift,
            SpecialKeyType::Meta => Self::Meta,
            SpecialKeyType::Tab => Self::Tab,
            SpecialKeyType::Enter => Self::Enter,
            SpecialKeyType::Escape => Self::Escape,
            SpecialKeyType::Space => Self::Space,
            SpecialKeyType::Backspace => Self::Backspace,
            SpecialKeyType::Delete => Self::Delete,
            SpecialKeyType::Home => Self::Home,
            SpecialKeyType::End => Self::End,
            SpecialKeyType::PageUp => Self::PageUp,
            SpecialKeyType::PageDown => Self::PageDown,
            SpecialKeyType::ArrowUp => Self::ArrowUp,
            SpecialKeyType::ArrowDown => Self::ArrowDown,
            SpecialKeyType::ArrowLeft => Self::ArrowLeft,
            SpecialKeyType::ArrowRight => Self::ArrowRight,
            SpecialKeyType::F1 => Self::F1,
            SpecialKeyType::F2 => Self::F2,
            SpecialKeyType::F3 => Self::F3,
            SpecialKeyType::F4 => Self::F4,
            SpecialKeyType::F5 => Self::F5,
            SpecialKeyType::F6 => Self::F6,
            SpecialKeyType::F7 => Self::F7,
            SpecialKeyType::F8 => Self::F8,
            SpecialKeyType::F9 => Self::F9,
            SpecialKeyType::F10 => Self::F10,
            SpecialKeyType::F11 => Self::F11,
            SpecialKeyType::F12 => Self::F12,
            // For keys not directly mapped, use Escape as fallback
            _ => {
                warn!("Unmapped special key: {:?}, using Escape as fallback", key);
                Self::Escape
            }
        }
    }
}

/// Get current timestamp in microseconds
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_input_event_serialization() {
        let event = InputEvent::new_mouse_click(MouseButtonType::Left, 100, 200, false);
        
        let json = event.to_json().unwrap();
        let deserialized = InputEvent::from_json(&json).unwrap();
        
        assert!(matches!(deserialized, InputEvent::MouseClick { .. }));
    }
    
    #[test]
    fn test_input_event_validation() {
        // Valid event
        let valid_event = InputEvent::new_mouse_move(100, 200);
        assert!(valid_event.validate().is_ok());
        
        // Invalid event - negative coordinates
        let invalid_event = InputEvent::MouseMove {
            x: -1,
            y: 100,
            timestamp: current_timestamp(),
        };
        assert!(invalid_event.validate().is_err());
    }
    
    #[test]
    fn test_input_stats() {
        let mut stats = InputStats::default();
        let event = InputEvent::new_mouse_click(MouseButtonType::Left, 100, 200, false);
        
        stats.record_event(&event);
        assert_eq!(stats.events_received, 1);
        assert_eq!(stats.mouse_events, 1);
        
        stats.record_success();
        assert_eq!(stats.success_rate(), 100.0);
        
        stats.record_failure("Test error".to_string());
        assert!(stats.success_rate() < 100.0);
    }
}