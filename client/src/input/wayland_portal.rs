//! Wayland-native input injection using XDG RemoteDesktop Portal
//!
//! This module provides input injection (keyboard, mouse) for Wayland compositors
//! using the org.freedesktop.portal.RemoteDesktop portal interface.
//!
//! The RemoteDesktop portal is the only sanctioned way to inject input on Wayland,
//! and it requires a valid ScreenCast session to be active first.
//!
//! Reference: https://flatpak.github.io/xdg-desktop-portal/#gdbus-org.freedesktop.portal.RemoteDesktop

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use dbus::arg::PropMap;
use dbus::blocking::SyncConnection;
use dbus::Message;

use tracing::{debug, info};

use crate::error::{GhostLinkError, InputError, Result};
use crate::capture::wayland::portal::PortalSession;

const PORTAL_BUS: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const REMOTE_DESKTOP_IFACE: &str = "org.freedesktop.portal.RemoteDesktop";

/// Device types for RemoteDesktop portal
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeviceType {
    Keyboard = 1,
    Pointer = 2,
    TouchScreen = 4,
}

/// Key state for keyboard events
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeyState {
    Released = 0,
    Pressed = 1,
}

/// Button state for pointer events
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ButtonState {
    Released = 0,
    Pressed = 1,
}

/// Axis for scroll events
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Axis {
    Vertical = 0,
    Horizontal = 1,
}

/// RemoteDesktop Portal session for input injection
pub struct RemoteDesktopSession {
    /// Session object path (from ScreenCast portal)
    session_path: dbus::Path<'static>,
    /// Available device types
    available_devices: u32,
    /// Whether the session is active
    is_active: bool,
}

impl std::fmt::Debug for RemoteDesktopSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteDesktopSession")
            .field("session_path", &self.session_path)
            .field("available_devices", &self.available_devices)
            .field("is_active", &self.is_active)
            .finish()
    }
}

/// RemoteDesktop Portal client for input injection
pub struct RemoteDesktopPortal {
    conn: Arc<SyncConnection>,
    session: Option<RemoteDesktopSession>,
}

impl RemoteDesktopPortal {
    /// Create a new RemoteDesktop portal client
    pub fn new() -> Result<Self> {
        info!("Initializing RemoteDesktop Portal for input injection");

        let conn = SyncConnection::new_session().map_err(|e| {
            GhostLinkError::Input(InputError::MethodUnavailable {
                method: format!("DBus session connection: {}", e),
            })
        })?;

        Ok(Self {
            conn: Arc::new(conn),
            session: None,
        })
    }

    /// Connect to an existing ScreenCast session for input injection
    /// The RemoteDesktop portal requires an active ScreenCast session
    pub fn connect_to_session(&mut self, portal_session: &PortalSession) -> Result<()> {
        info!("Connecting RemoteDesktop to existing ScreenCast session");

        // Get available devices - default to keyboard + pointer
        let available_devices = DeviceType::Keyboard as u32 | DeviceType::Pointer as u32;

        info!("RemoteDesktop devices available: keyboard={}, pointer={}, touch={}",
            (available_devices & DeviceType::Keyboard as u32) != 0,
            (available_devices & DeviceType::Pointer as u32) != 0,
            (available_devices & DeviceType::TouchScreen as u32) != 0,
        );

        self.session = Some(RemoteDesktopSession {
            session_path: portal_session.session_path.clone(),
            available_devices,
            is_active: true,
        });

        Ok(())
    }

    /// Send a keyboard key event
    pub fn notify_keyboard_keycode(&self, keycode: u32, state: KeyState) -> Result<()> {
        let session = self.session.as_ref().ok_or_else(|| {
            GhostLinkError::Input(InputError::MethodUnavailable {
                method: "No active RemoteDesktop session".to_string(),
            })
        })?;

        if !session.is_active {
            return Err(GhostLinkError::Input(InputError::InputBlocked));
        }

        debug!("Sending keyboard keycode {} state {:?}", keycode, state);

        // Build options dict
        let options: PropMap = HashMap::new();

        // Call NotifyKeyboardKeycode(session_handle, options, keycode, state)
        let msg = Message::new_method_call(
            PORTAL_BUS,
            PORTAL_PATH,
            REMOTE_DESKTOP_IFACE,
            "NotifyKeyboardKeycode",
        )
        .map_err(|e| GhostLinkError::Other(format!("Failed to create message: {}", e)))?
        .append2(&session.session_path, &options)
        .append2(keycode as i32, state as u32);

        self.conn.channel().send(msg).map_err(|_| {
            GhostLinkError::Input(InputError::KeyMappingFailed {
                key: format!("keycode {}", keycode),
            })
        })?;

        Ok(())
    }

