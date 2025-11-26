// Main app module - needs to compile for server SSR
pub mod app;

// For SSR (server-side rendering), we need stub modules that provide the same interface
// The actual implementations with browser APIs are only compiled for WASM

// Client-side modules - only compile for WASM target
#[cfg(target_arch = "wasm32")]
pub mod components;
#[cfg(target_arch = "wasm32")]
pub mod api_client;
#[cfg(target_arch = "wasm32")]
pub mod dashboard;
#[cfg(target_arch = "wasm32")]
pub mod sessions;
#[cfg(target_arch = "wasm32")]
pub mod device_discovery;
#[cfg(target_arch = "wasm32")]
pub mod monitor_controls;
#[cfg(target_arch = "wasm32")]
pub mod realtime;
#[cfg(target_arch = "wasm32")]
pub mod session_launcher;

// Server-side stub modules that provide the same component signatures
// These are used during SSR to render placeholder content
#[cfg(not(target_arch = "wasm32"))]
pub mod dashboard {
    use leptos::*;

    #[component]
    pub fn Dashboard() -> impl IntoView {
        view! {
            <div class="loading">"Loading dashboard..."</div>
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub mod sessions {
    use leptos::*;

    #[component]
    pub fn SessionsPage() -> impl IntoView {
        view! {
            <div class="loading">"Loading sessions..."</div>
        }
    }

    #[component]
    pub fn SessionViewer() -> impl IntoView {
        view! {
            <div class="loading">"Loading session viewer..."</div>
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub mod device_discovery {
    use leptos::*;

    #[component]
    pub fn DeviceDiscovery() -> impl IntoView {
        view! {
            <div class="loading">"Loading device discovery..."</div>
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub mod api_client {
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Serialize, Deserialize, Default)]
    pub struct ServerStats {
        pub connected_devices: usize,
        pub active_sessions: usize,
    }

    pub struct ApiClient;

    impl ApiClient {
        #[allow(dead_code)]
        pub fn new() -> Self { Self }

        pub async fn get_stats() -> Result<ServerStats, String> {
            // Server-side stub - returns default stats
            Ok(ServerStats::default())
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub mod components {
    // Server-side stubs for shared components
}

#[cfg(not(target_arch = "wasm32"))]
pub mod monitor_controls {
    // Server-side stubs
}

#[cfg(not(target_arch = "wasm32"))]
pub mod realtime {
    // Server-side stubs
}

#[cfg(not(target_arch = "wasm32"))]
pub mod session_launcher {
    // Server-side stubs
}
