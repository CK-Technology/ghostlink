use leptos::*;
use serde::{Deserialize, Serialize};
use web_sys::{WebSocket, MessageEvent};
use wasm_bindgen::prelude::*;
use wasm_bindgen::closure::Closure;
use std::time::Duration;
use crate::web::api_client::*;

/// Device discovery status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DiscoveryStatus {
    Idle,
    Scanning,
    Found,
    Error,
}

/// Discovered device information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscoveredDevice {
    pub ip_address: String,
    pub hostname: String,
    pub mac_address: Option<String>,
    pub device_type: String,
    pub platform: String,
    pub agent_version: Option<String>,
    pub is_verified: bool,
    pub last_seen: String,
    pub signal_strength: Option<f32>,
}

/// Device invitation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInvitation {
    pub id: String,
    pub code: String,
    pub qr_code: String,
    pub expires_at: String,
    pub organization_id: String,
    pub created_by: String,
    pub used: bool,
}

/// Installation instructions for different platforms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallationInstructions {
    pub platform: String,
    pub steps: Vec<String>,
    pub download_url: String,
    pub config_template: String,
}

/// Onboarding wizard step
#[derive(Debug, Clone, PartialEq)]
pub enum OnboardingStep {
    Welcome,
    SelectMethod,
    AutoDiscovery,
    ManualEntry,
    QrCodeGeneration,
    PlatformSelection,
    InstallationGuide,
    ConfigurationFile,
    DeviceVerification,
    Complete,
}

/// Device Discovery and Auto-Detection UI Component
#[component]
pub fn DeviceDiscovery() -> impl IntoView {
    let (current_step, set_current_step) = create_signal(OnboardingStep::Welcome);
    let (discovery_status, set_discovery_status) = create_signal(DiscoveryStatus::Idle);
    let (discovered_devices, set_discovered_devices) = create_signal(Vec::<DiscoveredDevice>::new());
    let (scan_progress, set_scan_progress) = create_signal(0);
    let (error_message, set_error_message) = create_signal(None::<String>);
    let (ws, set_ws) = create_signal(None::<WebSocket>);
    let (invitation, set_invitation) = create_signal(None::<DeviceInvitation>);
    let (selected_platform, set_selected_platform) = create_signal("windows".to_string());
    let (manual_device_id, set_manual_device_id) = create_signal(String::new());
    let (manual_device_code, set_manual_device_code) = create_signal(String::new());

    // Initialize WebSocket connection for real-time discovery updates
    create_effect(move |_| {
        let window = web_sys::window().expect("no window");
        let location = window.location();
        let protocol = if location.protocol().unwrap_or_default() == "https:" { "wss:" } else { "ws:" };
        let host = location.host().unwrap_or_default();
        
        let ws_url = format!("{}//{}/api/discovery/ws", protocol, host);
        
        match WebSocket::new(&ws_url) {
            Ok(websocket) => {
                // Set up message handler
                let onmessage_callback = Closure::<dyn FnMut(_)>::new(move |e: MessageEvent| {
                    if let Ok(text) = e.data().dyn_into::<js_sys::JsString>() {
                        let msg_str = text.as_string().unwrap();
                        
                        if let Ok(devices) = serde_json::from_str::<Vec<DiscoveredDevice>>(&msg_str) {
                            set_discovered_devices.set(devices);
                            set_discovery_status.set(DiscoveryStatus::Found);
                        } else if let Ok(progress) = serde_json::from_str::<i32>(&msg_str) {
                            set_scan_progress.set(progress);
                            if progress >= 100 {
                                set_discovery_status.set(DiscoveryStatus::Found);
                            }
                        }
                    }
                });
                websocket.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
                onmessage_callback.forget();

                // Set up error handler
                let onerror_callback = Closure::<dyn FnMut(_)>::new(move |_| {
                    set_discovery_status.set(DiscoveryStatus::Error);
                    set_error_message.set(Some("Connection error during discovery".to_string()));
                });
                websocket.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
                onerror_callback.forget();
                
                set_ws.set(Some(websocket));
            }
            Err(e) => {
                logging::error!("Failed to create discovery WebSocket: {:?}", e);
                set_error_message.set(Some("Failed to connect to discovery service".to_string()));
            }
        }
    });

    let start_network_scan = move || {
        set_discovery_status.set(DiscoveryStatus::Scanning);
        set_scan_progress.set(0);
        set_error_message.set(None);
        
        if let Some(websocket) = ws.get() {
            let scan_msg = serde_json::json!({
                "action": "start_scan",
                "subnet": "auto"
            });
            let _ = websocket.send_with_str(&scan_msg.to_string());
        }
    };

    let generate_invitation = move || {
        spawn_local(async move {
            // Create device invitation
            match create_device_invitation().await {
                Ok(inv) => {
                    set_invitation.set(Some(inv));
                    set_current_step.set(OnboardingStep::QrCodeGeneration);
                }
                Err(e) => {
                    set_error_message.set(Some(format!("Failed to generate invitation: {}", e)));
                }
            }
        });
    };

    let verify_device = move |device_id: String| {
        spawn_local(async move {
            match verify_discovered_device(&device_id).await {
                Ok(_) => {
                    set_current_step.set(OnboardingStep::Complete);
                }
                Err(e) => {
                    set_error_message.set(Some(format!("Device verification failed: {}", e)));
                }
            }
        });
    };

    view! {
        <div class="device-discovery">
            <div class="container-fluid p-4">
                // Header
                <div class="row mb-4">
                    <div class="col-12">
                        <div class="card border-0 shadow-sm">
                            <div class="card-body">
                                <div class="d-flex align-items-center">
                                    <div class="text-primary me-3">
                                        <i class="bi bi-radar fs-1"></i>
                                    </div>
                                    <div>
                                        <h2 class="h4 mb-1 fw-bold">"Device Discovery & Setup"</h2>
                                        <p class="text-muted mb-0">"Add new devices to your GhostLink network"</p>
                                    </div>
                                </div>
                                
                                // Progress indicator
                                <div class="mt-3">
                                    <div class="progress" style="height: 4px;">
                                        <div 
                                            class="progress-bar bg-primary"
                                            style=move || format!("width: {}%", 
                                                match current_step.get() {
                                                    OnboardingStep::Welcome => 10,
                                                    OnboardingStep::SelectMethod => 20,
                                                    OnboardingStep::AutoDiscovery | OnboardingStep::ManualEntry => 40,
                                                    OnboardingStep::QrCodeGeneration | OnboardingStep::PlatformSelection => 60,
                                                    OnboardingStep::InstallationGuide | OnboardingStep::ConfigurationFile => 80,
                                                    OnboardingStep::DeviceVerification => 90,
                                                    OnboardingStep::Complete => 100,
                                                }
                                            )
                                        ></div>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>

                // Error display
                {move || {
                    if let Some(error) = error_message.get() {
                        view! {
                            <div class="row mb-4">
                                <div class="col-12">
                                    <div class="alert alert-danger alert-dismissible fade show" role="alert">
                                        <i class="bi bi-exclamation-triangle-fill me-2"></i>
                                        {error}
                                        <button 
                                            type="button" 
                                            class="btn-close" 
                                            on:click=move |_| set_error_message.set(None)
                                        ></button>
                                    </div>
                                </div>
                            </div>
                        }.into_view()
                    } else {
                        view! {}.into_view()
                    }
                }}

                // Main content based on current step
                {move || {
                    match current_step.get() {
                        OnboardingStep::Welcome => {
                            view! { <WelcomeStep on_next=move || set_current_step.set(OnboardingStep::SelectMethod) /> }
                        },
                        OnboardingStep::SelectMethod => {
                            view! { 
                                <SelectMethodStep 
                                    on_auto_discovery=move || set_current_step.set(OnboardingStep::AutoDiscovery)
                                    on_manual_entry=move || set_current_step.set(OnboardingStep::ManualEntry)
                                    on_qr_generation=generate_invitation
                                    on_platform_selection=move || set_current_step.set(OnboardingStep::PlatformSelection)
                                />
                            }
                        },
                        OnboardingStep::AutoDiscovery => {
                            view! { 
                                <AutoDiscoveryStep 
                                    discovery_status=discovery_status
                                    discovered_devices=discovered_devices
                                    scan_progress=scan_progress
                                    on_start_scan=start_network_scan
                                    on_verify_device=verify_device
                                    on_back=move || set_current_step.set(OnboardingStep::SelectMethod)
                                />
                            }
                        },
                        OnboardingStep::ManualEntry => {
                            view! { 
                                <ManualEntryStep 
                                    device_id=manual_device_id
                                    device_code=manual_device_code
                                    set_device_id=set_manual_device_id
                                    set_device_code=set_manual_device_code
                                    on_verify=verify_device
                                    on_back=move || set_current_step.set(OnboardingStep::SelectMethod)
                                />
                            }
                        },
                        OnboardingStep::QrCodeGeneration => {
                            view! { 
                                <QrCodeStep 
                                    invitation=invitation
                                    on_back=move || set_current_step.set(OnboardingStep::SelectMethod)
                                    on_next=move || set_current_step.set(OnboardingStep::DeviceVerification)
                                />
                            }
                        },
                        OnboardingStep::PlatformSelection => {
                            view! { 
                                <PlatformSelectionStep 
                                    selected_platform=selected_platform
                                    set_selected_platform=set_selected_platform
                                    on_next=move || set_current_step.set(OnboardingStep::InstallationGuide)
                                    on_back=move || set_current_step.set(OnboardingStep::SelectMethod)
                                />
                            }
                        },
                        OnboardingStep::InstallationGuide => {
                            view! { 
                                <InstallationGuideStep 
                                    platform=selected_platform.get()
                                    on_next=move || set_current_step.set(OnboardingStep::ConfigurationFile)
                                    on_back=move || set_current_step.set(OnboardingStep::PlatformSelection)
                                />
                            }
                        },
                        OnboardingStep::ConfigurationFile => {
                            view! { 
                                <ConfigurationStep 
                                    platform=selected_platform.get()
                                    on_next=move || set_current_step.set(OnboardingStep::DeviceVerification)
                                    on_back=move || set_current_step.set(OnboardingStep::InstallationGuide)
                                />
                            }
                        },
                        OnboardingStep::DeviceVerification => {
                            view! { 
                                <DeviceVerificationStep 
                                    on_complete=move || set_current_step.set(OnboardingStep::Complete)
                                    on_back=move || set_current_step.set(OnboardingStep::ConfigurationFile)
                                />
                            }
                        },
                        OnboardingStep::Complete => {
                            view! { 
                                <CompleteStep 
                                    on_add_another=move || set_current_step.set(OnboardingStep::SelectMethod)
                                />
                            }
                        },
                    }
                }}
            </div>
        </div>
    }
}

