use leptos::*;
use serde::{Deserialize, Serialize};
use web_sys::{WebSocket, MessageEvent};
use wasm_bindgen::prelude::*;
use wasm_bindgen::closure::Closure;

/// Monitor information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MonitorInfo {
    pub id: u32,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
    pub scale_factor: f32,
}

/// Monitor selection mode
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MonitorSelection {
    All,
    Primary,
    Specific(u32),
    Custom { x: i32, y: i32, width: u32, height: u32 },
}

/// Monitor control message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorControlMessage {
    pub action: String,
    pub selection: MonitorSelection,
    pub quality: Option<String>,
    pub fps: Option<u32>,
}

/// Multi-monitor selection component
#[component]
pub fn MonitorSelector(
    device_id: String,
    session_id: String,
) -> impl IntoView {
    let (monitors, set_monitors) = create_signal(Vec::<MonitorInfo>::new());
    let (selected, set_selected) = create_signal(MonitorSelection::Primary);
    let (preview_mode, set_preview_mode) = create_signal(false);
    let (custom_region, set_custom_region) = create_signal((0, 0, 1920, 1080));
    let (quality, set_quality) = create_signal("balanced".to_string());
    let (fps, set_fps) = create_signal(30u32);
    let (ws, set_ws) = create_signal(None::<WebSocket>);
    
    // Connect to monitor control WebSocket
    create_effect(move |_| {
        let window = web_sys::window().expect("no window");
        let location = window.location();
        let protocol = if location.protocol().unwrap_or_default() == "https:" { "wss:" } else { "ws:" };
        let host = location.host().unwrap_or_default();
        
        let ws_url = format!("{}//{}/api/monitor/ws?session_id={}", protocol, host, session_id);
        
        match WebSocket::new(&ws_url) {
            Ok(websocket) => {
                // Set up message handler
                let onmessage_callback = Closure::<dyn FnMut(_)>::new(move |e: MessageEvent| {
                    if let Ok(text) = e.data().dyn_into::<js_sys::JsString>() {
                        if let Ok(msg) = serde_json::from_str::<Vec<MonitorInfo>>(&text.as_string().unwrap()) {
                            set_monitors.set(msg);
                        }
                    }
                });
                websocket.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
                onmessage_callback.forget();
                
                set_ws.set(Some(websocket));
            }
            Err(e) => {
                logging::error!("Failed to create WebSocket: {:?}", e);
            }
        }
    });
    
    // Send monitor selection update
    let send_selection = move |selection: MonitorSelection| {
        if let Some(websocket) = ws.get() {
            let msg = MonitorControlMessage {
                action: "select".to_string(),
                selection: selection.clone(),
                quality: Some(quality.get()),
                fps: Some(fps.get()),
            };
            
            if let Ok(json) = serde_json::to_string(&msg) {
                let _ = websocket.send_with_str(&json);
            }
        }
        set_selected.set(selection);
    };

    view! {
        <div class="monitor-selector card">
            <div class="card-header bg-primary text-white">
                <h5 class="mb-0">
                    <i class="bi bi-display me-2"></i>
                    "Monitor Selection"
                </h5>
            </div>
            <div class="card-body">
                // Monitor grid visualization
                <div class="monitor-grid mb-4">
                    <svg viewBox="0 0 800 400" class="w-100 border rounded">
                        <For
                            each=move || monitors.get()
                            key=|monitor| monitor.id
                            children=move |monitor: MonitorInfo| {
                                let is_selected = move || match selected.get() {
                                    MonitorSelection::All => true,
                                    MonitorSelection::Primary => monitor.is_primary,
                                    MonitorSelection::Specific(id) => id == monitor.id,
                                    MonitorSelection::Custom { .. } => false,
                                };
                                
                                let x_scale = 800.0 / 5120.0; // Assume max 5K width
                                let y_scale = 400.0 / 2880.0; // Assume max 2880 height
                                
                                view! {
                                    <g>
                                        <rect
                                            x=(monitor.x as f32 * x_scale).to_string()
                                            y=(monitor.y as f32 * y_scale).to_string()
                                            width=(monitor.width as f32 * x_scale).to_string()
                                            height=(monitor.height as f32 * y_scale).to_string()
                                            fill=move || if is_selected() { "#0d6efd" } else { "#e9ecef" }
                                            stroke="#495057"
                                            stroke-width="2"
                                            rx="4"
                                            class="cursor-pointer"
                                            on:click=move |_| send_selection(MonitorSelection::Specific(monitor.id))
                                        />
                                        <text
                                            x=((monitor.x as f32 + monitor.width as f32 / 2.0) * x_scale).to_string()
                                            y=((monitor.y as f32 + monitor.height as f32 / 2.0) * y_scale).to_string()
                                            text-anchor="middle"
                                            dominant-baseline="middle"
                                            fill=move || if is_selected() { "white" } else { "#495057" }
                                            font-size="14"
                                            font-weight="bold"
                                        >
                                            {monitor.name.clone()}
                                        </text>
                                        <Show when=move || monitor.is_primary>
                                            <text
                                                x=((monitor.x as f32 + monitor.width as f32 / 2.0) * x_scale).to_string()
                                                y=((monitor.y as f32 + monitor.height as f32 / 2.0 + 20.0) * y_scale).to_string()
                                                text-anchor="middle"
                                                fill=move || if is_selected() { "white" } else { "#6c757d" }
                                                font-size="10"
                                            >
                                                "(Primary)"
                                            </text>
                                        </Show>
                                    </g>
                                }
                            }
                        />
                    </svg>
                </div>
                
                // Selection options
                <div class="row mb-3">
                    <div class="col-md-6">
                        <label class="form-label">"Selection Mode"</label>
                        <div class="btn-group w-100" role="group">
                            <button
                                type="button"
                                class=move || {
                                    format!("btn btn-outline-primary {}",
                                        if matches!(selected.get(), MonitorSelection::All) { "active" } else { "" })
                                }
                                on:click=move |_| send_selection(MonitorSelection::All)
                            >
                                <i class="bi bi-grid-3x2-gap me-1"></i>
                                "All Monitors"
                            </button>
                            <button
                                type="button"
                                class=move || {
                                    format!("btn btn-outline-primary {}",
                                        if matches!(selected.get(), MonitorSelection::Primary) { "active" } else { "" })
                                }
                                on:click=move |_| send_selection(MonitorSelection::Primary)
                            >
                                <i class="bi bi-display me-1"></i>
                                "Primary Only"
                            </button>
                            <button
                                type="button"
                                class=move || {
                                    format!("btn btn-outline-primary {}",
                                        if matches!(selected.get(), MonitorSelection::Custom { .. }) { "active" } else { "" })
                                }
                                on:click=move |_| set_preview_mode.set(true)
                            >
                                <i class="bi bi-crop me-1"></i>
                                "Custom Region"
                            </button>
                        </div>
                    </div>
                    
                    <div class="col-md-3">
                        <label class="form-label">"Quality"</label>
                        <select
                            class="form-select"
                            on:change=move |ev| {
                                set_quality.set(event_target_value(&ev));
                                send_selection(selected.get());
                            }
                        >
                            <option value="ultra">"Ultra (Lossless)"</option>
                            <option value="high">"High"</option>
                            <option value="balanced" selected>"Balanced"</option>
                            <option value="low">"Low (Save Bandwidth)"</option>
                        </select>
                    </div>
                    
                    <div class="col-md-3">
                        <label class="form-label">"Frame Rate"</label>
                        <select
                            class="form-select"
                            on:change=move |ev| {
                                set_fps.set(event_target_value(&ev).parse().unwrap_or(30));
                                send_selection(selected.get());
                            }
                        >
                            <option value="15">"15 FPS"</option>
                            <option value="30" selected>"30 FPS"</option>
                            <option value="60">"60 FPS"</option>
                            <option value="120">"120 FPS (High-end)"</option>
                        </select>
                    </div>
                </div>
                
                // Monitor details
                <div class="monitor-details">
                    <h6>"Active Monitors"</h6>
                    <div class="row g-2">
                        <For
                            each=move || monitors.get()
                            key=|monitor| monitor.id
                            children=move |monitor: MonitorInfo| {
                                view! {
                                    <div class="col-md-6">
                                        <div class="card border-secondary">
                                            <div class="card-body p-2">
                                                <div class="d-flex justify-content-between align-items-center">
                                                    <div>
                                                        <strong>{monitor.name}</strong>
                                                        <Show when=move || monitor.is_primary>
                                                            <span class="badge bg-primary ms-2">"Primary"</span>
                                                        </Show>
                                                    </div>
                                                    <button
                                                        class="btn btn-sm btn-outline-primary"
                                                        on:click=move |_| send_selection(MonitorSelection::Specific(monitor.id))
                                                    >
                                                        "Select"
                                                    </button>
                                                </div>
                                                <small class="text-muted">
                                                    {format!("{}x{} @ ({}, {})", 
                                                        monitor.width, monitor.height, monitor.x, monitor.y)}
                                                </small>
                                            </div>
                                        </div>
                                    </div>
                                }
                            }
                        />
                    </div>
                </div>
                
                // Custom region selector (modal-like)
                <Show when=preview_mode>
                    <div class="custom-region-selector mt-3 p-3 border rounded bg-light">
                        <h6>"Custom Region Selection"</h6>
                        <div class="row g-2">
                            <div class="col-md-3">
                                <label class="form-label">"X Position"</label>
                                <input
                                    type="number"
                                    class="form-control"
                                    value=custom_region.get().0
                                    on:input=move |ev| {
                                        let val = event_target_value(&ev).parse().unwrap_or(0);
                                        set_custom_region.update(|(x, y, w, h)| *x = val);
                                    }
                                />
                            </div>
                            <div class="col-md-3">
                                <label class="form-label">"Y Position"</label>
                                <input
                                    type="number"
                                    class="form-control"
                                    value=custom_region.get().1
                                    on:input=move |ev| {
                                        let val = event_target_value(&ev).parse().unwrap_or(0);
                                        set_custom_region.update(|(x, y, w, h)| *y = val);
                                    }
                                />
                            </div>
                            <div class="col-md-3">
                                <label class="form-label">"Width"</label>
                                <input
                                    type="number"
                                    class="form-control"
                                    value=custom_region.get().2
                                    on:input=move |ev| {
                                        let val = event_target_value(&ev).parse().unwrap_or(1920);
                                        set_custom_region.update(|(x, y, w, h)| *w = val);
                                    }
                                />
                            </div>
                            <div class="col-md-3">
                                <label class="form-label">"Height"</label>
                                <input
                                    type="number"
                                    class="form-control"
                                    value=custom_region.get().3
                                    on:input=move |ev| {
                                        let val = event_target_value(&ev).parse().unwrap_or(1080);
                                        set_custom_region.update(|(x, y, w, h)| *h = val);
                                    }
                                />
                            </div>
                        </div>
                        <div class="mt-2">
                            <button
                                class="btn btn-primary me-2"
                                on:click=move |_| {
                                    let (x, y, w, h) = custom_region.get();
                                    send_selection(MonitorSelection::Custom { x, y, width: w, height: h });
                                    set_preview_mode.set(false);
                                }
                            >
                                "Apply Region"
                            </button>
                            <button
                                class="btn btn-secondary"
                                on:click=move |_| set_preview_mode.set(false)
                            >
                                "Cancel"
                            </button>
                        </div>
                    </div>
                </Show>
            </div>
        </div>
    }
}