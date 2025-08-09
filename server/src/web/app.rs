use leptos::*;
use leptos_meta::*;
use leptos_router::*;

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Html lang="en" dir="ltr" attr:data-theme="light"/>
        <Title text="AtlasConnect - Remote Access Platform"/>
        <Meta charset="utf-8"/>
        <Meta name="viewport" content="width=device-width, initial-scale=1"/>
        <Meta name="description" content="AtlasConnect - Secure remote access platform"/>
        
        <Link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/css/bootstrap.min.css"/>
        <Link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bootstrap-icons@1.10.0/font/bootstrap-icons.css"/>
        
        <Body class="bg-light"/>
        <Router>
            <Routes>
                <Route path="/" view=MainLayout>
                    <Route path="" view=Dashboard/>
                    <Route path="/sessions" view=Sessions/>
                    <Route path="/settings" view=Settings/>
                </Route>
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
fn Dashboard() -> impl IntoView {
    let (devices, _set_devices) = create_signal(vec![
        Device {
            id: "device-001".to_string(),
            name: "Windows Workstation".to_string(),
            os: "Windows 11".to_string(),
            status: "online".to_string(),
            last_seen: "2025-07-02T10:30:00Z".to_string(),
            ip_address: "192.168.1.100".to_string(),
        },
        Device {
            id: "device-002".to_string(),
            name: "Ubuntu Server".to_string(),
            os: "Ubuntu 22.04".to_string(),
            status: "online".to_string(),
            last_seen: "2025-07-02T10:29:45Z".to_string(),
            ip_address: "192.168.1.50".to_string(),
        },
    ]);

    view! {
        <div class="row">
            <div class="col-12">
                <div class="d-flex justify-content-between align-items-center mb-4">
                    <h1 class="h3 mb-0">"Connected Devices"</h1>
                    <div class="d-flex gap-2">
                        <button class="btn btn-outline-primary btn-sm">
                            <i class="bi bi-arrow-clockwise me-1"></i>
                            "Refresh"
                        </button>
                        <button class="btn btn-primary btn-sm">
                            <i class="bi bi-plus-lg me-1"></i>
                            "Add Device"
                        </button>
                    </div>
                </div>
            </div>
        </div>
        
        <div class="row g-3">
            <For
                each=move || devices.get()
                key=|device| device.id.clone()
                children=move |device| {
                    view! {
                        <div class="col-lg-4 col-md-6">
                            <DeviceCard device=device/>
                        </div>
                    }
                }
            />
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
    view! {
        <header class="bg-white border-bottom px-4 py-3">
            <div class="d-flex justify-content-between align-items-center">
                <div class="d-flex align-items-center">
                    <h2 class="h5 mb-0 me-3">"AtlasConnect"</h2>
                    <span class="badge bg-primary">"Server Online"</span>
                </div>
                
                <div class="d-flex align-items-center gap-3">
                    <div class="text-sm text-muted">
                        "Connected: 2 devices"
                    </div>
                    <div class="dropdown">
                        <button class="btn btn-link text-decoration-none p-0" type="button" data-bs-toggle="dropdown">
                            <i class="bi bi-person-circle fs-4"></i>
                        </button>
                        <ul class="dropdown-menu dropdown-menu-end">
                            <li><a class="dropdown-item" href="#">"Profile"</a></li>
                            <li><a class="dropdown-item" href="#">"Settings"</a></li>
                            <li><hr class="dropdown-divider"/></li>
                            <li><a class="dropdown-item" href="#">"Sign out"</a></li>
                        </ul>
                    </div>
                </div>
            </div>
        </header>
    }
}

#[component]
fn Sidebar() -> impl IntoView {
    view! {
        <nav class="bg-dark text-white p-3" style="width: 250px;">
            <div class="mb-4">
                <h4 class="mb-0">"AtlasConnect"</h4>
                <small class="text-muted">"Remote Access Platform"</small>
            </div>
            
            <ul class="nav nav-pills flex-column">
                <li class="nav-item mb-1">
                    <A href="/" class="nav-link text-white">"Dashboard"</A>
                </li>
                <li class="nav-item mb-1">
                    <A href="/sessions" class="nav-link text-white">"Active Sessions"</A>
                </li>
                <li class="nav-item">
                    <A href="/settings" class="nav-link text-white">"Settings"</A>
                </li>
            </ul>
        </nav>
    }
}
