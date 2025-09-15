use leptos::*;
use wasm_bindgen::prelude::*;
use web_sys::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Session launcher that detects and launches native client
#[component]
pub fn SessionLauncher() -> impl IntoView {
    let (client_status, set_client_status) = create_signal(ClientStatus::Checking);

    // Check if native client is installed
    create_effect(move |_| {
        spawn_local(async move {
            let status = check_native_client().await;
            set_client_status.set(status);
        });
    });

    view! {
        <div class="session-launcher">
            {move || {
                match client_status.get() {
                    ClientStatus::Installed(version) => {
                        view! {
                            <div class="alert alert-success">
                                <i class="bi bi-check-circle me-2"></i>
                                {format!("GhostLink Client v{} installed", version)}
                            </div>
                        }.into_view()
                    }
                    ClientStatus::NotInstalled => {
                        view! {
                            <div class="alert alert-warning">
                                <i class="bi bi-download me-2"></i>
                                "GhostLink Client not installed"
                                <button
                                    class="btn btn-primary btn-sm ms-2"
                                    on:click=move |_| {
                                        download_and_install_client();
                                    }
                                >
                                    "Download Client"
                                </button>
                            </div>
                        }.into_view()
                    }
                    ClientStatus::Checking => {
                        view! {
                            <div class="alert alert-info">
                                <div class="spinner-border spinner-border-sm me-2"></div>
                                "Checking for GhostLink Client..."
                            </div>
                        }.into_view()
                    }
                    ClientStatus::Outdated(current, latest) => {
                        view! {
                            <div class="alert alert-warning">
                                <i class="bi bi-exclamation-triangle me-2"></i>
                                {format!("Client outdated (v{} → v{})", current, latest)}
                                <button
                                    class="btn btn-warning btn-sm ms-2"
                                    on:click=move |_| {
                                        download_and_install_client();
                                    }
                                >
                                    "Update Client"
                                </button>
                            </div>
                        }.into_view()
                    }
                }
            }}
        </div>
    }
}

#[derive(Debug, Clone)]
pub enum ClientStatus {
    Checking,
    Installed(String), // version
    NotInstalled,
    Outdated(String, String), // current, latest
}

/// Enhanced device card with native session launcher
#[component]
pub fn DeviceCardWithLauncher(device: Device) -> impl IntoView {
    let (launching_session, set_launching_session) = create_signal(false);
    let (client_status, set_client_status) = create_signal(ClientStatus::Checking);

    // Check client status
    create_effect(move |_| {
        spawn_local(async move {
            let status = check_native_client().await;
            set_client_status.set(status);
        });
    });

    let launch_session = move |session_type: SessionType| {
        let device_id = device.id.clone();
        spawn_local(async move {
            set_launching_session.set(true);

            match launch_native_session(&device_id, session_type).await {
                Ok(_) => {
                    logging::log!("Session launched successfully");
                }
                Err(e) => {
                    logging::log!("Failed to launch session: {}", e);
                    // Show error modal
                }
            }

            set_launching_session.set(false);
        });
    };

    view! {
        <div class="card device-card h-100">
            <div class="card-body">
                // Device info header
                <div class="d-flex justify-content-between align-items-start mb-3">
                    <div class="d-flex align-items-center">
                        <DeviceIcon platform=device.platform.clone() online=device.is_online />
                        <div class="ms-2">
                            <h6 class="card-title mb-0 fw-bold">{device.name.clone()}</h6>
                            <small class="text-muted">{device.hostname.clone()}</small>
                        </div>
                    </div>
                    <OnlineStatusBadge online=device.is_online />
                </div>

                // Device details
                <DeviceSpecs device=device.clone() />

                // Session launch buttons
                <div class="d-grid gap-2 mt-3">
                    {move || {
                        match client_status.get() {
                            ClientStatus::Installed(_) => {
                                view! {
                                    <div class="btn-group" role="group">
                                        <button
                                            class="btn btn-primary"
                                            disabled=move || !device.is_online || launching_session.get()
                                            on:click={
                                                let launch = launch_session.clone();
                                                move |_| launch(SessionType::Control)
                                            }
                                        >
                                            {move || if launching_session.get() {
                                                view! {
                                                    <span>
                                                        <span class="spinner-border spinner-border-sm me-2"></span>
                                                        "Launching..."
                                                    </span>
                                                }.into_view()
                                            } else {
                                                view! {
                                                    <span>
                                                        <i class="bi bi-display me-2"></i>
                                                        "Remote Control"
                                                    </span>
                                                }.into_view()
                                            }}
                                        </button>
                                        <button
                                            class="btn btn-outline-primary"
                                            disabled=move || !device.is_online
                                            on:click={
                                                let launch = launch_session.clone();
                                                move |_| launch(SessionType::View)
                                            }
                                        >
                                            <i class="bi bi-eye"></i>
                                        </button>
                                        <button
                                            class="btn btn-outline-primary"
                                            disabled=move || !device.is_online
                                            on:click={
                                                let launch = launch_session.clone();
                                                move |_| launch(SessionType::FileTransfer)
                                            }
                                        >
                                            <i class="bi bi-folder"></i>
                                        </button>
                                    </div>
                                }.into_view()
                            }
                            ClientStatus::NotInstalled | ClientStatus::Outdated(_, _) => {
                                view! {
                                    <button
                                        class="btn btn-warning"
                                        on:click=move |_| download_and_install_client()
                                    >
                                        <i class="bi bi-download me-2"></i>
                                        "Install GhostLink Client"
                                    </button>
                                }.into_view()
                            }
                            ClientStatus::Checking => {
                                view! {
                                    <button class="btn btn-secondary" disabled=true>
                                        <span class="spinner-border spinner-border-sm me-2"></span>
                                        "Checking Client..."
                                    </button>
                                }.into_view()
                            }
                        }
                    }}
                </div>
            </div>
        </div>
    }
}

