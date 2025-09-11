use leptos::*;
use leptos_router::*;
use crate::web::api_client::*;
use std::time::Duration;

#[component]
pub fn Dashboard() -> impl IntoView {
    let (devices, set_devices) = create_signal(Vec::<Device>::new());
    let (stats, set_stats) = create_signal(None::<ServerStats>);
    let (loading, set_loading) = create_signal(true);
    let (error, set_error) = create_signal(None::<String>);
    let (search_filter, set_search_filter) = create_signal(String::new());
    let (platform_filter, set_platform_filter) = create_signal(String::new());

    // Load initial data
    create_effect(move |_| {
        spawn_local(async move {
            set_loading.set(true);
            
            match ApiClient::get_devices().await {
                Ok(device_list) => {
                    set_devices.set(device_list);
                    set_error.set(None);
                }
                Err(e) => {
                    set_error.set(Some(format!("Failed to load devices: {}", e)));
                }
            }

            match ApiClient::get_stats().await {
                Ok(server_stats) => {
                    set_stats.set(Some(server_stats));
                }
                Err(e) => {
                    logging::log!("Failed to load stats: {}", e);
                }
            }
            
            set_loading.set(false);
        });
    });

    // Set up auto-refresh every 30 seconds
    create_effect(move |_| {
        set_interval(
            move || {
                spawn_local(async move {
                    if let Ok(device_list) = ApiClient::get_devices().await {
                        set_devices.set(device_list);
                    }
                    if let Ok(server_stats) = ApiClient::get_stats().await {
                        set_stats.set(Some(server_stats));
                    }
                });
            },
            Duration::from_secs(30),
        );
    });

    // Filter devices based on search and platform
    let filtered_devices = create_memo(move |_| {
        let devices_list = devices.get();
        let search = search_filter.get().to_lowercase();
        let platform = platform_filter.get().to_lowercase();

        devices_list.into_iter().filter(|device| {
            let matches_search = if search.is_empty() {
                true
            } else {
                device.name.to_lowercase().contains(&search) ||
                device.hostname.to_lowercase().contains(&search) ||
                device.platform.to_lowercase().contains(&search)
            };

            let matches_platform = if platform.is_empty() || platform == "all" {
                true
            } else {
                device.platform.to_lowercase() == platform
            };

            matches_search && matches_platform
        }).collect::<Vec<_>>()
    });

    view! {
        <div class="dashboard">
            // Header with stats
            <div class="row mb-4">
                <div class="col-12">
                    <div class="d-flex justify-content-between align-items-center">
                        <h1 class="h3 mb-0 text-dark">
                            <i class="bi bi-speedometer2 me-2 text-primary"></i>
                            "Dashboard"
                        </h1>
                        <div class="d-flex gap-2">
                            <button 
                                class="btn btn-outline-primary btn-sm"
                                on:click=move |_| {
                                    spawn_local(async move {
                                        set_loading.set(true);
                                        if let Ok(device_list) = ApiClient::get_devices().await {
                                            set_devices.set(device_list);
                                        }
                                        if let Ok(server_stats) = ApiClient::get_stats().await {
                                            set_stats.set(Some(server_stats));
                                        }
                                        set_loading.set(false);
                                    });
                                }
                            >
                                <i class="bi bi-arrow-clockwise me-1"></i>
                                "Refresh"
                            </button>
                        </div>
                    </div>
                </div>
            </div>

            // Stats cards
            {move || {
                if let Some(server_stats) = stats.get() {
                    view! {
                        <div class="row g-3 mb-4">
                            <div class="col-lg-3 col-md-6">
                                <StatsCard
                                    icon="bi-hdd-network"
                                    title="Connected Devices"
                                    value=server_stats.connected_devices.to_string()
                                    color="primary"
                                />
                            </div>
                            <div class="col-lg-3 col-md-6">
                                <StatsCard
                                    icon="bi-display"
                                    title="Active Sessions"
                                    value=server_stats.active_sessions.to_string()
                                    color="success"
                                />
                            </div>
                            <div class="col-lg-3 col-md-6">
                                <StatsCard
                                    icon="bi-windows"
                                    title="Windows"
                                    value=server_stats.devices_by_platform.get("windows").unwrap_or(&0).to_string()
                                    color="info"
                                />
                            </div>
                            <div class="col-lg-3 col-md-6">
                                <StatsCard
                                    icon="bi-ubuntu"
                                    title="Linux"
                                    value=server_stats.devices_by_platform.get("linux").unwrap_or(&0).to_string()
                                    color="warning"
                                />
                            </div>
                        </div>
                    }.into_view()
                } else {
                    view! {
                        <div class="row g-3 mb-4">
                            <div class="col-12">
                                <div class="text-center text-muted">
                                    <div class="spinner-border spinner-border-sm me-2" role="status"></div>
                                    "Loading stats..."
                                </div>
                            </div>
                        </div>
                    }.into_view()
                }
            }}

            // Filter controls
            <div class="row mb-4">
                <div class="col-12">
                    <div class="card">
                        <div class="card-body">
                            <div class="row g-3 align-items-end">
                                <div class="col-md-6">
                                    <label for="search" class="form-label small text-muted">
                                        "Search devices"
                                    </label>
                                    <div class="input-group">
                                        <span class="input-group-text">
                                            <i class="bi bi-search"></i>
                                        </span>
                                        <input
                                            type="text"
                                            class="form-control"
                                            id="search"
                                            placeholder="Search by name, hostname, or platform..."
                                            prop:value=move || search_filter.get()
                                            on:input=move |ev| {
                                                set_search_filter.set(event_target_value(&ev));
                                            }
                                        />
                                    </div>
                                </div>
                                <div class="col-md-3">
                                    <label for="platform" class="form-label small text-muted">
                                        "Platform"
                                    </label>
                                    <select 
                                        class="form-select"
                                        id="platform"
                                        on:change=move |ev| {
                                            set_platform_filter.set(event_target_value(&ev));
                                        }
                                    >
                                        <option value="all">"All Platforms"</option>
                                        <option value="windows">"Windows"</option>
                                        <option value="linux">"Linux"</option>
                                        <option value="macos">"macOS"</option>
                                        <option value="android">"Android"</option>
                                        <option value="ios">"iOS"</option>
                                    </select>
                                </div>
                                <div class="col-md-3">
                                    <div class="text-end">
                                        <span class="text-muted small">
                                            {move || format!("{} devices found", filtered_devices.get().len())}
                                        </span>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            // Error display
            {move || {
                if let Some(error_msg) = error.get() {
                    view! {
                        <div class="row mb-4">
                            <div class="col-12">
                                <div class="alert alert-danger d-flex align-items-center" role="alert">
                                    <i class="bi bi-exclamation-triangle-fill me-2"></i>
                                    <div>{error_msg}</div>
                                    <button 
                                        type="button" 
                                        class="btn-close ms-auto" 
                                        on:click=move |_| set_error.set(None)
                                    ></button>
                                </div>
                            </div>
                        </div>
                    }.into_view()
                } else {
                    view! {}.into_view()
                }
            }}

            // Loading state
            {move || {
                if loading.get() {
                    view! {
                        <div class="row">
                            <div class="col-12">
                                <div class="card">
                                    <div class="card-body text-center py-5">
                                        <div class="spinner-border text-primary mb-3" role="status">
                                            <span class="visually-hidden">"Loading..."</span>
                                        </div>
                                        <p class="text-muted mb-0">"Loading devices..."</p>
                                    </div>
                                </div>
                            </div>
                        </div>
                    }.into_view()
                } else {
                    view! {}.into_view()
                }
            }}

            // Device grid
            {move || {
                let filtered = filtered_devices.get();
                if !loading.get() {
                    if filtered.is_empty() {
                        view! {
                            <div class="row">
                                <div class="col-12">
                                    <div class="card">
                                        <div class="card-body text-center py-5">
                                            <i class="bi bi-hdd-network-fill text-muted mb-3" style="font-size: 3rem;"></i>
                                            <h5 class="card-title">"No devices found"</h5>
                                            <p class="card-text text-muted">
                                                "No devices match your current filters. Try adjusting your search criteria."
                                            </p>
                                            <button 
                                                class="btn btn-outline-primary"
                                                on:click=move |_| {
                                                    set_search_filter.set(String::new());
                                                    set_platform_filter.set(String::new());
                                                }
                                            >
                                                <i class="bi bi-arrow-clockwise me-1"></i>
                                                "Clear Filters"
                                            </button>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        }.into_view()
                    } else {
                        view! {
                            <div class="row g-3">
                                <For
                                    each=move || filtered_devices.get()
                                    key=|device| device.id.clone()
                                    children=move |device| {
                                        view! {
                                            <div class="col-xl-4 col-lg-6">
                                                <DeviceCard device=device />
                                            </div>
                                        }
                                    }
                                />
                            </div>
                        }.into_view()
                    }
                } else {
                    view! {}.into_view()
                }
            }}
        </div>
    }
}

