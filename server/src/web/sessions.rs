use leptos::*;
use leptos_router::*;
use crate::web::api_client::*;
use web_sys::WebSocket;
use wasm_bindgen::prelude::*;
use wasm_bindgen::closure::Closure;
use std::time::Duration;

#[component]
pub fn SessionsPage() -> impl IntoView {
    let (sessions, set_sessions) = create_signal(Vec::<Session>::new());
    let (devices, set_devices) = create_signal(Vec::<Device>::new());
    let (loading, set_loading) = create_signal(true);
    let (error, set_error) = create_signal(None::<String>);

    // Load initial data
    create_effect(move |_| {
        spawn_local(async move {
            set_loading.set(true);
            
            // Load devices first to get session data from each
            match ApiClient::get_devices().await {
                Ok(device_list) => {
                    set_devices.set(device_list.clone());
                    
                    // Load sessions for all devices
                    let mut all_sessions = Vec::new();
                    for device in device_list {
                        if let Ok(device_sessions) = ApiClient::get_device_sessions(&device.id).await {
                            all_sessions.extend(device_sessions);
                        }
                    }
                    set_sessions.set(all_sessions);
                    set_error.set(None);
                }
                Err(e) => {
                    set_error.set(Some(format!("Failed to load sessions: {}", e)));
                }
            }
            
            set_loading.set(false);
        });
    });

    // Auto-refresh every 10 seconds
    create_effect(move |_| {
        set_interval(
            move || {
                spawn_local(async move {
                    if let Ok(device_list) = ApiClient::get_devices().await {
                        let mut all_sessions = Vec::new();
                        for device in device_list {
                            if let Ok(device_sessions) = ApiClient::get_device_sessions(&device.id).await {
                                all_sessions.extend(device_sessions);
                            }
                        }
                        set_sessions.set(all_sessions);
                    }
                });
            },
            Duration::from_secs(10),
        );
    });

    // Filter active sessions only
    let active_sessions = create_memo(move |_| {
        sessions.get().into_iter()
            .filter(|session| matches!(session.status, SessionStatus::Active | SessionStatus::Connecting))
            .collect::<Vec<_>>()
    });

    view! {
        <div class="sessions-page">
            <div class="row mb-4">
                <div class="col-12">
                    <div class="d-flex justify-content-between align-items-center">
                        <h1 class="h3 mb-0 text-dark">
                            <i class="bi bi-display me-2 text-primary"></i>
                            "Active Sessions"
                        </h1>
                        <div class="d-flex gap-2">
                            <button 
                                class="btn btn-outline-primary btn-sm"
                                on:click=move |_| {
                                    spawn_local(async move {
                                        set_loading.set(true);
                                        if let Ok(device_list) = ApiClient::get_devices().await {
                                            let mut all_sessions = Vec::new();
                                            for device in device_list {
                                                if let Ok(device_sessions) = ApiClient::get_device_sessions(&device.id).await {
                                                    all_sessions.extend(device_sessions);
                                                }
                                            }
                                            set_sessions.set(all_sessions);
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

            // Error display
            {move || {
                if let Some(error_msg) = error.get() {
                    view! {
                        <div class="row mb-4">
                            <div class="col-12">
                                <div class="alert alert-danger d-flex align-items-center" role="alert">
                                    <i class="bi bi-exclamation-triangle-fill me-2"></i>
                                    <div>{error_msg}</div>
                                </div>
                            </div>
                        </div>
                    }.into_view()
                } else {
                    view! {}.into_view()
                }
            }}

            // Stats summary
            <div class="row g-3 mb-4">
                <div class="col-lg-3 col-md-6">
                    <div class="card border-0 shadow-sm">
                        <div class="card-body">
                            <div class="d-flex align-items-center">
                                <div class="text-primary me-3">
                                    <i class="bi bi-display fs-2"></i>
                                </div>
                                <div>
                                    <div class="h4 mb-0">{move || active_sessions.get().len()}</div>
                                    <div class="text-muted small">"Active Sessions"</div>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
                <div class="col-lg-3 col-md-6">
                    <div class="card border-0 shadow-sm">
                        <div class="card-body">
                            <div class="d-flex align-items-center">
                                <div class="text-success me-3">
                                    <i class="bi bi-hdd-network fs-2"></i>
                                </div>
                                <div>
                                    <div class="h4 mb-0">{move || devices.get().iter().filter(|d| d.is_online).count()}</div>
                                    <div class="text-muted small">"Online Devices"</div>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
                <div class="col-lg-3 col-md-6">
                    <div class="card border-0 shadow-sm">
                        <div class="card-body">
                            <div class="d-flex align-items-center">
                                <div class="text-info me-3">
                                    <i class="bi bi-eye fs-2"></i>
                                </div>
                                <div>
                                    <div class="h4 mb-0">
                                        {move || active_sessions.get().iter().filter(|s| matches!(s.session_type, SessionType::View)).count()}
                                    </div>
                                    <div class="text-muted small">"View Sessions"</div>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
                <div class="col-lg-3 col-md-6">
                    <div class="card border-0 shadow-sm">
                        <div class="card-body">
                            <div class="d-flex align-items-center">
                                <div class="text-warning me-3">
                                    <i class="bi bi-joystick fs-2"></i>
                                </div>
                                <div>
                                    <div class="h4 mb-0">
                                        {move || active_sessions.get().iter().filter(|s| matches!(s.session_type, SessionType::Control)).count()}
                                    </div>
                                    <div class="text-muted small">"Control Sessions"</div>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

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
                                        <p class="text-muted mb-0">"Loading sessions..."</p>
                                    </div>
                                </div>
                            </div>
                        </div>
                    }.into_view()
                } else {
                    view! {}.into_view()
                }
            }}

            // Sessions list
            {move || {
                let active = active_sessions.get();
                if !loading.get() {
                    if active.is_empty() {
                        view! {
                            <div class="row">
                                <div class="col-12">
                                    <div class="card">
                                        <div class="card-body text-center py-5">
                                            <i class="bi bi-display text-muted mb-3" style="font-size: 4rem;"></i>
                                            <h5 class="card-title">"No Active Sessions"</h5>
                                            <p class="card-text text-muted mb-4">
                                                "There are currently no active remote sessions. Connect to a device from the dashboard to start a session."
                                            </p>
                                            <A href="/" class="btn btn-primary">
                                                <i class="bi bi-speedometer2 me-1"></i>
                                                "Go to Dashboard"
                                            </A>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        }.into_view()
                    } else {
                        view! {
                            <div class="row">
                                <div class="col-12">
                                    <div class="card">
                                        <div class="card-header bg-white">
                                            <div class="d-flex justify-content-between align-items-center">
                                                <h6 class="mb-0 fw-bold">"Active Sessions"</h6>
                                                <small class="text-muted">{active.len()}" sessions"</small>
                                            </div>
                                        </div>
                                        <div class="card-body p-0">
                                            <div class="table-responsive">
                                                <table class="table table-hover mb-0">
                                                    <thead class="table-light">
                                                        <tr>
                                                            <th>"Device"</th>
                                                            <th>"Type"</th>
                                                            <th>"Status"</th>
                                                            <th>"Duration"</th>
                                                            <th>"User"</th>
                                                            <th>"Actions"</th>
                                                        </tr>
                                                    </thead>
                                                    <tbody>
                                                        <For
                                                            each=move || active_sessions.get()
                                                            key=|session| session.id.clone()
                                                            children=move |session| {
                                                                view! {
                                                                    <SessionRow session=session devices=devices.get() />
                                                                }
                                                            }
                                                        />
                                                    </tbody>
                                                </table>
                                            </div>
                                        </div>
                                    </div>
                                </div>
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
fn SessionRow(session: Session, devices: Vec<Device>) -> impl IntoView {
    let device = devices.iter().find(|d| d.id == session.agent_id).cloned();
    
    let session_type_icon = match session.session_type {
        SessionType::View => "bi-eye",
        SessionType::Control => "bi-joystick",
        SessionType::FileTransfer => "bi-folder",
        SessionType::Terminal => "bi-terminal",
    };

    let session_type_color = match session.session_type {
        SessionType::View => "text-info",
        SessionType::Control => "text-warning",
        SessionType::FileTransfer => "text-success",
        SessionType::Terminal => "text-primary",
    };

    let status_badge = match session.status {
        SessionStatus::Active => ("bg-success", "Active"),
        SessionStatus::Connecting => ("bg-warning", "Connecting"),
        SessionStatus::Ended => ("bg-secondary", "Ended"),
        SessionStatus::Failed => ("bg-danger", "Failed"),
    };

    let handle_end_session = {
        let session_id = session.id.clone();
        move |_| {
            let session_id = session_id.clone();
            spawn_local(async move {
                match ApiClient::end_session(&session_id).await {
                    Ok(_) => {
                        logging::log!("Session ended: {}", session_id);
                    }
                    Err(e) => {
                        logging::log!("Failed to end session: {}", e);
                    }
                }
            });
        }
    };

    let handle_join_session = {
        let session_id = session.id.clone();
        move |_| {
            if let Some(window) = web_sys::window() {
                let session_url = format!("/session/{}", session_id);
                let _ = window.open_with_url_and_target(&session_url, "_blank");
            }
        }
    };

    view! {
        <tr>
            <td>
                <div class="d-flex align-items-center">
                    {if let Some(device) = &device {
                        view! {
                            <div class="me-2">
                                <i class={format!("bi {} text-muted", get_platform_icon(&device.platform))}></i>
                            </div>
                            <div>
                                <div class="fw-semibold">{device.name.clone()}</div>
                                <small class="text-muted">{device.hostname.clone()}</small>
                            </div>
                        }.into_view()
                    } else {
                        view! {
                            <div class="text-muted">
                                <i class="bi bi-question-circle me-2"></i>
                                "Unknown Device"
                            </div>
                        }.into_view()
                    }}
                </div>
            </td>
            <td>
                <div class="d-flex align-items-center">
                    <i class={format!("bi {} {} me-2", session_type_icon, session_type_color)}></i>
                    <span class="text-capitalize">{format!("{:?}", session.session_type)}</span>
                </div>
            </td>
            <td>
                <span class={format!("badge {}", status_badge.0)}>{status_badge.1}</span>
            </td>
            <td>
                <span class="text-muted small">
                    {format_timestamp(&session.started_at)}
                </span>
            </td>
            <td>
                <div class="d-flex align-items-center">
                    <i class="bi bi-person-circle me-2 text-muted"></i>
                    <span class="small">{session.user_id[..8].to_string()}...</span>
                </div>
            </td>
            <td>
                <div class="btn-group btn-group-sm" role="group">
                    <button
                        type="button"
                        class="btn btn-outline-primary"
                        title="Join Session"
                        disabled={!matches!(session.status, SessionStatus::Active)}
                        on:click=handle_join_session
                    >
                        <i class="bi bi-box-arrow-in-right"></i>
                    </button>
                    <button
                        type="button"
                        class="btn btn-outline-secondary"
                        title="Session Details"
                    >
                        <i class="bi bi-info-circle"></i>
                    </button>
                    <button
                        type="button"
                        class="btn btn-outline-danger"
                        title="End Session"
                        disabled={matches!(session.status, SessionStatus::Ended | SessionStatus::Failed)}
                        on:click=handle_end_session
                    >
                        <i class="bi bi-stop-circle"></i>
                    </button>
                </div>
            </td>
        </tr>
    }
}

#[component]
pub fn SessionViewer() -> impl IntoView {
    let params = use_params_map();
    let session_id = move || params.with(|p| p.get("id").cloned().unwrap_or_default());
    
    let (ws, set_ws) = create_signal(None::<WebSocket>);
    let (connected, set_connected) = create_signal(false);
    let (error, set_error) = create_signal(None::<String>);

    // Initialize WebSocket connection
    create_effect(move |_| {
        let id = session_id();
        if !id.is_empty() {
            match create_session_websocket(&id) {
                Ok(websocket) => {
                    // Set up WebSocket event handlers
                    let onopen_callback = Closure::wrap(Box::new(move |_| {
                        set_connected.set(true);
                        logging::log!("WebSocket connected for session: {}", id);
                    }) as Box<dyn FnMut(web_sys::Event)>);

                    let onerror_callback = Closure::wrap(Box::new(move |e: web_sys::ErrorEvent| {
                        set_error.set(Some("WebSocket connection error".to_string()));
                        logging::log!("WebSocket error: {:?}", e);
                    }) as Box<dyn FnMut(web_sys::ErrorEvent)>);

                    let onclose_callback = Closure::wrap(Box::new(move |_| {
                        set_connected.set(false);
                        logging::log!("WebSocket connection closed");
                    }) as Box<dyn FnMut(web_sys::CloseEvent)>);

                    websocket.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
                    websocket.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
                    websocket.set_onclose(Some(onclose_callback.as_ref().unchecked_ref()));

                    // Prevent closures from being dropped
                    onopen_callback.forget();
                    onerror_callback.forget();
                    onclose_callback.forget();

                    set_ws.set(Some(websocket));
                }
                Err(e) => {
                    set_error.set(Some(e));
                }
            }
        }
    });

    view! {
        <div class="session-viewer vh-100 d-flex flex-column">
            <div class="session-header bg-dark text-white p-3">
                <div class="d-flex justify-content-between align-items-center">
                    <div class="d-flex align-items-center">
                        <h5 class="mb-0 me-3">
                            <i class="bi bi-display me-2"></i>
                            "Session: " {session_id}
                        </h5>
                        <span class={if connected.get() { "badge bg-success" } else { "badge bg-danger" }}>
                            {if connected.get() { "Connected" } else { "Disconnected" }}
                        </span>
                    </div>
                    <div class="d-flex gap-2">
                        <button class="btn btn-outline-light btn-sm">
                            <i class="bi bi-fullscreen"></i>
                        </button>
                        <button class="btn btn-outline-light btn-sm">
                            <i class="bi bi-gear"></i>
                        </button>
                        <A href="/sessions" class="btn btn-outline-light btn-sm">
                            <i class="bi bi-x-lg"></i>
                        </A>
                    </div>
                </div>
            </div>
            
            <div class="flex-grow-1 position-relative bg-dark">
                {move || {
                    if let Some(error_msg) = error.get() {
                        view! {
                            <div class="position-absolute top-50 start-50 translate-middle text-center text-white">
                                <i class="bi bi-exclamation-triangle fs-1 mb-3"></i>
                                <h5>"Connection Error"</h5>
                                <p>{error_msg}</p>
                                <A href="/sessions" class="btn btn-outline-light">
                                    "Back to Sessions"
                                </A>
                            </div>
                        }.into_view()
                    } else if connected.get() {
                        view! {
                            <div id="remote-screen" class="w-100 h-100 position-relative">
                                // Remote screen will be rendered here via WebSocket
                                <div class="position-absolute top-50 start-50 translate-middle text-center text-white-50">
                                    <div class="spinner-border mb-3" role="status"></div>
                                    <p>"Waiting for screen data..."</p>
                                </div>
                            </div>
                        }.into_view()
                    } else {
                        view! {
                            <div class="position-absolute top-50 start-50 translate-middle text-center text-white-50">
                                <div class="spinner-border mb-3" role="status"></div>
                                <p>"Connecting to session..."</p>
                            </div>
                        }.into_view()
                    }
                }}
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