/// Check if native client is installed and get version
async fn check_native_client() -> ClientStatus {
    // Try to detect installed client via registry/file system
    #[cfg(target_os = "windows")]
    {
        match check_windows_client().await {
            Ok(version) => ClientStatus::Installed(version),
            Err(_) => ClientStatus::NotInstalled,
        }
    }

    #[cfg(target_os = "macos")]
    {
        match check_macos_client().await {
            Ok(version) => ClientStatus::Installed(version),
            Err(_) => ClientStatus::NotInstalled,
        }
    }

    #[cfg(target_os = "linux")]
    {
        match check_linux_client().await {
            Ok(version) => ClientStatus::Installed(version),
            Err(_) => ClientStatus::NotInstalled,
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        ClientStatus::NotInstalled
    }
}

#[cfg(target_os = "windows")]
async fn check_windows_client() -> Result<String, String> {
    // Check Windows registry for installed client
    // HKEY_LOCAL_MACHINE\SOFTWARE\GhostLink\Version
    if let Some(window) = web_sys::window() {
        // Use web API to check for protocol handler registration
        if window.location().protocol().unwrap_or_default() == "https:" {
            // Try to detect ghostlink:// protocol support
            match window.location().href() {
                Ok(_) => {
                    // TODO: Implement actual client detection
                    Ok("1.0.0".to_string())
                }
                Err(_) => Err("Client not found".to_string())
            }
        } else {
            Err("Client not found".to_string())
        }
    } else {
        Err("No window object".to_string())
    }
}

#[cfg(target_os = "macos")]
async fn check_macos_client() -> Result<String, String> {
    // Check for GhostLink.app in /Applications
    Err("Not implemented".to_string())
}

#[cfg(target_os = "linux")]
async fn check_linux_client() -> Result<String, String> {
    // Check for ghostlink-viewer in PATH
    Err("Not implemented".to_string())
}

/// Launch native client with session parameters
async fn launch_native_session(device_id: &str, session_type: SessionType) -> Result<(), String> {
    let session_url = format!("ghostlink://connect/{}/{:?}", device_id, session_type);

    if let Some(window) = web_sys::window() {
        // Try to launch via custom protocol
        match window.location().assign(&session_url) {
            Ok(_) => Ok(()),
            Err(_) => {
                // Fallback: Try alternative launch methods
                launch_via_file_download(device_id, session_type).await
            }
        }
    } else {
        Err("No window object available".to_string())
    }
}

/// Download and install native client
fn download_and_install_client() {
    if let Some(window) = web_sys::window() {
        let download_url = get_client_download_url();
        let _ = window.open_with_url(&download_url);
    }
}

fn get_client_download_url() -> String {
    let platform = get_platform();
    format!("/downloads/ghostlink-client-{}.exe", platform)
}

fn get_platform() -> &'static str {
    #[cfg(target_os = "windows")]
    return "windows";

    #[cfg(target_os = "macos")]
    return "macos";

    #[cfg(target_os = "linux")]
    return "linux";

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    return "unknown";
}

/// Fallback: Download a launcher file that opens the native client
async fn launch_via_file_download(device_id: &str, session_type: SessionType) -> Result<(), String> {
    let launcher_url = format!("/launch/{}/{:?}.ghostlink", device_id, session_type);

    if let Some(window) = web_sys::window() {
        let _ = window.location().assign(&launcher_url);
        Ok(())
    } else {
        Err("Cannot download launcher".to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionType {
    Control,
    View,
    FileTransfer,
    Terminal,
}

#[derive(Debug, Clone)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub hostname: String,
    pub platform: String,
    pub is_online: bool,
}

// Component stubs - you'd implement these
#[component]
fn DeviceIcon(platform: String, online: bool) -> impl IntoView {
    view! { <i class="bi bi-pc-display"></i> }
}

#[component]
fn OnlineStatusBadge(online: bool) -> impl IntoView {
    view! {
        <span class={if online { "badge bg-success" } else { "badge bg-secondary" }}>
            {if online { "Online" } else { "Offline" }}
        </span>
    }
}

#[component]
fn DeviceSpecs(device: Device) -> impl IntoView {
    view! {
        <div class="small text-muted">
            <div>{device.platform}</div>
            <div>{device.hostname}</div>
        </div>
    }
}