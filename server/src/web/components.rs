use leptos::*;
use leptos_router::*;
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub os: String,
    pub status: String,
    pub last_seen: String,
    pub ip_address: String,
}

#[component]
pub fn DeviceCard(device: Device) -> impl IntoView {
    let status_class = match device.status.as_str() {
        "online" => "status-online",
        "offline" => "status-offline",
        _ => "status-unknown",
    };

    view! {
        <div class="device-card">
            <div class="device-header">
                <h3 class="device-name">{device.name}</h3>
                <span class={format!("device-status {}", status_class)}>
                    {device.status.clone()}
                </span>
            </div>
            <div class="device-details">
                <p class="device-os">{device.os}</p>
                <p class="device-ip">{device.ip_address}</p>
                <p class="device-last-seen">Last seen: {device.last_seen}</p>
            </div>
            <div class="device-actions">
                <button 
                    class="btn btn-primary"
                    on:click={
                        let device_id = device.id.clone();
                        move |_| {
                            // Launch remote session
                            launch_remote_session(device_id.clone());
                        }
                    }
                >
                    "Connect"
                </button>
                <button class="btn btn-secondary">
                    "File Transfer"
                </button>
                <button class="btn btn-secondary">
                    "Terminal"
                </button>
            </div>
        </div>
    }
}

#[component]
pub fn Header() -> impl IntoView {
    view! {
        <header class="app-header">
            <div class="header-content">
                <div class="logo">
                    <h1>"AtlasConnect"</h1>
                </div>
                <nav class="header-nav">
                    <A href="/" class="nav-link">"Dashboard"</A>
                    <A href="/sessions" class="nav-link">"Active Sessions"</A>
                    <A href="/settings" class="nav-link">"Settings"</A>
                </nav>
                <div class="user-menu">
                    <button class="btn btn-outline">"Admin"</button>
                </div>
            </div>
        </header>
    }
}

#[component]
pub fn Sidebar() -> impl IntoView {
    view! {
        <aside class="sidebar">
            <div class="sidebar-content">
                <div class="sidebar-section">
                    <h3>"Quick Actions"</h3>
                    <button class="sidebar-btn">"Add Device"</button>
                    <button class="sidebar-btn">"Create Group"</button>
                </div>
                <div class="sidebar-section">
                    <h3>"Device Groups"</h3>
                    <ul class="device-groups">
                        <li><a href="#" class="group-link">"All Devices"</a></li>
                        <li><a href="#" class="group-link">"Servers"</a></li>
                        <li><a href="#" class="group-link">"Workstations"</a></li>
                        <li><a href="#" class="group-link">"Mobile"</a></li>
                    </ul>
                </div>
            </div>
        </aside>
    }
}

fn launch_remote_session(device_id: String) {
    // This will trigger the native client to launch
    info!("Launching remote session for device: {}", device_id);
    
    // In a real implementation, this would:
    // 1. Create a session token
    // 2. Launch the native client with the session details
    // 3. Establish WebSocket connection for real-time control
    
    if let Some(window) = web_sys::window() {
        let url = format!("atlasconnect://connect/{}", device_id);
        let _ = window.open_with_url(&url);
    }
}