#[component]
fn WelcomeStep<F>(on_next: F) -> impl IntoView 
where 
    F: Fn() + 'static,
{
    view! {
        <div class="row justify-content-center">
            <div class="col-lg-8">
                <div class="card border-0 shadow-sm text-center">
                    <div class="card-body py-5">
                        <div class="mb-4">
                            <i class="bi bi-shield-check text-primary" style="font-size: 4rem;"></i>
                        </div>
                        <h3 class="h4 fw-bold mb-3">"Welcome to GhostLink Device Setup"</h3>
                        <p class="text-muted mb-4 fs-6">
                            "Set up secure remote access to your devices in just a few steps. "
                            "Choose from automatic discovery, manual setup, or mobile device pairing."
                        </p>
                        
                        <div class="row g-3 mb-4">
                            <div class="col-md-4">
                                <div class="bg-light rounded p-3">
                                    <i class="bi bi-radar text-primary fs-4 d-block mb-2"></i>
                                    <h6 class="fw-semibold">"Auto Discovery"</h6>
                                    <small class="text-muted">"Find devices on your network"</small>
                                </div>
                            </div>
                            <div class="col-md-4">
                                <div class="bg-light rounded p-3">
                                    <i class="bi bi-qr-code text-success fs-4 d-block mb-2"></i>
                                    <h6 class="fw-semibold">"QR Code Setup"</h6>
                                    <small class="text-muted">"Easy mobile device pairing"</small>
                                </div>
                            </div>
                            <div class="col-md-4">
                                <div class="bg-light rounded p-3">
                                    <i class="bi bi-keyboard text-warning fs-4 d-block mb-2"></i>
                                    <h6 class="fw-semibold">"Manual Entry"</h6>
                                    <small class="text-muted">"Add devices by ID or code"</small>
                                </div>
                            </div>
                        </div>

                        <button class="btn btn-primary btn-lg px-5" on:click=move |_| on_next()>
                            <i class="bi bi-arrow-right me-2"></i>
                            "Get Started"
                        </button>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn SelectMethodStep<F1, F2, F3, F4>(
    on_auto_discovery: F1,
    on_manual_entry: F2,
    on_qr_generation: F3,
    on_platform_selection: F4,
) -> impl IntoView 
where 
    F1: Fn() + 'static,
    F2: Fn() + 'static,
    F3: Fn() + 'static,
    F4: Fn() + 'static,
{
    view! {
        <div class="row">
            <div class="col-12">
                <h3 class="h5 fw-bold mb-4">"Choose Setup Method"</h3>
                
                <div class="row g-4">
                    // Auto Discovery
                    <div class="col-lg-6">
                        <div class="card h-100 border-2 border-primary">
                            <div class="card-body text-center p-4">
                                <div class="text-primary mb-3">
                                    <i class="bi bi-radar" style="font-size: 3rem;"></i>
                                </div>
                                <h5 class="fw-bold mb-3">"Auto Discovery"</h5>
                                <p class="text-muted mb-4">
                                    "Automatically scan your network to find devices with GhostLink agent already installed. "
                                    "This is the fastest way to add multiple devices."
                                </p>
                                <ul class="list-unstyled text-start mb-4">
                                    <li class="mb-2">
                                        <i class="bi bi-check-circle text-success me-2"></i>
                                        "Finds all network devices"
                                    </li>
                                    <li class="mb-2">
                                        <i class="bi bi-check-circle text-success me-2"></i>
                                        "No manual configuration"
                                    </li>
                                    <li class="mb-2">
                                        <i class="bi bi-check-circle text-success me-2"></i>
                                        "Real-time discovery"
                                    </li>
                                </ul>
                                <button class="btn btn-primary w-100" on:click=move |_| on_auto_discovery()>
                                    <i class="bi bi-search me-2"></i>
                                    "Start Network Scan"
                                </button>
                            </div>
                        </div>
                    </div>
                    
                    // Manual Entry
                    <div class="col-lg-6">
                        <div class="card h-100">
                            <div class="card-body text-center p-4">
                                <div class="text-warning mb-3">
                                    <i class="bi bi-keyboard" style="font-size: 3rem;"></i>
                                </div>
                                <h5 class="fw-bold mb-3">"Manual Entry"</h5>
                                <p class="text-muted mb-4">
                                    "Add a device using its unique ID or pairing code. "
                                    "Perfect for devices on different networks or with specific security requirements."
                                </p>
                                <ul class="list-unstyled text-start mb-4">
                                    <li class="mb-2">
                                        <i class="bi bi-check-circle text-success me-2"></i>
                                        "Works across networks"
                                    </li>
                                    <li class="mb-2">
                                        <i class="bi bi-check-circle text-success me-2"></i>
                                        "Secure pairing codes"
                                    </li>
                                    <li class="mb-2">
                                        <i class="bi bi-check-circle text-success me-2"></i>
                                        "Precise control"
                                    </li>
                                </ul>
                                <button class="btn btn-outline-warning w-100" on:click=move |_| on_manual_entry()>
                                    <i class="bi bi-plus-circle me-2"></i>
                                    "Add by ID/Code"
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
                
                <div class="row g-4 mt-2">
                    // QR Code Generation
                    <div class="col-lg-6">
                        <div class="card h-100">
                            <div class="card-body text-center p-4">
                                <div class="text-success mb-3">
                                    <i class="bi bi-qr-code" style="font-size: 3rem;"></i>
                                </div>
                                <h5 class="fw-bold mb-3">"QR Code Pairing"</h5>
                                <p class="text-muted mb-4">
                                    "Generate a QR code for easy mobile device setup. "
                                    "Users can scan with their phone to quickly join your network."
                                </p>
                                <ul class="list-unstyled text-start mb-4">
                                    <li class="mb-2">
                                        <i class="bi bi-check-circle text-success me-2"></i>
                                        "Mobile-friendly setup"
                                    </li>
                                    <li class="mb-2">
                                        <i class="bi bi-check-circle text-success me-2"></i>
                                        "Time-limited invitations"
                                    </li>
                                    <li class="mb-2">
                                        <i class="bi bi-check-circle text-success me-2"></i>
                                        "Organization-specific"
                                    </li>
                                </ul>
                                <button class="btn btn-outline-success w-100" on:click=move |_| on_qr_generation()>
                                    <i class="bi bi-qr-code-scan me-2"></i>
                                    "Generate QR Code"
                                </button>
                            </div>
                        </div>
                    </div>
                    
                    // Installation Guide
                    <div class="col-lg-6">
                        <div class="card h-100">
                            <div class="card-body text-center p-4">
                                <div class="text-info mb-3">
                                    <i class="bi bi-download" style="font-size: 3rem;"></i>
                                </div>
                                <h5 class="fw-bold mb-3">"Installation Guide"</h5>
                                <p class="text-muted mb-4">
                                    "Need to install the GhostLink agent on a new device? "
                                    "Get platform-specific installation instructions and download links."
                                </p>
                                <ul class="list-unstyled text-start mb-4">
                                    <li class="mb-2">
                                        <i class="bi bi-check-circle text-success me-2"></i>
                                        "All platforms supported"
                                    </li>
                                    <li class="mb-2">
                                        <i class="bi bi-check-circle text-success me-2"></i>
                                        "Step-by-step guide"
                                    </li>
                                    <li class="mb-2">
                                        <i class="bi bi-check-circle text-success me-2"></i>
                                        "Auto-generated config"
                                    </li>
                                </ul>
                                <button class="btn btn-outline-info w-100" on:click=move |_| on_platform_selection()>
                                    <i class="bi bi-book me-2"></i>
                                    "View Install Guide"
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn AutoDiscoveryStep<F1, F2, F3>(
    discovery_status: ReadSignal<DiscoveryStatus>,
    discovered_devices: ReadSignal<Vec<DiscoveredDevice>>,
    scan_progress: ReadSignal<i32>,
    on_start_scan: F1,
    on_verify_device: F2,
    on_back: F3,
) -> impl IntoView 
where 
    F1: Fn() + 'static,
    F2: Fn(String) + 'static,
    F3: Fn() + 'static,
{
    view! {
        <div class="row">
            <div class="col-12">
                <div class="d-flex justify-content-between align-items-center mb-4">
                    <h3 class="h5 fw-bold mb-0">"Network Discovery"</h3>
                    <button class="btn btn-outline-secondary" on:click=move |_| on_back()>
                        <i class="bi bi-arrow-left me-2"></i>
                        "Back"
                    </button>
                </div>
                
                // Discovery status card
                <div class="card border-0 shadow-sm mb-4">
                    <div class="card-body">
                        <div class="row align-items-center">
                            <div class="col-md-8">
                                <h6 class="fw-semibold mb-2">"Network Scan Status"</h6>
                                {move || {
                                    match discovery_status.get() {
                                        DiscoveryStatus::Idle => view! {
                                            <p class="text-muted mb-0">"Ready to start network discovery"</p>
                                        }.into_view(),
                                        DiscoveryStatus::Scanning => view! {
                                            <div>
                                                <p class="text-info mb-2">
                                                    <i class="bi bi-arrow-clockwise me-2"></i>
                                                    "Scanning network for GhostLink devices..."
                                                </p>
                                                <div class="progress mb-0" style="height: 8px;">
                                                    <div 
                                                        class="progress-bar bg-info progress-bar-striped progress-bar-animated"
                                                        style=move || format!("width: {}%", scan_progress.get())
                                                    ></div>
                                                </div>
                                            </div>
                                        }.into_view(),
                                        DiscoveryStatus::Found => view! {
                                            <p class="text-success mb-0">
                                                <i class="bi bi-check-circle me-2"></i>
                                                {format!("Found {} devices", discovered_devices.get().len())}
                                            </p>
                                        }.into_view(),
                                        DiscoveryStatus::Error => view! {
                                            <p class="text-danger mb-0">
                                                <i class="bi bi-exclamation-triangle me-2"></i>
                                                "Network scan failed"
                                            </p>
                                        }.into_view(),
                                    }
                                }}
                            </div>
                            <div class="col-md-4 text-end">
                                <button 
                                    class="btn btn-primary"
                                    disabled=move || matches!(discovery_status.get(), DiscoveryStatus::Scanning)
                                    on:click=move |_| on_start_scan()
                                >
                                    {move || {
                                        if matches!(discovery_status.get(), DiscoveryStatus::Scanning) {
                                            view! {
                                                <span>
                                                    <span class="spinner-border spinner-border-sm me-2"></span>
                                                    "Scanning..."
                                                </span>
                                            }.into_view()
                                        } else {
                                            view! {
                                                <span>
                                                    <i class="bi bi-search me-2"></i>
                                                    "Start Scan"
                                                </span>
                                            }.into_view()
                                        }
                                    }}
                                </button>
                            </div>
                        </div>
                    </div>
                </div>

                // Discovered devices
                {move || {
                    let devices = discovered_devices.get();
                    if devices.is_empty() && discovery_status.get() == DiscoveryStatus::Found {
                        view! {
                            <div class="card border-0">
                                <div class="card-body text-center py-5">
                                    <i class="bi bi-wifi-off text-muted mb-3" style="font-size: 3rem;"></i>
                                    <h6 class="fw-semibold">"No devices found"</h6>
                                    <p class="text-muted mb-3">
                                        "No GhostLink agents were found on your network. "
                                        "Make sure devices are connected and the agent is installed."
                                    </p>
                                    <button class="btn btn-outline-primary me-2" on:click=move |_| on_start_scan()>
                                        <i class="bi bi-arrow-clockwise me-2"></i>
                                        "Scan Again"
                                    </button>
                                    <button class="btn btn-outline-secondary" on:click=move |_| on_back()>
                                        "Try Another Method"
                                    </button>
                                </div>
                            </div>
                        }.into_view()
                    } else if !devices.is_empty() {
                        view! {
                            <div>
                                <h6 class="fw-semibold mb-3">"Discovered Devices"</h6>
                                <div class="row g-3">
                                    <For
                                        each=move || discovered_devices.get()
                                        key=|device| device.ip_address.clone()
                                        children=move |device| {
                                            let device_id = device.hostname.clone();
                                            let verify_fn = on_verify_device.clone();
                                            
                                            view! {
                                                <div class="col-lg-6">
                                                    <div class="card border-0 shadow-sm">
                                                        <div class="card-body">
                                                            <div class="d-flex justify-content-between align-items-start mb-3">
                                                                <div class="d-flex align-items-center">
                                                                    <div class="text-primary me-3">
                                                                        <i class={format!("bi {} fs-4", get_platform_icon(&device.platform))}></i>
                                                                    </div>
                                                                    <div>
                                                                        <h6 class="fw-bold mb-1">{device.hostname.clone()}</h6>
                                                                        <small class="text-muted">{device.ip_address.clone()}</small>
                                                                    </div>
                                                                </div>
                                                                <span class={format!("badge {}", if device.is_verified { "bg-success" } else { "bg-warning" })}>
                                                                    {if device.is_verified { "Verified" } else { "Unverified" }}
                                                                </span>
                                                            </div>
                                                            
                                                            <div class="row text-sm text-muted mb-3">
                                                                <div class="col-6">
                                                                    <div class="mb-2">
                                                                        <i class="bi bi-laptop me-2"></i>
                                                                        {device.platform.clone()}
                                                                    </div>
                                                                    <div>
                                                                        <i class="bi bi-hdd me-2"></i>
                                                                        {device.device_type.clone()}
                                                                    </div>
                                                                </div>
                                                                <div class="col-6">
                                                                    {device.agent_version.as_ref().map(|v| view! {
                                                                        <div class="mb-2">
                                                                            <i class="bi bi-info-circle me-2"></i>
                                                                            "v"{v.clone()}
                                                                        </div>
                                                                    })}
                                                                    {device.signal_strength.map(|s| view! {
                                                                        <div>
                                                                            <i class="bi bi-wifi me-2"></i>
                                                                            {format!("{:.0}%", s * 100.0)}
                                                                        </div>
                                                                    })}
                                                                </div>
                                                            </div>
                                                            
                                                            <div class="d-grid gap-2">
                                                                <button 
                                                                    class="btn btn-primary btn-sm"
                                                                    disabled=device.is_verified
                                                                    on:click=move |_| verify_fn(device_id.clone())
                                                                >
                                                                    {if device.is_verified {
                                                                        view! {
                                                                            <span>
                                                                                <i class="bi bi-check-circle me-1"></i>
                                                                                "Already Added"
                                                                            </span>
                                                                        }.into_view()
                                                                    } else {
                                                                        view! {
                                                                            <span>
                                                                                <i class="bi bi-plus-circle me-1"></i>
                                                                                "Add Device"
                                                                            </span>
                                                                        }.into_view()
                                                                    }}
                                                                </button>
                                                                <button class="btn btn-outline-secondary btn-sm">
                                                                    <i class="bi bi-info-circle me-1"></i>
                                                                    "View Details"
                                                                </button>
                                                            </div>
                                                        </div>
                                                    </div>
                                                </div>
                                            }
                                        }
                                    />
                                </div>
                            </div>
                        }.into_view()
                    } else {
                        view! {}.into_view()
                    }
                }}
            </div>
        </div>
    }
}

#[component]
fn ManualEntryStep<F1, F2>(
    device_id: ReadSignal<String>,
    device_code: ReadSignal<String>,
    set_device_id: WriteSignal<String>,
    set_device_code: WriteSignal<String>,
    on_verify: F1,
    on_back: F2,
) -> impl IntoView 
where 
    F1: Fn(String) + 'static,
    F2: Fn() + 'static,
{
    let (verification_method, set_verification_method) = create_signal("id".to_string());

    view! {
        <div class="row justify-content-center">
            <div class="col-lg-8">
                <div class="d-flex justify-content-between align-items-center mb-4">
                    <h3 class="h5 fw-bold mb-0">"Manual Device Entry"</h3>
                    <button class="btn btn-outline-secondary" on:click=move |_| on_back()>
                        <i class="bi bi-arrow-left me-2"></i>
                        "Back"
                    </button>
                </div>
                
                <div class="card border-0 shadow-sm">
                    <div class="card-body p-4">
                        <div class="text-center mb-4">
                            <i class="bi bi-keyboard text-warning" style="font-size: 3rem;"></i>
                            <h5 class="fw-bold mt-3">"Add Device Manually"</h5>
                            <p class="text-muted">
                                "Enter the device ID or pairing code to add a device to your network"
                            </p>
                        </div>
                        
                        // Method selection
                        <div class="mb-4">
                            <label class="form-label fw-semibold">"Verification Method"</label>
                            <div class="btn-group w-100" role="group">
                                <input 
                                    type="radio" 
                                    class="btn-check" 
                                    name="method" 
                                    id="method-id"
                                    checked=move || verification_method.get() == "id"
                                    on:change=move |_| set_verification_method.set("id".to_string())
                                />
                                <label class="btn btn-outline-primary" for="method-id">
                                    <i class="bi bi-key me-2"></i>
                                    "Device ID"
                                </label>
                                
                                <input 
                                    type="radio" 
                                    class="btn-check" 
                                    name="method" 
                                    id="method-code"
                                    checked=move || verification_method.get() == "code"
                                    on:change=move |_| set_verification_method.set("code".to_string())
                                />
                                <label class="btn btn-outline-primary" for="method-code">
                                    <i class="bi bi-qr-code me-2"></i>
                                    "Pairing Code"
                                </label>
                            </div>
                        </div>
                        
                        {move || {
                            if verification_method.get() == "id" {
                                view! {
                                    <div class="mb-4">
                                        <label for="device-id" class="form-label fw-semibold">"Device ID"</label>
                                        <input 
                                            type="text"
                                            class="form-control form-control-lg"
                                            id="device-id"
                                            placeholder="Enter device ID (e.g., GHOST-A1B2C3D4E5F6)"
                                            prop:value=move || device_id.get()
                                            on:input=move |ev| set_device_id.set(event_target_value(&ev))
                                        />
                                        <div class="form-text">
                                            "Device IDs are typically found in the GhostLink agent interface"
                                        </div>
                                    </div>
                                }.into_view()
                            } else {
                                view! {
                                    <div class="mb-4">
                                        <label for="device-code" class="form-label fw-semibold">"Pairing Code"</label>
                                        <input 
                                            type="text"
                                            class="form-control form-control-lg"
                                            id="device-code"
                                            placeholder="Enter 6-digit pairing code (e.g., 123456)"
                                            prop:value=move || device_code.get()
                                            on:input=move |ev| set_device_code.set(event_target_value(&ev))
                                            maxlength="6"
                                        />
                                        <div class="form-text">
                                            "Pairing codes are temporary and expire after 15 minutes"
                                        </div>
                                    </div>
                                }.into_view()
                            }
                        }}
                        
                        <div class="d-grid gap-2">
                            <button 
                                class="btn btn-primary btn-lg"
                                disabled=move || {
                                    if verification_method.get() == "id" {
                                        device_id.get().trim().is_empty()
                                    } else {
                                        device_code.get().trim().len() != 6
                                    }
                                }
                                on:click=move |_| {
                                    let id_or_code = if verification_method.get() == "id" {
                                        device_id.get()
                                    } else {
                                        device_code.get()
                                    };
                                    on_verify(id_or_code);
                                }
                            >
                                <i class="bi bi-shield-check me-2"></i>
                                "Verify and Add Device"
                            </button>
                            
                            <div class="text-center mt-3">
                                <small class="text-muted">
                                    "Need help finding your device ID? "
                                    <a href="#" class="text-primary">"View instructions"</a>
                                </small>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn QrCodeStep<F1, F2>(
    invitation: ReadSignal<Option<DeviceInvitation>>,
    on_back: F1,
    on_next: F2,
) -> impl IntoView 
where 
    F1: Fn() + 'static,
    F2: Fn() + 'static,
{
    view! {
        <div class="row justify-content-center">
            <div class="col-lg-8">
                <div class="d-flex justify-content-between align-items-center mb-4">
                    <h3 class="h5 fw-bold mb-0">"QR Code Device Pairing"</h3>
                    <button class="btn btn-outline-secondary" on:click=move |_| on_back()>
                        <i class="bi bi-arrow-left me-2"></i>
                        "Back"
                    </button>
                </div>
                
                {move || {
                    if let Some(inv) = invitation.get() {
                        view! {
                            <div class="card border-0 shadow-sm">
                                <div class="card-body text-center p-5">
                                    <div class="mb-4">
                                        <i class="bi bi-qr-code text-success" style="font-size: 3rem;"></i>
                                        <h5 class="fw-bold mt-3">"Scan to Connect"</h5>
                                        <p class="text-muted">
                                            "Have the device scan this QR code with the GhostLink mobile app"
                                        </p>
                                    </div>
                                    
                                    // QR Code display
                                    <div class="mb-4">
                                        <div class="bg-white border rounded p-4 d-inline-block">
                                            <img 
                                                src=inv.qr_code.clone()
                                                alt="Device pairing QR code"
                                                class="img-fluid"
                                                style="max-width: 200px; height: auto;"
                                            />
                                        </div>
                                    </div>
                                    
                                    // Invitation details
                                    <div class="row g-3 mb-4">
                                        <div class="col-md-6">
                                            <div class="bg-light rounded p-3">
                                                <small class="text-muted d-block">"Pairing Code"</small>
                                                <strong class="h5">{inv.code.clone()}</strong>
                                            </div>
                                        </div>
                                        <div class="col-md-6">
                                            <div class="bg-light rounded p-3">
                                                <small class="text-muted d-block">"Expires At"</small>
                                                <strong>{format_timestamp(&inv.expires_at)}</strong>
                                            </div>
                                        </div>
                                    </div>
                                    
                                    // Instructions
                                    <div class="text-start mb-4">
                                        <h6 class="fw-semibold">"Setup Instructions:"</h6>
                                        <ol class="list-unstyled">
                                            <li class="mb-2">
                                                <span class="badge bg-primary rounded-circle me-3">"1"</span>
                                                "Install the GhostLink app on your mobile device"
                                            </li>
                                            <li class="mb-2">
                                                <span class="badge bg-primary rounded-circle me-3">"2"</span>
                                                "Open the app and tap \"Join Organization\""
                                            </li>
                                            <li class="mb-2">
                                                <span class="badge bg-primary rounded-circle me-3">"3"</span>
                                                "Scan this QR code or enter the pairing code manually"
                                            </li>
                                            <li class="mb-2">
                                                <span class="badge bg-primary rounded-circle me-3">"4"</span>
                                                "Complete the verification process on your device"
                                            </li>
                                        </ol>
                                    </div>
                                    
                                    <div class="d-flex gap-2 justify-content-center">
                                        <button class="btn btn-success" on:click=move |_| on_next()>
                                            <i class="bi bi-check-circle me-2"></i>
                                            "Device Connected"
                                        </button>
                                        <button class="btn btn-outline-primary">
                                            <i class="bi bi-download me-2"></i>
                                            "Download QR Code"
                                        </button>
                                    </div>
                                </div>
                            </div>
                        }.into_view()
                    } else {
                        view! {
                            <div class="card border-0">
                                <div class="card-body text-center py-5">
                                    <div class="spinner-border text-primary mb-3"></div>
                                    <p class="text-muted">"Generating QR code..."</p>
                                </div>
                            </div>
                        }.into_view()
                    }
                }}
            </div>
        </div>
    }
}

#[component]
fn PlatformSelectionStep<F1, F2>(
    selected_platform: ReadSignal<String>,
    set_selected_platform: WriteSignal<String>,
    on_next: F1,
    on_back: F2,
) -> impl IntoView 
where 
    F1: Fn() + 'static,
    F2: Fn() + 'static,
{
    let platforms = vec![
        ("windows", "Windows", "bi-windows", "Download for Windows 10/11"),
        ("macos", "macOS", "bi-apple", "Download for macOS 10.15+"),
        ("linux", "Linux", "bi-ubuntu", "Download for Ubuntu/Debian/CentOS"),
        ("android", "Android", "bi-android2", "Download from Google Play Store"),
        ("ios", "iOS", "bi-apple", "Download from App Store"),
    ];

    view! {
        <div class="row justify-content-center">
            <div class="col-lg-10">
                <div class="d-flex justify-content-between align-items-center mb-4">
                    <h3 class="h5 fw-bold mb-0">"Select Platform"</h3>
                    <button class="btn btn-outline-secondary" on:click=move |_| on_back()>
                        <i class="bi bi-arrow-left me-2"></i>
                        "Back"
                    </button>
                </div>
                
                <div class="card border-0 shadow-sm mb-4">
                    <div class="card-body">
                        <div class="text-center mb-4">
                            <i class="bi bi-download text-info" style="font-size: 3rem;"></i>
                            <h5 class="fw-bold mt-3">"Choose Your Platform"</h5>
                            <p class="text-muted">
                                "Select the operating system for the device you want to set up"
                            </p>
                        </div>
                        
                        <div class="row g-3">
                            <For
                                each=move || platforms.clone()
                                key=|(id, _, _, _)| id.to_string()
                                children=move |(platform_id, platform_name, icon, description)| {
                                    let is_selected = move || selected_platform.get() == platform_id;
                                    let platform_id = platform_id.to_string();
                                    
                                    view! {
                                        <div class="col-lg-4 col-md-6">
                                            <div 
                                                class=move || format!("card h-100 cursor-pointer {}", 
                                                    if is_selected() { "border-primary border-2" } else { "border-1" })
                                                on:click=move |_| set_selected_platform.set(platform_id.clone())
                                            >
                                                <div class="card-body text-center p-4">
                                                    <div class=move || format!("mb-3 {}", 
                                                        if is_selected() { "text-primary" } else { "text-muted" })
                                                    >
                                                        <i class={format!("bi {} fs-1", icon)}></i>
                                                    </div>
                                                    <h6 class="fw-bold mb-2">{platform_name}</h6>
                                                    <p class="text-muted small mb-0">{description}</p>
                                                    {move || if is_selected() {
                                                        view! {
                                                            <div class="mt-3">
                                                                <span class="badge bg-primary">
                                                                    <i class="bi bi-check-circle me-1"></i>
                                                                    "Selected"
                                                                </span>
                                                            </div>
                                                        }.into_view()
                                                    } else {
                                                        view! {}.into_view()
                                                    }}
                                                </div>
                                            </div>
                                        </div>
                                    }
                                }
                            />
                        </div>
                        
                        <div class="text-center mt-4">
                            <button 
                                class="btn btn-primary btn-lg px-5"
                                disabled=selected_platform.get().is_empty()
                                on:click=move |_| on_next()
                            >
                                <i class="bi bi-arrow-right me-2"></i>
                                "Continue to Installation"
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn InstallationGuideStep<F1, F2>(
    platform: String,
    on_next: F1,
    on_back: F2,
) -> impl IntoView 
where 
    F1: Fn() + 'static,
    F2: Fn() + 'static,
{
    let (instructions, set_instructions) = create_signal(None::<InstallationInstructions>);
    
    // Load installation instructions
    create_effect(move |_| {
        let platform = platform.clone();
        spawn_local(async move {
            match get_installation_instructions(&platform).await {
                Ok(inst) => set_instructions.set(Some(inst)),
                Err(_) => {
                    // Fallback instructions
                    let fallback = InstallationInstructions {
                        platform: platform.clone(),
                        steps: get_default_steps(&platform),
                        download_url: get_download_url(&platform),
                        config_template: "".to_string(),
                    };
                    set_instructions.set(Some(fallback));
                }
            }
        });
    });

    view! {
        <div class="row justify-content-center">
            <div class="col-lg-10">
                <div class="d-flex justify-content-between align-items-center mb-4">
                    <h3 class="h5 fw-bold mb-0">"Installation Guide"</h3>
                    <button class="btn btn-outline-secondary" on:click=move |_| on_back()>
                        <i class="bi bi-arrow-left me-2"></i>
                        "Back"
                    </button>
                </div>
                
                {move || {
                    if let Some(inst) = instructions.get() {
                        let platform_icon = get_platform_icon(&inst.platform);
                        
                        view! {
                            <div class="card border-0 shadow-sm">
                                <div class="card-body p-4">
                                    <div class="d-flex align-items-center mb-4">
                                        <div class="text-primary me-3">
                                            <i class={format!("bi {} fs-2", platform_icon)}></i>
                                        </div>
                                        <div>
                                            <h5 class="fw-bold mb-1">
                                                {format!("GhostLink Agent for {}", inst.platform.to_uppercase())}
                                            </h5>
                                            <p class="text-muted mb-0">"Follow these steps to install the agent"</p>
                                        </div>
                                    </div>
                                    
                                    // Download section
                                    <div class="bg-light rounded p-4 mb-4">
                                        <div class="d-flex justify-content-between align-items-center">
                                            <div>
                                                <h6 class="fw-semibold mb-1">"Download GhostLink Agent"</h6>
                                                <small class="text-muted">"Latest version with security updates"</small>
                                            </div>
                                            <a 
                                                href={inst.download_url.clone()}
                                                class="btn btn-primary"
                                                download
                                            >
                                                <i class="bi bi-download me-2"></i>
                                                "Download"
                                            </a>
                                        </div>
                                    </div>
                                    
                                    // Installation steps
                                    <div class="mb-4">
                                        <h6 class="fw-semibold mb-3">"Installation Steps"</h6>
                                        <div class="list-group list-group-flush">
                                            <For
                                                each=move || inst.steps.iter().enumerate().collect::<Vec<_>>()
                                                key=|(i, _)| *i
                                                children=move |(index, step)| {
                                                    view! {
                                                        <div class="list-group-item border-0 px-0">
                                                            <div class="d-flex align-items-start">
                                                                <span class="badge bg-primary rounded-circle me-3 mt-1">
                                                                    {index + 1}
                                                                </span>
                                                                <div class="flex-grow-1">
                                                                    <p class="mb-0">{step.clone()}</p>
                                                                </div>
                                                            </div>
                                                        </div>
                                                    }
                                                }
                                            />
                                        </div>
                                    </div>
                                    
                                    // Security notice
                                    <div class="alert alert-info d-flex align-items-start">
                                        <i class="bi bi-shield-check me-3 mt-1"></i>
                                        <div>
                                            <strong>"Security Notice:"</strong>
                                            " The GhostLink agent uses end-to-end encryption and requires explicit "
                                            "authorization before allowing remote access. Your data remains secure."
                                        </div>
                                    </div>
                                    
                                    <div class="text-center">
                                        <button class="btn btn-primary btn-lg px-5" on:click=move |_| on_next()>
                                            <i class="bi bi-arrow-right me-2"></i>
                                            "Continue to Configuration"
                                        </button>
                                    </div>
                                </div>
                            </div>
                        }.into_view()
                    } else {
                        view! {
                            <div class="card border-0">
                                <div class="card-body text-center py-5">
                                    <div class="spinner-border text-primary mb-3"></div>
                                    <p class="text-muted">"Loading installation instructions..."</p>
                                </div>
                            </div>
                        }.into_view()
                    }
                }}
            </div>
        </div>
    }
}

#[component]
fn ConfigurationStep<F1, F2>(
    platform: String,
    on_next: F1,
    on_back: F2,
) -> impl IntoView 
where 
    F1: Fn() + 'static,
    F2: Fn() + 'static,
{
    let (config_content, set_config_content) = create_signal(String::new());
    
    // Generate configuration
    create_effect(move |_| {
        spawn_local(async move {
            match generate_config_file(&platform).await {
                Ok(config) => set_config_content.set(config),
                Err(_) => {
                    // Fallback configuration
                    let config = format!(
                        r#"{{
    "server_url": "https://your-ghostlink-server.com",
    "device_name": "My-{}-Device",
    "organization_id": "your-org-id",
    "auto_connect": true,
    "security": {{
        "require_approval": true,
        "allow_file_transfer": true,
        "allow_terminal": false
    }}
}}"#, platform.to_uppercase()
                    );
                    set_config_content.set(config);
                }
            }
        });
    });

    view! {
        <div class="row justify-content-center">
            <div class="col-lg-10">
                <div class="d-flex justify-content-between align-items-center mb-4">
                    <h3 class="h5 fw-bold mb-0">"Configuration File"</h3>
                    <button class="btn btn-outline-secondary" on:click=move |_| on_back()>
                        <i class="bi bi-arrow-left me-2"></i>
                        "Back"
                    </button>
                </div>
                
                <div class="card border-0 shadow-sm">
                    <div class="card-body p-4">
                        <div class="text-center mb-4">
                            <i class="bi bi-file-earmark-code text-info" style="font-size: 3rem;"></i>
                            <h5 class="fw-bold mt-3">"Agent Configuration"</h5>
                            <p class="text-muted">
                                "Copy this configuration file to complete the setup"
                            </p>
                        </div>
                        
                        // Configuration file display
                        <div class="mb-4">
                            <div class="d-flex justify-content-between align-items-center mb-2">
                                <label class="form-label fw-semibold">"Configuration File (config.json)"</label>
                                <button 
                                    class="btn btn-outline-secondary btn-sm"
                                    on:click=move |_| {
                                        // Copy to clipboard
                                        if let Some(window) = web_sys::window() {
                                            if let Some(navigator) = window.navigator() {
                                                if let Some(clipboard) = navigator.clipboard() {
                                                    let _ = clipboard.write_text(&config_content.get());
                                                }
                                            }
                                        }
                                    }
                                >
                                    <i class="bi bi-clipboard me-1"></i>
                                    "Copy"
                                </button>
                            </div>
                            <textarea 
                                class="form-control"
                                rows="15"
                                readonly
                                style="font-family: 'Monaco', 'Consolas', monospace; font-size: 0.9em;"
                            >
                                {config_content.get()}
                            </textarea>
                        </div>
                        
                        // Platform-specific instructions
                        <div class="alert alert-info">
                            <h6 class="alert-heading">
                                <i class="bi bi-info-circle me-2"></i>
                                "Configuration Instructions"
                            </h6>
                            {match platform.as_str() {
                                "windows" => view! {
                                    <p class="mb-0">
                                        "Save this file as " <code>"config.json"</code> " in the GhostLink installation directory "
                                        <code>"C:\\Program Files\\GhostLink\\config.json"</code>
                                    </p>
                                }.into_view(),
                                "macos" => view! {
                                    <p class="mb-0">
                                        "Save this file as " <code>"config.json"</code> " in "
                                        <code>"/Applications/GhostLink.app/Contents/Resources/config.json"</code>
                                    </p>
                                }.into_view(),
                                "linux" => view! {
                                    <p class="mb-0">
                                        "Save this file as " <code>"config.json"</code> " in "
                                        <code>"/etc/ghostlink/config.json"</code> " or "
                                        <code>"~/.config/ghostlink/config.json"</code>
                                    </p>
                                }.into_view(),
                                _ => view! {
                                    <p class="mb-0">
                                        "Save this configuration in the appropriate location for your platform"
                                    </p>
                                }.into_view(),
                            }}
                        </div>
                        
                        <div class="d-grid gap-2">
                            <button 
                                class="btn btn-success btn-lg"
                                on:click=move |_| {
                                    // Download config file
                                    let content = config_content.get();
                                    let blob = web_sys::Blob::new_with_str_sequence_and_options(
                                        &js_sys::Array::of1(&JsValue::from_str(&content)),
                                        web_sys::BlobPropertyBag::new().type_("application/json")
                                    ).unwrap();
                                    
                                    if let Some(window) = web_sys::window() {
                                        let url = web_sys::Url::create_object_url_with_blob(&blob).unwrap();
                                        let document = window.document().unwrap();
                                        let anchor = document.create_element("a").unwrap();
                                        anchor.set_attribute("href", &url).unwrap();
                                        anchor.set_attribute("download", "config.json").unwrap();
                                        anchor.dyn_ref::<web_sys::HtmlElement>().unwrap().click();
                                        web_sys::Url::revoke_object_url(&url).unwrap();
                                    }
                                }
                            >
                                <i class="bi bi-download me-2"></i>
                                "Download Configuration File"
                            </button>
                            
                            <button class="btn btn-primary btn-lg" on:click=move |_| on_next()>
                                <i class="bi bi-arrow-right me-2"></i>
                                "Configuration Complete"
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn DeviceVerificationStep<F1, F2>(
    on_complete: F1,
    on_back: F2,
) -> impl IntoView 
where 
    F1: Fn() + 'static,
    F2: Fn() + 'static,
{
    let (verification_status, set_verification_status) = create_signal("waiting".to_string());
    let (device_info, set_device_info) = create_signal(None::<Device>);

    // Simulate device verification process
    create_effect(move |_| {
        set_timeout(
            move || {
                set_verification_status.set("connected".to_string());
                // Simulate device info
                let mock_device = Device {
                    id: "GHOST-DEV-123".to_string(),
                    name: "New Device".to_string(),
                    hostname: "new-device".to_string(),
                    platform: "windows".to_string(),
                    architecture: "x64".to_string(),
                    version: "1.0.0".to_string(),
                    last_seen: Some(chrono::Utc::now().to_rfc3339()),
                    is_online: true,
                    owner_id: "current-user".to_string(),
                    group_id: None,
                    tags: vec![],
                    created_at: chrono::Utc::now().to_rfc3339(),
                    updated_at: chrono::Utc::now().to_rfc3339(),
                };
                set_device_info.set(Some(mock_device));
            },
            Duration::from_secs(3),
        );
    });

    view! {
        <div class="row justify-content-center">
            <div class="col-lg-8">
                <div class="d-flex justify-content-between align-items-center mb-4">
                    <h3 class="h5 fw-bold mb-0">"Device Verification"</h3>
                    <button class="btn btn-outline-secondary" on:click=move |_| on_back()>
                        <i class="bi bi-arrow-left me-2"></i>
                        "Back"
                    </button>
                </div>
                
                <div class="card border-0 shadow-sm">
                    <div class="card-body text-center p-5">
                        {move || {
                            match verification_status.get().as_str() {
                                "waiting" => view! {
                                    <div>
                                        <div class="mb-4">
                                            <div class="spinner-border text-primary" style="width: 3rem; height: 3rem;"></div>
                                        </div>
                                        <h5 class="fw-bold mb-3">"Waiting for Device Connection"</h5>
                                        <p class="text-muted mb-4">
                                            "Please ensure the GhostLink agent is running on your device and "
                                            "the configuration file is properly installed."
                                        </p>
                                        <div class="list-unstyled text-start">
                                            <div class="d-flex align-items-center mb-2">
                                                <div class="spinner-border spinner-border-sm text-primary me-3"></div>
                                                <span>"Scanning for device..."</span>
                                            </div>
                                            <div class="d-flex align-items-center mb-2">
                                                <i class="bi bi-clock text-muted me-3"></i>
                                                <span class="text-muted">"Verifying certificate..."</span>
                                            </div>
                                            <div class="d-flex align-items-center">
                                                <i class="bi bi-clock text-muted me-3"></i>
                                                <span class="text-muted">"Establishing connection..."</span>
                                            </div>
                                        </div>
                                    </div>
                                }.into_view(),
                                "connected" => view! {
                                    <div>
                                        <div class="mb-4">
                                            <i class="bi bi-check-circle text-success" style="font-size: 3rem;"></i>
                                        </div>
                                        <h5 class="fw-bold text-success mb-3">"Device Connected Successfully!"</h5>
                                        
                                        {move || {
                                            if let Some(device) = device_info.get() {
                                                view! {
                                                    <div class="bg-light rounded p-4 mb-4">
                                                        <div class="row text-start">
                                                            <div class="col-6">
                                                                <div class="mb-2">
                                                                    <small class="text-muted">"Device Name"</small>
                                                                    <div class="fw-semibold">{device.name}</div>
                                                                </div>
                                                                <div class="mb-2">
                                                                    <small class="text-muted">"Platform"</small>
                                                                    <div class="fw-semibold">{device.platform} {device.architecture}</div>
                                                                </div>
                                                            </div>
                                                            <div class="col-6">
                                                                <div class="mb-2">
                                                                    <small class="text-muted">"Device ID"</small>
                                                                    <div class="fw-semibold">{device.id}</div>
                                                                </div>
                                                                <div class="mb-2">
                                                                    <small class="text-muted">"Status"</small>
                                                                    <span class="badge bg-success">Online</span>
                                                                </div>
                                                            </div>
                                                        </div>
                                                    </div>
                                                }.into_view()
                                            } else {
                                                view! {}.into_view()
                                            }
                                        }}
                                        
                                        <div class="d-grid gap-2">
                                            <button class="btn btn-success btn-lg" on:click=move |_| on_complete()>
                                                <i class="bi bi-check-circle me-2"></i>
                                                "Complete Setup"
                                            </button>
                                            <button class="btn btn-outline-primary">
                                                <i class="bi bi-display me-2"></i>
                                                "Test Connection"
                                            </button>
                                        </div>
                                    </div>
                                }.into_view(),
                                _ => view! {}.into_view(),
                            }
                        }}
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn CompleteStep<F>(on_add_another: F) -> impl IntoView 
where 
    F: Fn() + 'static,
{
    view! {
        <div class="row justify-content-center">
            <div class="col-lg-8">
                <div class="card border-0 shadow-sm">
                    <div class="card-body text-center p-5">
                        <div class="mb-4">
                            <i class="bi bi-check-circle-fill text-success" style="font-size: 4rem;"></i>
                        </div>
                        <h3 class="h4 fw-bold text-success mb-3">"Setup Complete!"</h3>
                        <p class="text-muted mb-4 fs-6">
                            "Your device has been successfully added to your GhostLink network. "
                            "You can now access it remotely from the dashboard."
                        </p>
                        
                        <div class="row g-3 mb-4">
                            <div class="col-md-4">
                                <div class="bg-light rounded p-3">
                                    <i class="bi bi-shield-check text-success fs-4 d-block mb-2"></i>
                                    <h6 class="fw-semibold">"Secure Connection"</h6>
                                    <small class="text-muted">"End-to-end encrypted"</small>
                                </div>
                            </div>
                            <div class="col-md-4">
                                <div class="bg-light rounded p-3">
                                    <i class="bi bi-lightning-charge text-primary fs-4 d-block mb-2"></i>
                                    <h6 class="fw-semibold">"Ready to Use"</h6>
                                    <small class="text-muted">"Connect immediately"</small>
                                </div>
                            </div>
                            <div class="col-md-4">
                                <div class="bg-light rounded p-3">
                                    <i class="bi bi-gear text-info fs-4 d-block mb-2"></i>
                                    <h6 class="fw-semibold">"Configurable"</h6>
                                    <small class="text-muted">"Customize settings"</small>
                                </div>
                            </div>
                        </div>
                        
                        <div class="d-grid gap-2">
                            <a href="/" class="btn btn-primary btn-lg">
                                <i class="bi bi-speedometer2 me-2"></i>
                                "Go to Dashboard"
                            </a>
                            <button class="btn btn-outline-primary" on:click=move |_| on_add_another()>
                                <i class="bi bi-plus-circle me-2"></i>
                                "Add Another Device"
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}

// Helper functions for API calls
async fn create_device_invitation() -> Result<DeviceInvitation, String> {
    // This would typically call your API
    Ok(DeviceInvitation {
        id: "inv_123".to_string(),
        code: "123456".to_string(),
        qr_code: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==".to_string(),
        expires_at: chrono::Utc::now().checked_add_signed(chrono::Duration::minutes(15)).unwrap().to_rfc3339(),
        organization_id: "org_123".to_string(),
        created_by: "user_123".to_string(),
        used: false,
    })
}

async fn verify_discovered_device(_device_id: &str) -> Result<(), String> {
    // This would typically call your API to verify and add the device
    Ok(())
}

async fn get_installation_instructions(platform: &str) -> Result<InstallationInstructions, String> {
    // This would typically fetch from your API
    Ok(InstallationInstructions {
        platform: platform.to_string(),
        steps: get_default_steps(platform),
        download_url: get_download_url(platform),
        config_template: "".to_string(),
    })
}

async fn generate_config_file(_platform: &str) -> Result<String, String> {
    // This would generate a proper config file with server details
    Ok(r#"{
    "server_url": "https://your-ghostlink-server.com",
    "device_name": "My-Device",
    "organization_id": "your-org-id",
    "auto_connect": true,
    "security": {
        "require_approval": true,
        "allow_file_transfer": true,
        "allow_terminal": false
    }
}"#.to_string())
}

fn get_default_steps(platform: &str) -> Vec<String> {
    match platform {
        "windows" => vec![
            "Download the GhostLink installer".to_string(),
            "Run the installer as Administrator".to_string(),
            "Follow the installation wizard".to_string(),
            "Copy the configuration file to the installation directory".to_string(),
            "Start the GhostLink service".to_string(),
        ],
        "macos" => vec![
            "Download the GhostLink.dmg file".to_string(),
            "Mount the disk image and drag GhostLink to Applications".to_string(),
            "Launch GhostLink from Applications".to_string(),
            "Grant necessary permissions in System Preferences".to_string(),
            "Copy the configuration file to the app bundle".to_string(),
        ],
        "linux" => vec![
            "Download the appropriate package for your distribution".to_string(),
            "Install using your package manager (apt, yum, etc.)".to_string(),
            "Copy the configuration file to /etc/ghostlink/".to_string(),
            "Start the ghostlink service: sudo systemctl start ghostlink".to_string(),
            "Enable auto-start: sudo systemctl enable ghostlink".to_string(),
        ],
        _ => vec![
            "Download the GhostLink package for your platform".to_string(),
            "Follow platform-specific installation instructions".to_string(),
            "Configure the application with provided settings".to_string(),
        ],
    }
}

fn get_download_url(platform: &str) -> String {
    match platform {
        "windows" => "/downloads/ghostlink-windows.exe".to_string(),
        "macos" => "/downloads/ghostlink-macos.dmg".to_string(),
        "linux" => "/downloads/ghostlink-linux.deb".to_string(),
        "android" => "https://play.google.com/store/apps/details?id=com.ghostlink".to_string(),
        "ios" => "https://apps.apple.com/app/ghostlink/id123456789".to_string(),
        _ => "/downloads/".to_string(),
    }
}

fn set_timeout<F>(f: F, delay: Duration) 
where
    F: FnOnce() + 'static,
{
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;
    
    let closure = Closure::once(f);
    
    web_sys::window()
        .unwrap()
        .set_timeout_with_callback_and_timeout_and_arguments_0(
            closure.as_ref().unchecked_ref(),
            delay.as_millis() as i32,
        )
        .expect("should register timeout");
        
    closure.forget();
}