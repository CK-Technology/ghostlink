pub mod wayland;
pub mod x11;

pub use wayland::WaylandInputController;
pub use x11::X11InputController;

use crate::error::Result;
use tracing::info;

/// Detect whether we're running on X11 or Wayland and create appropriate controller
pub async fn create_input_controller() -> Result<Box<dyn super::InputController>> {
    let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    
    info!("Detected session type: {}", session_type);
    
    match session_type.as_str() {
        "wayland" => {
            info!("Creating Wayland input controller");
            let controller = WaylandInputController::new().await?;
            Ok(Box::new(controller))
        }
        "x11" | "X11" => {
            info!("Creating X11 input controller");
            let controller = X11InputController::new().await?;
            Ok(Box::new(controller))
        }
        _ => {
            // Try to detect based on other environment variables
            if std::env::var("WAYLAND_DISPLAY").is_ok() {
                info!("WAYLAND_DISPLAY detected, creating Wayland input controller");
                let controller = WaylandInputController::new().await?;
                Ok(Box::new(controller))
            } else if std::env::var("DISPLAY").is_ok() {
                info!("DISPLAY detected, creating X11 input controller");
                let controller = X11InputController::new().await?;
                Ok(Box::new(controller))
            } else {
                // Default to X11 as fallback
                info!("Unknown session type, defaulting to X11");
                let controller = X11InputController::new().await?;
                Ok(Box::new(controller))
            }
        }
    }
}