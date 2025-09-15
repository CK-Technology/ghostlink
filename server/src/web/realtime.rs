use leptos::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use web_sys::{MessageEvent, WebSocket, CloseEvent, Event};
use tracing::{info, warn, error};
use std::collections::HashMap;

/// Real-time WebSocket client for live updates
#[derive(Clone)]
pub struct RealtimeClient {
    websocket: Option<WebSocket>,
    connection_state: RwSignal<ConnectionState>,
    message_handler: RwSignal<Option<Box<dyn Fn(RealtimeMessage)>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RealtimeMessage {
    DeviceStatusUpdate {
        device_id: String,
        is_online: bool,
        last_seen: String,
    },
    SessionUpdate {
        session_id: String,
        device_id: String,
        status: String,
        user_name: String,
    },
    SystemStats {
        connected_devices: usize,
        active_sessions: usize,
        bandwidth_usage: f64,
        cpu_usage: f64,
    },
    DeviceConnected {
        device_id: String,
        device_name: String,
        platform: String,
    },
    DeviceDisconnected {
        device_id: String,
    },
    Notification {
        level: String, // "info", "warning", "error"
        title: String,
        message: String,
        timestamp: String,
    },
}

impl RealtimeClient {
    pub fn new() -> Self {
        Self {
            websocket: None,
            connection_state: create_rw_signal(ConnectionState::Disconnected),
            message_handler: create_rw_signal(None),
        }
    }

    pub fn connection_state(&self) -> ReadSignal<ConnectionState> {
        self.connection_state.read_only()
    }

