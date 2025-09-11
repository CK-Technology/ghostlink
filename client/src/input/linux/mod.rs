pub mod wayland;
pub mod x11;

pub use wayland::WaylandInputHandler;
pub use x11::X11InputHandler;