#[component]
fn StatsCard(
    icon: &'static str,
    title: &'static str,
    value: String,
    color: &'static str,
) -> impl IntoView {
    view! {
        <div class="card h-100 border-0 shadow-sm">
            <div class="card-body">
                <div class="row align-items-center">
                    <div class="col">
                        <div class={format!("text-{} mb-2", color)}>
                            <i class={format!("bi {} fs-4", icon)}></i>
                        </div>
                        <div class="h5 mb-1 font-weight-bold">{value}</div>
                        <div class="text-muted small">{title}</div>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn DeviceCard(device: Device) -> impl IntoView {
    let (sessions, set_sessions) = create_signal(Vec::<Session>::new());
    let (connecting, set_connecting) = create_signal(false);

    // Load sessions for this device
    let device_id = device.id.clone();
    create_effect(move |_| {
        let device_id = device_id.clone();
        spawn_local(async move {
            if let Ok(device_sessions) = ApiClient::get_device_sessions(&device_id).await {
                set_sessions.set(device_sessions);
            }
        });
    });

    let status_badge_class = get_status_badge_class(device.is_online);
    let status_icon = get_status_icon(device.is_online);
    let platform_icon = get_platform_icon(&device.platform);

    let handle_connect = {
        let device_id = device.id.clone();
        move |session_type: SessionType| {
            let device_id = device_id.clone();
            spawn_local(async move {
                set_connecting.set(true);
                
                let request = CreateSessionRequest {
                    session_type,
                    user_id: None, // Will use current user
                };

                match ApiClient::create_session(&device_id, request).await {
                    Ok(session_id) => {
                        logging::log!("Session created: {}", session_id);
                        // Navigate to session view or open in popup
                        if let Some(window) = web_sys::window() {
                            let session_url = format!("/session/{}", session_id);
                            let _ = window.open_with_url_and_target(&session_url, "_blank");
                        }
                    }
                    Err(e) => {
                        logging::log!("Failed to create session: {}", e);
                        // Show error notification
                    }
                }
                
                set_connecting.set(false);
            });
        }
    };

    view! {
        <div class="card h-100 shadow-sm border-0 device-card">
            <div class="card-body">
                <div class="d-flex justify-content-between align-items-start mb-3">
                    <div class="d-flex align-items-center">
                        <div class={format!("text-{} me-2", if device.is_online { "success" } else { "muted" })}>
                            <i class={format!("bi {} fs-5", platform_icon)}></i>
                        </div>
                        <div>
                            <h6 class="card-title mb-0 fw-bold">{device.name.clone()}</h6>
                            <small class="text-muted">{device.hostname.clone()}</small>
                        </div>
                    </div>
                    <span class={format!("badge {}", status_badge_class)}>
                        <i class={format!("bi {} me-1", status_icon)}></i>
                        {if device.is_online { "Online" } else { "Offline" }}
                    </span>
                </div>
                
                <div class="row text-sm text-muted mb-3">
                    <div class="col-6">
                        <div class="d-flex align-items-center mb-1">
                            <i class="bi bi-cpu me-2"></i>
                            <span class="small">{device.platform.clone()} {device.architecture.clone()}</span>
                        </div>
                        <div class="d-flex align-items-center">
                            <i class="bi bi-clock me-2"></i>
                            <span class="small">
                                {if let Some(last_seen) = &device.last_seen {
                                    format_timestamp(last_seen)
                                } else {
                                    "Never".to_string()
                                }}
                            </span>
                        </div>
                    </div>
                    <div class="col-6">
                        <div class="d-flex align-items-center mb-1">
                            <i class="bi bi-tag me-2"></i>
                            <span class="small">
                                {if device.tags.is_empty() {
                                    "No tags".to_string()
                                } else {
                                    device.tags.join(", ")
                                }}
                            </span>
                        </div>
                        <div class="d-flex align-items-center">
                            <i class="bi bi-display me-2"></i>
                            <span class="small">{sessions.get().len()} " sessions"</span>
                        </div>
                    </div>
                </div>

                // Action buttons
                <div class="d-grid gap-2">
                    <button
                        class="btn btn-primary btn-sm"
                        disabled=move || !device.is_online || connecting.get()
                        on:click={
                            let handle = handle_connect.clone();
                            move |_| handle(SessionType::Control)
                        }
                    >
                        {move || if connecting.get() {
                            view! {
                                <span>
                                    <span class="spinner-border spinner-border-sm me-2" role="status"></span>
                                    "Connecting..."
                                </span>
                            }.into_view()
                        } else {
                            view! {
                                <span>
                                    <i class="bi bi-display me-1"></i>
                                    "Control"
                                </span>
                            }.into_view()
                        }}
                    </button>
                    
                    <div class="btn-group" role="group">
                        <button 
                            type="button" 
                            class="btn btn-outline-secondary btn-sm"
                            disabled=move || !device.is_online
                            title="View Only"
                            on:click={
                                let handle = handle_connect.clone();
                                move |_| handle(SessionType::View)
                            }
                        >
                            <i class="bi bi-eye"></i>
                        </button>
                        <button 
                            type="button" 
                            class="btn btn-outline-secondary btn-sm"
                            disabled=move || !device.is_online
                            title="File Transfer"
                            on:click={
                                let handle = handle_connect.clone();
                                move |_| handle(SessionType::FileTransfer)
                            }
                        >
                            <i class="bi bi-folder"></i>
                        </button>
                        <button 
                            type="button" 
                            class="btn btn-outline-secondary btn-sm"
                            disabled=move || !device.is_online
                            title="Terminal"
                            on:click={
                                let handle = handle_connect.clone();
                                move |_| handle(SessionType::Terminal)
                            }
                        >
                            <i class="bi bi-terminal"></i>
                        </button>
                        <button 
                            type="button" 
                            class="btn btn-outline-secondary btn-sm"
                            title="Device Settings"
                        >
                            <i class="bi bi-gear"></i>
                        </button>
                    </div>
                </div>
            </div>
        </div>
    }
}

fn set_interval<F>(f: F, delay: Duration) 
where
    F: Fn() + 'static,
{
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;
    
    let closure = Closure::wrap(Box::new(f) as Box<dyn Fn()>);
    
    web_sys::window()
        .unwrap()
        .set_interval_with_callback_and_timeout_and_arguments_0(
            closure.as_ref().unchecked_ref(),
            delay.as_millis() as i32,
        )
        .expect("should register interval");
        
    closure.forget();
}