    /// Send a keyboard key event using keysym (for Unicode/special keys)
    pub fn notify_keyboard_keysym(&self, keysym: u32, state: KeyState) -> Result<()> {
        let session = self.session.as_ref().ok_or_else(|| {
            GhostLinkError::Input(InputError::MethodUnavailable {
                method: "No active RemoteDesktop session".to_string(),
            })
        })?;

        if !session.is_active {
            return Err(GhostLinkError::Input(InputError::InputBlocked));
        }

        debug!("Sending keyboard keysym {} state {:?}", keysym, state);

        let options: PropMap = HashMap::new();

        let msg = Message::new_method_call(
            PORTAL_BUS,
            PORTAL_PATH,
            REMOTE_DESKTOP_IFACE,
            "NotifyKeyboardKeysym",
        )
        .map_err(|e| GhostLinkError::Other(format!("Failed to create message: {}", e)))?
        .append2(&session.session_path, &options)
        .append2(keysym as i32, state as u32);

        self.conn.channel().send(msg).map_err(|_| {
            GhostLinkError::Input(InputError::KeyMappingFailed {
                key: format!("keysym {}", keysym),
            })
        })?;

        Ok(())
    }

    /// Send absolute pointer motion
    pub fn notify_pointer_motion_absolute(&self, x: f64, y: f64, stream_id: u32) -> Result<()> {
        let session = self.session.as_ref().ok_or_else(|| {
            GhostLinkError::Input(InputError::MethodUnavailable {
                method: "No active RemoteDesktop session".to_string(),
            })
        })?;

        if !session.is_active {
            return Err(GhostLinkError::Input(InputError::InputBlocked));
        }

        debug!("Sending pointer motion absolute ({}, {}) stream {}", x, y, stream_id);

        let options: PropMap = HashMap::new();

        let msg = Message::new_method_call(
            PORTAL_BUS,
            PORTAL_PATH,
            REMOTE_DESKTOP_IFACE,
            "NotifyPointerMotionAbsolute",
        )
        .map_err(|e| GhostLinkError::Other(format!("Failed to create message: {}", e)))?
        .append2(&session.session_path, &options)
        .append3(stream_id, x, y);

        self.conn.channel().send(msg).map_err(|_| {
            GhostLinkError::Input(InputError::InvalidCoordinates { x: x as i32, y: y as i32 })
        })?;

        Ok(())
    }

    /// Send relative pointer motion
    pub fn notify_pointer_motion(&self, dx: f64, dy: f64) -> Result<()> {
        let session = self.session.as_ref().ok_or_else(|| {
            GhostLinkError::Input(InputError::MethodUnavailable {
                method: "No active RemoteDesktop session".to_string(),
            })
        })?;

        if !session.is_active {
            return Err(GhostLinkError::Input(InputError::InputBlocked));
        }

        debug!("Sending pointer motion relative ({}, {})", dx, dy);

        let options: PropMap = HashMap::new();

        let msg = Message::new_method_call(
            PORTAL_BUS,
            PORTAL_PATH,
            REMOTE_DESKTOP_IFACE,
            "NotifyPointerMotion",
        )
        .map_err(|e| GhostLinkError::Other(format!("Failed to create message: {}", e)))?
        .append2(&session.session_path, &options)
        .append2(dx, dy);

        self.conn.channel().send(msg).map_err(|_| {
            GhostLinkError::Other("Failed to send pointer motion".to_string())
        })?;

        Ok(())
    }

    /// Send pointer button event
    pub fn notify_pointer_button(&self, button: i32, state: ButtonState) -> Result<()> {
        let session = self.session.as_ref().ok_or_else(|| {
            GhostLinkError::Input(InputError::MethodUnavailable {
                method: "No active RemoteDesktop session".to_string(),
            })
        })?;

        if !session.is_active {
            return Err(GhostLinkError::Input(InputError::InputBlocked));
        }

        debug!("Sending pointer button {} state {:?}", button, state);

        let options: PropMap = HashMap::new();

        let msg = Message::new_method_call(
            PORTAL_BUS,
            PORTAL_PATH,
            REMOTE_DESKTOP_IFACE,
            "NotifyPointerButton",
        )
        .map_err(|e| GhostLinkError::Other(format!("Failed to create message: {}", e)))?
        .append2(&session.session_path, &options)
        .append2(button, state as u32);

        self.conn.channel().send(msg).map_err(|_| {
            GhostLinkError::Other("Failed to send pointer button".to_string())
        })?;

        Ok(())
    }

