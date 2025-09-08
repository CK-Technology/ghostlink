use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use crate::web::sessions::{SessionsPage, SessionViewer};

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Html lang="en" dir="ltr" attr:data-theme="light"/>
        <Title text="GhostLink - Remote Access Platform"/>
        <Meta charset="utf-8"/>
        <Meta name="viewport" content="width=device-width, initial-scale=1"/>
        <Meta name="description" content="GhostLink - Secure remote access platform"/>
        
        <Link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.2/dist/css/bootstrap.min.css"/>
        <Link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bootstrap-icons@1.11.0/font/bootstrap-icons.css"/>
        <Link rel="stylesheet" href="/assets/app.css"/>
        
        <Script src="https://cdn.jsdelivr.net/npm/bootstrap@5.3.2/dist/js/bootstrap.bundle.min.js"></Script>
        
        <Body class="bg-light"/>
        <Router>
            <Routes>
                <Route path="/" view=MainLayout>
                    <Route path="" view=crate::web::dashboard::Dashboard/>
                    <Route path="/sessions" view=SessionsPage/>
                    <Route path="/settings" view=Settings/>
                </Route>
                <Route path="/session/:id" view=SessionViewer/>
            </Routes>
        </Router>
    }
}

#[component]
fn MainLayout() -> impl IntoView {
    view! {
        <div class="d-flex vh-100">
            <Sidebar/>
            <main class="flex-grow-1 overflow-auto">
                <Header/>
                <div class="container-fluid p-4">
                    <Outlet/>
                </div>
            </main>
        </div>
    }
}