    pub fn connect(&mut self, url: &str) -> Result<(), JsValue> {
        if matches!(self.connection_state.get(), ConnectionState::Connected | ConnectionState::Connecting) {
            return Ok(());
        }

        self.connection_state.set(ConnectionState::Connecting);

        let ws = WebSocket::new(url)?;
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        // Clone signals for closures
        let connection_state = self.connection_state;
        let message_handler = self.message_handler;

        // Handle WebSocket open
        let onopen_callback = Closure::wrap(Box::new(move |_: Event| {
            info!("WebSocket connected");
            connection_state.set(ConnectionState::Connected);
        }) as Box<dyn FnMut(Event)>);
        ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
        onopen_callback.forget();

        // Handle WebSocket messages
        let onmessage_callback = Closure::wrap(Box::new(move |e: MessageEvent| {
            if let Ok(text) = e.data().dyn_into::<js_sys::JsString>() {
                let message_str = String::from(text);
                match serde_json::from_str::<RealtimeMessage>(&message_str) {
                    Ok(message) => {
                        if let Some(handler) = message_handler.get() {
                            handler(message);
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse WebSocket message: {}", e);
                    }
                }
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        onmessage_callback.forget();

        // Handle WebSocket close
        let connection_state_close = self.connection_state;
        let onclose_callback = Closure::wrap(Box::new(move |e: CloseEvent| {
            warn!("WebSocket closed: {} - {}", e.code(), e.reason());
            connection_state_close.set(ConnectionState::Disconnected);
            // TODO: Implement auto-reconnection logic
        }) as Box<dyn FnMut(CloseEvent)>);
        ws.set_onclose(Some(onclose_callback.as_ref().unchecked_ref()));
        onclose_callback.forget();

        // Handle WebSocket errors
        let connection_state_error = self.connection_state;
        let onerror_callback = Closure::wrap(Box::new(move |_: Event| {
            error!("WebSocket error occurred");
            connection_state_error.set(ConnectionState::Error("Connection error".to_string()));
        }) as Box<dyn FnMut(Event)>);
        ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
        onerror_callback.forget();

        self.websocket = Some(ws);
        Ok(())
    }

    pub fn disconnect(&mut self) {
        if let Some(ws) = &self.websocket {
            let _ = ws.close();
        }
        self.websocket = None;
        self.connection_state.set(ConnectionState::Disconnected);
    }

    pub fn send_message(&self, message: &RealtimeMessage) -> Result<(), JsValue> {
        if let Some(ws) = &self.websocket {
            let json = serde_json::to_string(message).map_err(|e| {
                JsValue::from_str(&format!("Serialization error: {}", e))
            })?;
            ws.send_with_str(&json)
        } else {
            Err(JsValue::from_str("WebSocket not connected"))
        }
    }

    pub fn set_message_handler<F>(&self, handler: F)
    where
        F: Fn(RealtimeMessage) + 'static,
    {
        self.message_handler.set(Some(Box::new(handler)));
    }
}

/// Real-time dashboard updates component
#[component]
pub fn RealtimeDashboard() -> impl IntoView {
    let (realtime_client, set_realtime_client) = create_signal(RealtimeClient::new());
    let (notifications, set_notifications) = create_signal(Vec::<RealtimeMessage>::new());
    let (system_stats, set_system_stats) = create_signal(None::<SystemStats>);
    let (device_updates, set_device_updates) = create_signal(HashMap::<String, DeviceStatus>::new());

    #[derive(Clone, Debug)]
    struct SystemStats {
        connected_devices: usize,
        active_sessions: usize,
        bandwidth_usage: f64,
        cpu_usage: f64,
    }

    #[derive(Clone, Debug)]
    struct DeviceStatus {
        device_id: String,
        is_online: bool,
        last_seen: String,
    }

    // Initialize WebSocket connection
    create_effect(move |_| {
        let mut client = realtime_client.get();

        // Set up message handler
        client.set_message_handler(move |message| {
            match message {
                RealtimeMessage::DeviceStatusUpdate { device_id, is_online, last_seen } => {
                    set_device_updates.update(|updates| {
                        updates.insert(device_id.clone(), DeviceStatus {
                            device_id,
                            is_online,
                            last_seen,
                        });
                    });
                }
                RealtimeMessage::SystemStats { connected_devices, active_sessions, bandwidth_usage, cpu_usage } => {
                    set_system_stats.set(Some(SystemStats {
                        connected_devices,
                        active_sessions,
                        bandwidth_usage,
                        cpu_usage,
                    }));
                }
                RealtimeMessage::Notification { .. } => {
                    set_notifications.update(|notifs| {
                        notifs.push(message.clone());
                        // Keep only last 10 notifications
                        if notifs.len() > 10 {
                            notifs.remove(0);
                        }
                    });
                }
                RealtimeMessage::DeviceConnected { device_id, device_name, platform } => {
                    set_notifications.update(|notifs| {
                        notifs.push(RealtimeMessage::Notification {
                            level: "info".to_string(),
                            title: "Device Connected".to_string(),
                            message: format!("{} ({}) connected", device_name, platform),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        });
                    });
                }
                RealtimeMessage::DeviceDisconnected { device_id } => {
                    set_notifications.update(|notifs| {
                        notifs.push(RealtimeMessage::Notification {
                            level: "warning".to_string(),
                            title: "Device Disconnected".to_string(),
                            message: format!("Device {} disconnected", device_id),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        });
                    });
                }
                _ => {}
            }
        });

        // Connect to WebSocket
        if let Err(e) = client.connect("ws://localhost:8080/ws/realtime") {
            error!("Failed to connect to WebSocket: {:?}", e);
        }

        set_realtime_client.set(client);
    });

    view! {
        <div class="realtime-dashboard">
            // Connection status indicator
            <div class="row mb-3">
                <div class="col-12">
                    <ConnectionStatus client=realtime_client />
                </div>
            </div>

            // Real-time system stats
            {move || {
                if let Some(stats) = system_stats.get() {
                    view! {
                        <div class="row g-3 mb-4">
                            <div class="col-md-3">
                                <div class="card bg-primary text-white">
                                    <div class="card-body">
                                        <div class="d-flex align-items-center">
                                            <i class="bi bi-hdd-network fs-1 me-3"></i>
                                            <div>
                                                <div class="fs-4 fw-bold">{stats.connected_devices}</div>
                                                <div class="small">"Devices Online"</div>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>
                            <div class="col-md-3">
                                <div class="card bg-success text-white">
                                    <div class="card-body">
                                        <div class="d-flex align-items-center">
                                            <i class="bi bi-display fs-1 me-3"></i>
                                            <div>
                                                <div class="fs-4 fw-bold">{stats.active_sessions}</div>
                                                <div class="small">"Active Sessions"</div>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>
                            <div class="col-md-3">
                                <div class="card bg-info text-white">
                                    <div class="card-body">
                                        <div class="d-flex align-items-center">
                                            <i class="bi bi-speedometer2 fs-1 me-3"></i>
                                            <div>
                                                <div class="fs-4 fw-bold">{format!("{:.1}%", stats.cpu_usage)}</div>
                                                <div class="small">"CPU Usage"</div>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>
                            <div class="col-md-3">
                                <div class="card bg-warning text-dark">
                                    <div class="card-body">
                                        <div class="d-flex align-items-center">
                                            <i class="bi bi-wifi fs-1 me-3"></i>
                                            <div>
                                                <div class="fs-4 fw-bold">{format!("{:.1} MB/s", stats.bandwidth_usage)}</div>
                                                <div class="small">"Bandwidth"</div>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </div>
                    }.into_view()
                } else {
                    view! {
                        <div class="row mb-4">
                            <div class="col-12">
                                <div class="card">
                                    <div class="card-body text-center">
                                        <div class="spinner-border text-primary me-2" role="status"></div>
                                        "Loading real-time stats..."
                                    </div>
                                </div>
                            </div>
                        </div>
                    }.into_view()
                }
            }}

            // Real-time notifications
            <div class="row">
                <div class="col-12">
                    <NotificationPanel notifications=notifications />
                </div>
            </div>
        </div>
    }
}

#[component]
fn ConnectionStatus(client: ReadSignal<RealtimeClient>) -> impl IntoView {
    let connection_state = create_memo(move |_| {
        client.with(|c| c.connection_state().get())
    });

    view! {
        <div class="connection-status">
            {move || {
                match connection_state.get() {
                    ConnectionState::Connected => {
                        view! {
                            <div class="alert alert-success d-flex align-items-center" role="alert">
                                <i class="bi bi-wifi me-2"></i>
                                <span>"Real-time updates connected"</span>
                                <div class="ms-auto">
                                    <span class="badge bg-success">
                                        <i class="bi bi-circle-fill me-1" style="font-size: 0.5rem;"></i>
                                        "LIVE"
                                    </span>
                                </div>
                            </div>
                        }.into_view()
                    }
                    ConnectionState::Connecting => {
                        view! {
                            <div class="alert alert-info d-flex align-items-center" role="alert">
                                <div class="spinner-border spinner-border-sm me-2" role="status"></div>
                                <span>"Connecting to real-time updates..."</span>
                            </div>
                        }.into_view()
                    }
                    ConnectionState::Reconnecting => {
                        view! {
                            <div class="alert alert-warning d-flex align-items-center" role="alert">
                                <div class="spinner-border spinner-border-sm me-2" role="status"></div>
                                <span>"Reconnecting to real-time updates..."</span>
                            </div>
                        }.into_view()
                    }
                    ConnectionState::Disconnected => {
                        view! {
                            <div class="alert alert-secondary d-flex align-items-center" role="alert">
                                <i class="bi bi-wifi-off me-2"></i>
                                <span>"Real-time updates disconnected"</span>
                            </div>
                        }.into_view()
                    }
                    ConnectionState::Error(msg) => {
                        view! {
                            <div class="alert alert-danger d-flex align-items-center" role="alert">
                                <i class="bi bi-exclamation-triangle me-2"></i>
                                <span>{format!("Connection error: {}", msg)}</span>
                            </div>
                        }.into_view()
                    }
                }
            }}
        </div>
    }
}

#[component]
fn NotificationPanel(notifications: ReadSignal<Vec<RealtimeMessage>>) -> impl IntoView {
    view! {
        <div class="card">
            <div class="card-header">
                <h6 class="mb-0">
                    <i class="bi bi-bell me-2"></i>
                    "Real-time Notifications"
                </h6>
            </div>
            <div class="card-body p-0">
                <div class="list-group list-group-flush" style="max-height: 300px; overflow-y: auto;">
                    <For
                        each=move || notifications.get()
                        key=|notification| {
                            if let RealtimeMessage::Notification { timestamp, .. } = notification {
                                timestamp.clone()
                            } else {
                                chrono::Utc::now().to_rfc3339()
                            }
                        }
                        children=move |notification| {
                            if let RealtimeMessage::Notification { level, title, message, timestamp } = notification {
                                let (icon, class) = match level.as_str() {
                                    "error" => ("bi-exclamation-triangle-fill", "text-danger"),
                                    "warning" => ("bi-exclamation-triangle", "text-warning"),
                                    "info" => ("bi-info-circle", "text-info"),
                                    _ => ("bi-info-circle", "text-secondary"),
                                };

                                view! {
                                    <div class="list-group-item">
                                        <div class="d-flex align-items-start">
                                            <i class={format!("bi {} {} me-2 mt-1", icon, class)}></i>
                                            <div class="flex-grow-1">
                                                <div class="fw-bold small">{title}</div>
                                                <div class="text-muted small">{message}</div>
                                            </div>
                                            <small class="text-muted">{format_timestamp(&timestamp)}</small>
                                        </div>
                                    </div>
                                }.into_view()
                            } else {
                                view! {}.into_view()
                            }
                        }
                    />
                </div>
            </div>
        </div>
    }
}

fn format_timestamp(timestamp: &str) -> String {
    // Simple timestamp formatting - in production you'd use a proper date library
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(timestamp) {
        dt.format("%H:%M:%S").to_string()
    } else {
        "N/A".to_string()
    }
}