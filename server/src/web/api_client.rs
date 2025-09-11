use leptos::*;
use serde::{Deserialize, Serialize};
use serde_json;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response};
use gloo_utils::format::JsValueSerdeExt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub hostname: String,
    pub platform: String,
    pub architecture: String,
    pub version: String,
    pub last_seen: Option<String>,
    pub is_online: bool,
    pub owner_id: String,
    pub group_id: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub id: String,
    pub agent_id: String,
    pub user_id: String,
    pub session_type: SessionType,
    pub status: SessionStatus,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub ip_address: String,
    pub user_agent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionType {
    View,
    Control,
    FileTransfer,
    Terminal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionStatus {
    Active,
    Connecting,
    Ended,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStats {
    pub connected_devices: usize,
    pub active_sessions: usize,
    pub devices_by_platform: std::collections::HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    pub session_type: SessionType,
    pub user_id: Option<String>,
}

pub struct ApiClient;

impl ApiClient {
    /// Fetch all connected devices
    pub async fn get_devices() -> Result<Vec<Device>, String> {
        let response = Self::fetch("/api/devices", "GET", None::<()>).await?;
        let json: serde_json::Value = response.into_serde()
            .map_err(|e| format!("Failed to parse JSON: {}", e))?;
        
        if let Some(devices) = json.get("devices") {
            serde_json::from_value(devices.clone())
                .map_err(|e| format!("Failed to parse devices: {}", e))
        } else {
            Err("Invalid response format".to_string())
        }
    }

    /// Get server statistics
    pub async fn get_stats() -> Result<ServerStats, String> {
        let response = Self::fetch("/api/stats", "GET", None::<()>).await?;
        response.into_serde()
            .map_err(|e| format!("Failed to parse stats: {}", e))
    }

    /// Get sessions for a specific device
    pub async fn get_device_sessions(device_id: &str) -> Result<Vec<Session>, String> {
        let url = format!("/api/devices/{}/sessions", device_id);
        let response = Self::fetch(&url, "GET", None::<()>).await?;
        let json: serde_json::Value = response.into_serde()
            .map_err(|e| format!("Failed to parse JSON: {}", e))?;
        
        if let Some(sessions) = json.get("sessions") {
            serde_json::from_value(sessions.clone())
                .map_err(|e| format!("Failed to parse sessions: {}", e))
        } else {
            Err("Invalid response format".to_string())
        }
    }

    /// Create a new session with a device
    pub async fn create_session(device_id: &str, request: CreateSessionRequest) -> Result<String, String> {
        let url = format!("/api/devices/{}/sessions", device_id);
        let response = Self::fetch(&url, "POST", Some(request)).await?;
        let json: serde_json::Value = response.into_serde()
            .map_err(|e| format!("Failed to parse JSON: {}", e))?;
        
        if let Some(session_id) = json.get("session_id").and_then(|v| v.as_str()) {
            Ok(session_id.to_string())
        } else {
            Err("Failed to create session".to_string())
        }
    }

    /// End a session
    pub async fn end_session(session_id: &str) -> Result<(), String> {
        let url = format!("/api/sessions/{}", session_id);
        Self::fetch(&url, "DELETE", None::<()>).await?;
        Ok(())
    }

    /// Generic fetch function
    async fn fetch<T: Serialize>(
        url: &str,
        method: &str,
        body: Option<T>,
    ) -> Result<JsValue, String> {
        let mut opts = RequestInit::new();
        opts.method(method);
        opts.mode(RequestMode::Cors);

        if let Some(body_data) = body {
            let body_str = serde_json::to_string(&body_data)
                .map_err(|e| format!("Failed to serialize request: {}", e))?;
            opts.body(Some(&JsValue::from_str(&body_str)));
        }

        let request = Request::new_with_str_and_init(url, &opts)
            .map_err(|_| "Failed to create request".to_string())?;

        request
            .headers()
            .set("Content-Type", "application/json")
            .map_err(|_| "Failed to set content type".to_string())?;

        let window = web_sys::window().ok_or("No window available")?;
        let resp_value = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|_| "Request failed".to_string())?;

        let resp: Response = resp_value
            .dyn_into()
            .map_err(|_| "Failed to cast response".to_string())?;

        if !resp.ok() {
            return Err(format!("Request failed with status: {}", resp.status()));
        }

        let json = JsFuture::from(resp.json().map_err(|_| "Failed to get JSON from response")?)
            .await
            .map_err(|_| "Failed to parse response JSON".to_string())?;

        Ok(json)
    }
}

/// Create a WebSocket connection for real-time session data
pub fn create_session_websocket(session_id: &str) -> Result<web_sys::WebSocket, String> {
    let window = web_sys::window().ok_or("No window available")?;
    let location = window.location();
    let protocol = if location.protocol().unwrap_or_default() == "https:" { "wss:" } else { "ws:" };
    let host = location.host().map_err(|_| "Failed to get host")?;
    
    let ws_url = format!("{}//{}//api/ws?session_id={}&type=viewer", protocol, host, session_id);
    
    web_sys::WebSocket::new(&ws_url)
        .map_err(|_| "Failed to create WebSocket".to_string())
}

/// Format timestamp for display
pub fn format_timestamp(timestamp: &str) -> String {
    // In a real implementation, you'd parse and format the timestamp properly
    // For now, just return a formatted version
    if let Ok(datetime) = chrono::DateTime::parse_from_rfc3339(timestamp) {
        datetime.format("%Y-%m-%d %H:%M:%S").to_string()
    } else {
        timestamp.to_string()
    }
}

/// Get status badge class for devices
pub fn get_status_badge_class(is_online: bool) -> &'static str {
    if is_online { "bg-success" } else { "bg-secondary" }
}

/// Get status icon for devices
pub fn get_status_icon(is_online: bool) -> &'static str {
    if is_online { "bi-circle-fill" } else { "bi-circle" }
}

/// Get platform icon for devices
pub fn get_platform_icon(platform: &str) -> &'static str {
    match platform.to_lowercase().as_str() {
        "windows" => "bi-windows",
        "linux" => "bi-ubuntu",
        "macos" | "darwin" => "bi-apple",
        "android" => "bi-android2",
        "ios" => "bi-apple",
        _ => "bi-laptop",
    }
}