#[component]
fn Sessions() -> impl IntoView {
    view! {
        <div class="row">
            <div class="col-12">
                <h1 class="h3 mb-4">"Active Sessions"</h1>
                <div class="card">
                    <div class="card-body text-center py-5">
                        <i class="bi bi-display text-muted" style="font-size: 3rem;"></i>
                        <h5 class="card-title mt-3">"No Active Sessions"</h5>
                        <p class="card-text text-muted">"Connect to a device to start a remote session."</p>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn Settings() -> impl IntoView {
    view! {
        <div class="row">
            <div class="col-12">
                <h1 class="h3 mb-4">"Settings"</h1>
                <div class="card">
                    <div class="card-body">
                        <h5 class="card-title">"Server Configuration"</h5>
                        <p class="card-text">"Settings panel coming soon..."</p>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[derive(Clone, Debug)]
struct Device {
    id: String,
    name: String,
    os: String,
    status: String,
    last_seen: String,
    ip_address: String,
}

#[component]
fn DeviceCard(device: Device) -> impl IntoView {
    let status_class = match device.status.as_str() {
        "online" => "text-success",
        "offline" => "text-danger",
        _ => "text-warning",
    };
    
    let status_icon = match device.status.as_str() {
        "online" => "bi-circle-fill",
        "offline" => "bi-circle",
        _ => "bi-exclamation-circle",
    };

    let device_id = device.id.clone();
    
    view! {
        <div class="card h-100">
            <div class="card-body">
                <div class="d-flex justify-content-between align-items-start mb-3">
                    <h6 class="card-title mb-0">{device.name}</h6>
                    <span class={format!("badge {}", if device.status == "online" { "bg-success" } else { "bg-secondary" })}>
                        <i class={format!("bi {} me-1", status_icon)}></i>
                        {device.status.to_uppercase()}
                    </span>
                </div>
                
                <div class="small text-muted mb-3">
                    <div class="d-flex align-items-center mb-1">
                        <i class="bi bi-laptop me-2"></i>
                        {device.os}
                    </div>
                    <div class="d-flex align-items-center mb-1">
                        <i class="bi bi-wifi me-2"></i>
                        {device.ip_address}
                    </div>
                    <div class="d-flex align-items-center">
                        <i class="bi bi-clock me-2"></i>
                        "Last seen: " {device.last_seen}
                    </div>
                </div>
                
                <div class="d-grid gap-2">
                    <button 
                        class="btn btn-primary btn-sm"
                        disabled={device.status != "online"}
                        on:click=move |_| {
                            // This will eventually launch a remote session
                            tracing::info!("Launching remote session for device: {}", device_id);
                        }
                    >
                        <i class="bi bi-display me-1"></i>
                        "Connect"
                    </button>
                    
                    <div class="btn-group" role="group">
                        <button type="button" class="btn btn-outline-secondary btn-sm">
                            <i class="bi bi-folder"></i>
                        </button>
                        <button type="button" class="btn btn-outline-secondary btn-sm">
                            <i class="bi bi-terminal"></i>
                        </button>
                        <button type="button" class="btn btn-outline-secondary btn-sm">
                            <i class="bi bi-gear"></i>
                        </button>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn Header() -> impl IntoView {
    let (server_stats, _set_server_stats) = create_signal(None::<crate::web::api_client::ServerStats>);
    
    // Load server stats for display
    create_effect(move |_| {
        spawn_local(async move {
            if let Ok(stats) = crate::web::api_client::ApiClient::get_stats().await {
                _set_server_stats.set(Some(stats));
            }
        });
    });

    view! {
        <header class="bg-white border-bottom shadow-sm">
            <div class="container-fluid">
                <div class="d-flex justify-content-between align-items-center py-3">
                    <div class="d-flex align-items-center">
                        <div class="d-flex align-items-center me-4">
                            <div class="text-primary me-2">
                                <i class="bi bi-shield-check fs-4"></i>
                            </div>
                            <div>
                                <h1 class="h5 mb-0 fw-bold text-dark">"GhostLink"</h1>
                                <small class="text-muted">"Remote Access Platform"</small>
                            </div>
                        </div>
                        <span class="badge bg-success-subtle text-success border border-success-subtle">
                            <i class="bi bi-circle-fill me-1" style="font-size: 0.5rem;"></i>
                            "Server Online"
                        </span>
                    </div>
                    
                    <div class="d-flex align-items-center gap-4">
                        <div class="d-none d-md-flex align-items-center gap-4 text-sm">
                            {move || {
                                if let Some(stats) = server_stats.get() {
                                    view! {
                                        <div class="d-flex align-items-center text-muted">
                                            <i class="bi bi-hdd-network me-1"></i>
                                            <span class="fw-medium text-dark me-1">{stats.connected_devices}</span>
                                            "devices"
                                        </div>
                                        <div class="d-flex align-items-center text-muted">
                                            <i class="bi bi-display me-1"></i>
                                            <span class="fw-medium text-dark me-1">{stats.active_sessions}</span>
                                            "sessions"
                                        </div>
                                    }.into_view()
                                } else {
                                    view! {
                                        <div class="d-flex align-items-center text-muted">
                                            <div class="spinner-border spinner-border-sm me-2" role="status"></div>
                                            "Loading..."
                                        </div>
                                    }.into_view()
                                }
                            }}
                        </div>
                        
                        <div class="vr d-none d-md-block"></div>
                        
                        <div class="dropdown">
                            <button 
                                class="btn btn-outline-secondary d-flex align-items-center" 
                                type="button" 
                                data-bs-toggle="dropdown"
                                aria-expanded="false"
                            >
                                <i class="bi bi-person-circle me-2"></i>
                                <span class="d-none d-sm-inline">"Administrator"</span>
                                <i class="bi bi-chevron-down ms-2"></i>
                            </button>
                            <ul class="dropdown-menu dropdown-menu-end shadow">
                                <li>
                                    <h6 class="dropdown-header">
                                        <i class="bi bi-person-badge me-1"></i>
                                        "Account"
                                    </h6>
                                </li>
                                <li><a class="dropdown-item" href="#"><i class="bi bi-person me-2"></i>"Profile"</a></li>
                                <li><a class="dropdown-item" href="#"><i class="bi bi-gear me-2"></i>"Settings"</a></li>
                                <li><a class="dropdown-item" href="#"><i class="bi bi-shield-check me-2"></i>"Security"</a></li>
                                <li><hr class="dropdown-divider"/></li>
                                <li><a class="dropdown-item text-danger" href="#"><i class="bi bi-box-arrow-right me-2"></i>"Sign Out"</a></li>
                            </ul>
                        </div>
                    </div>
                </div>
            </div>
        </header>
    }
}

#[component]
fn Sidebar() -> impl IntoView {
    view! {
        <nav class="sidebar bg-dark border-end" style="width: 280px;">
            <div class="d-flex flex-column h-100">
                <div class="p-4 border-bottom border-secondary">
                    <div class="d-flex align-items-center text-white">
                        <div class="text-primary me-3">
                            <i class="bi bi-shield-shaded fs-3"></i>
                        </div>
                        <div>
                            <h4 class="mb-0 fw-bold">"GhostLink"</h4>
                            <small class="text-light opacity-75">"Remote Access"</small>
                        </div>
                    </div>
                </div>
                
                <div class="flex-grow-1 p-3">
                    <div class="mb-4">
                        <h6 class="text-uppercase text-light opacity-75 fw-bold mb-3 small">"Navigation"</h6>
                        <ul class="nav nav-pills flex-column gap-1">
                            <li class="nav-item">
                                <A 
                                    href="/" 
                                    class="nav-link text-light d-flex align-items-center py-2 px-3 rounded"
                                    active_class="active bg-primary"
                                >
                                    <i class="bi bi-speedometer2 me-3"></i>
                                    "Dashboard"
                                </A>
                            </li>
                            <li class="nav-item">
                                <A 
                                    href="/sessions" 
                                    class="nav-link text-light d-flex align-items-center py-2 px-3 rounded"
                                    active_class="active bg-primary"
                                >
                                    <i class="bi bi-display me-3"></i>
                                    "Active Sessions"
                                </A>
                            </li>
                            <li class="nav-item">
                                <A 
                                    href="/settings" 
                                    class="nav-link text-light d-flex align-items-center py-2 px-3 rounded"
                                    active_class="active bg-primary"
                                >
                                    <i class="bi bi-gear me-3"></i>
                                    "Settings"
                                </A>
                            </li>
                        </ul>
                    </div>
                    
                    <div class="mb-4">
                        <h6 class="text-uppercase text-light opacity-75 fw-bold mb-3 small">"Quick Actions"</h6>
                        <div class="d-grid gap-2">
                            <button class="btn btn-outline-light btn-sm">
                                <i class="bi bi-plus-circle me-2"></i>
                                "Add Device"
                            </button>
                            <button class="btn btn-outline-secondary btn-sm">
                                <i class="bi bi-collection me-2"></i>
                                "Device Groups"
                            </button>
                        </div>
                    </div>
                    
                    <div class="mb-4">
                        <h6 class="text-uppercase text-light opacity-75 fw-bold mb-3 small">"Platform Status"</h6>
                        <div class="row g-2 text-center">
                            <div class="col-6">
                                <div class="bg-secondary bg-opacity-25 rounded p-2">
                                    <i class="bi bi-windows text-info fs-5 d-block mb-1"></i>
                                    <small class="text-light d-block">Windows</small>
                                </div>
                            </div>
                            <div class="col-6">
                                <div class="bg-secondary bg-opacity-25 rounded p-2">
                                    <i class="bi bi-ubuntu text-warning fs-5 d-block mb-1"></i>
                                    <small class="text-light d-block">Linux</small>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
                
                <div class="p-3 border-top border-secondary">
                    <div class="bg-secondary bg-opacity-25 rounded p-3">
                        <div class="d-flex align-items-center text-light">
                            <i class="bi bi-info-circle me-2"></i>
                            <div class="flex-grow-1">
                                <div class="small fw-medium">"Server Health"</div>
                                <div class="small opacity-75">"All systems operational"</div>
                            </div>
                            <div class="text-success">
                                <i class="bi bi-check-circle-fill"></i>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </nav>
    }
}