    /// Send scroll axis event
    pub fn notify_pointer_axis(&self, axis: Axis, steps: i32) -> Result<()> {
        let session = self.session.as_ref().ok_or_else(|| {
            GhostLinkError::Input(InputError::MethodUnavailable {
                method: "No active RemoteDesktop session".to_string(),
            })
        })?;

        if !session.is_active {
            return Err(GhostLinkError::Input(InputError::InputBlocked));
        }

        debug!("Sending pointer axis {:?} steps {}", axis, steps);

        let options: PropMap = HashMap::new();

        // NotifyPointerAxisDiscrete for scroll wheel
        let msg = Message::new_method_call(
            PORTAL_BUS,
            PORTAL_PATH,
            REMOTE_DESKTOP_IFACE,
            "NotifyPointerAxisDiscrete",
        )
        .map_err(|e| GhostLinkError::Other(format!("Failed to create message: {}", e)))?
        .append2(&session.session_path, &options)
        .append2(axis as u32, steps);

        self.conn.channel().send(msg).map_err(|_| {
            GhostLinkError::Other("Failed to send pointer axis".to_string())
        })?;

        Ok(())
    }

    /// Check if input injection is available
    pub fn is_available(&self) -> bool {
        self.session.as_ref().map(|s| s.is_active).unwrap_or(false)
    }

    /// Check if keyboard input is available
    pub fn has_keyboard(&self) -> bool {
        self.session.as_ref().map(|s| {
            s.is_active && (s.available_devices & DeviceType::Keyboard as u32) != 0
        }).unwrap_or(false)
    }

    /// Check if pointer input is available
    pub fn has_pointer(&self) -> bool {
        self.session.as_ref().map(|s| {
            s.is_active && (s.available_devices & DeviceType::Pointer as u32) != 0
        }).unwrap_or(false)
    }

    /// Close the RemoteDesktop session
    pub fn close(&mut self) {
        if let Some(session) = self.session.take() {
            info!("Closing RemoteDesktop session");
            // Session will be closed when ScreenCast session closes
            drop(session);
        }
    }
}

impl Drop for RemoteDesktopPortal {
    fn drop(&mut self) {
        self.close();
    }
}

// Linux evdev button codes for common mouse buttons
pub mod button_codes {
    pub const BTN_LEFT: i32 = 0x110;    // 272
    pub const BTN_RIGHT: i32 = 0x111;   // 273
    pub const BTN_MIDDLE: i32 = 0x112;  // 274
    pub const BTN_SIDE: i32 = 0x113;    // 275 (back)
    pub const BTN_EXTRA: i32 = 0x114;   // 276 (forward)
}

// Common keysyms from X11
pub mod keysyms {
    // Modifier keys
    pub const XK_SHIFT_L: u32 = 0xffe1;
    pub const XK_SHIFT_R: u32 = 0xffe2;
    pub const XK_CONTROL_L: u32 = 0xffe3;
    pub const XK_CONTROL_R: u32 = 0xffe4;
    pub const XK_ALT_L: u32 = 0xffe9;
    pub const XK_ALT_R: u32 = 0xffea;
    pub const XK_SUPER_L: u32 = 0xffeb;
    pub const XK_SUPER_R: u32 = 0xffec;

    // Special keys
    pub const XK_ESCAPE: u32 = 0xff1b;
    pub const XK_TAB: u32 = 0xff09;
    pub const XK_RETURN: u32 = 0xff0d;
    pub const XK_BACKSPACE: u32 = 0xff08;
    pub const XK_DELETE: u32 = 0xffff;
    pub const XK_INSERT: u32 = 0xff63;
    pub const XK_HOME: u32 = 0xff50;
    pub const XK_END: u32 = 0xff57;
    pub const XK_PAGE_UP: u32 = 0xff55;
    pub const XK_PAGE_DOWN: u32 = 0xff56;

    // Arrow keys
    pub const XK_LEFT: u32 = 0xff51;
    pub const XK_UP: u32 = 0xff52;
    pub const XK_RIGHT: u32 = 0xff53;
    pub const XK_DOWN: u32 = 0xff54;

    // Function keys
    pub const XK_F1: u32 = 0xffbe;
    pub const XK_F2: u32 = 0xffbf;
    pub const XK_F3: u32 = 0xffc0;
    pub const XK_F4: u32 = 0xffc1;
    pub const XK_F5: u32 = 0xffc2;
    pub const XK_F6: u32 = 0xffc3;
    pub const XK_F7: u32 = 0xffc4;
    pub const XK_F8: u32 = 0xffc5;
    pub const XK_F9: u32 = 0xffc6;
    pub const XK_F10: u32 = 0xffc7;
    pub const XK_F11: u32 = 0xffc8;
    pub const XK_F12: u32 = 0xffc9;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_types() {
        let devices = DeviceType::Keyboard as u32 | DeviceType::Pointer as u32;
        assert!((devices & DeviceType::Keyboard as u32) != 0);
        assert!((devices & DeviceType::Pointer as u32) != 0);
        assert!((devices & DeviceType::TouchScreen as u32) == 0);
    }
}
