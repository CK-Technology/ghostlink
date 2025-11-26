use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Router,
};
use leptos::*;
use leptos_axum::{generate_route_list, handle_server_fns, LeptosRoutes};
use leptos_config::get_configuration;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    services::ServeDir,
    trace::TraceLayer,
};
use tracing::info;

mod api;
mod config;
mod database;
mod models;
mod relay;
mod web;
mod device_manager;
mod toolbox;
mod branding;
mod direct_connect;
mod vpn_integration;
mod auth {
    pub mod jwt;
    pub mod oidc;
}
mod pam;
mod terminal;

use crate::{
    config::AppConfig,
    web::app::App,
    device_manager::DeviceManager,
    database::DatabaseService,
};

// Application state for the server
#[derive(Clone)]
pub struct AppState {
    pub device_manager: Arc<DeviceManager>,
    pub config: AppConfig,
    pub db: Option<Arc<DatabaseService>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Load configuration
    let config = AppConfig::load()?;
    info!("Starting AtlasConnect Server on {}:{}", config.host, config.port);

    // Initialize app state
    let device_manager = Arc::new(DeviceManager::new());
    
    // Initialize all managers
    if let Err(e) = device_manager.initialize().await {
        eprintln!("Failed to initialize device manager: {}", e);
        std::process::exit(1);
    }
    
    let app_state = AppState {
        device_manager,
        config: config.clone(),
        db: None, // Database connection is optional, set via DATABASE_URL if needed
    };

    // Setting get_configuration(None) means we'll be using cargo-leptos's env values
    // For deployment these variables are:
    // <https://github.com/leptos-rs/start-axum#executing-a-server-on-a-remote-machine-without-the-toolchain>
    // Alternately a file can be specified such as Some("Cargo.toml")
    // The file would need to be included with the executable when moved to deployment
    let conf = get_configuration(None).await.unwrap();
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(App);

    // Build relay routes (for relay.cktechx.com - client connections)
    let relay_routes = Router::new()
        .route("/ws", get(api::websocket_device_handler))
        .route("/health", get(api::health_check))
        .route("/register", post(api::api_register_device))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive()),
        )
        .with_state(app_state.clone());

    // Build API routes
    let api_routes = Router::new()
        // Authentication routes (public)
        .route("/api/auth/login", post(auth::jwt::endpoints::login))
        .route("/api/auth/refresh", post(auth::jwt::endpoints::refresh))
        .route("/api/auth/logout", post(auth::jwt::endpoints::logout))
        .route("/api/auth/me", get(auth::jwt::endpoints::me))
        
        // Device management routes (protected)
        .route("/api/devices", get(api::api_get_devices))
        .route("/api/devices/:id/sessions", get(api::api_get_device_sessions))
        .route("/api/devices/:id/sessions", post(api::api_create_session))
        .route("/api/sessions/:id", delete(api::api_end_session))
        .route("/api/stats", get(api::api_get_stats))
        .route("/api/ws", get(api::websocket_session_handler))
        
        // Toolbox API routes
        .route("/api/toolbox/tools", get(toolbox::api_get_tools))
        .route("/api/toolbox/available", get(toolbox::api_get_available_tools))
        .route("/api/toolbox/tools/:category", get(toolbox::api_get_tools_by_category))
        .route("/api/toolbox/execute", post(toolbox::api_execute_tool))
        .route("/api/toolbox/upload", post(toolbox::api_upload_tool))
        .route("/api/toolbox/upload-custom", post(toolbox::api_upload_custom_tool))
        .route("/api/toolbox/history", get(toolbox::api_get_execution_history))
        
        // Branding API routes
        .route("/api/branding/config", get(branding::api_get_branding_config))
        .route("/api/branding/config", post(branding::api_update_branding_config))
        .route("/api/branding/banners/:session_id", post(branding::api_create_banner))
        .route("/api/branding/banners/:session_id", get(branding::api_get_session_banner))
        .route("/api/branding/banners/:banner_id/acknowledge", post(branding::api_acknowledge_banner))
        .route("/api/branding/theme.css", get(branding::api_get_theme_css))
        
        // Direct Connect API routes
        .route("/api/direct/register", post(direct_connect::api_register_direct_client))
        .route("/api/direct/connect", post(direct_connect::api_connect_direct))
        .route("/api/direct/stats", get(direct_connect::api_direct_connect_stats))
        .route("/api/direct/relay/ws", get(direct_connect::websocket_direct_relay_handler))
        
        // VPN Integration API routes
        .route("/api/vpn/status", get(vpn_integration::api_get_vpn_status))
        .route("/api/vpn/peers", get(vpn_integration::api_get_vpn_peers))
        .route("/api/vpn/tailscale/enable", post(vpn_integration::api_enable_tailscale))
        .route("/api/vpn/wireguard/config", get(vpn_integration::api_get_wireguard_config))
        .route("/api/vpn/config", get(vpn_integration::api_get_vpn_config))
        .route("/api/vpn/config", put(vpn_integration::api_update_vpn_config))
        
        // OIDC Authentication API routes
        .route("/api/auth/oidc/login", get(auth::oidc::api_oidc_login))
        .route("/api/auth/oidc/callback", get(auth::oidc::api_oidc_callback))
        .route("/api/auth/oidc/oauth-callback", get(auth::oidc::api_oauth_callback))
        .route("/api/auth/oidc/auth-url", get(auth::oidc::api_get_auth_url))
        .route("/api/auth/oidc/validate", get(auth::oidc::api_validate_session))
        .route("/api/auth/oidc/logout", post(auth::oidc::api_logout))
        .route("/api/auth/oidc/nginx", get(auth::oidc::api_nginx_auth))
        .route("/api/auth/oidc/config", get(auth::oidc::api_get_oidc_config))
        .route("/api/auth/oidc/config", put(auth::oidc::api_update_oidc_config))
        
        // PAM (Privileged Access Management) API routes
        .route("/api/pam/sessions/:session_id/elevate", post(pam::api_request_elevation))
        .route("/api/pam/elevation/:request_id/approve", post(pam::api_approve_elevation))
        .route("/api/pam/elevation/:request_id/session", post(pam::api_start_elevated_session))
        .route("/api/pam/elevated/:session_id/execute", post(pam::api_execute_elevated_command))
        .route("/api/pam/audit", get(pam::api_get_pam_audit_log))
        .route("/api/pam/stats", get(pam::api_get_pam_stats))
        
        // Terminal API routes
        .route("/api/terminal/:client_session_id/create", post(terminal::api_create_terminal_session))
        .route("/api/terminal/:session_id", get(terminal::api_get_terminal_session))
        .route("/api/terminal/:session_id/output", get(terminal::api_get_terminal_output))
        .route("/api/terminal/:session_id/ws", get(terminal::websocket_terminal_handler))
        .route("/api/terminal/history", get(terminal::api_get_command_history))
        .route("/api/terminal/config", get(terminal::api_get_terminal_config))
        
        .with_state(app_state.clone());

    // Build web GUI routes (for atlas.cktechx.com - admin interface)
    let web_routes = Router::new()
        .leptos_routes(&leptos_options, routes, App)
        .fallback(file_and_error_handler)
        .route("/api/*fn_name", post(handle_server_fns))
        .nest_service("/pkg", ServeDir::new("target/site/pkg"))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive()),
        )
        .with_state(leptos_options.clone());

    // Combine routes - each router already has its own state bound
    // Use Router::<()> as the base to combine routers with different states
    let app = Router::<()>::new()
        .nest("/relay", relay_routes.with_state(())) // relay routes with AppState
        .merge(api_routes.with_state(())) // API routes with AppState
        .merge(web_routes.with_state(())); // Web routes with LeptosOptions

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    info!("🚀 AtlasConnect Server listening on http://{}", &addr);
    info!("📡 Relay endpoints: /relay/* (for clients)");
    info!("🌐 Web GUI: /* (for admins)");
    info!("💡 Configure nginx:");
    info!("   - relay.cktechx.com → proxy_pass to /relay/*");
    info!("   - atlas.cktechx.com → proxy_pass to /*");

    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

async fn file_and_error_handler(
    uri: axum::http::Uri,
    State(options): State<leptos::LeptosOptions>,
) -> Result<Response, (StatusCode, String)> {
    let root = options.site_root.clone();
    let (status, file) = get_static_file(uri.clone(), &root).await?;

    if status == StatusCode::OK {
        Ok(file.into_response())
    } else {
        let handler = leptos_axum::render_app_to_stream(options.to_owned(), App);
        Ok(handler(axum::http::Request::builder().uri(uri).body(axum::body::Body::empty()).unwrap()).await.into_response())
    }
}

async fn get_static_file(
    uri: axum::http::Uri,
    root: &str,
) -> Result<(StatusCode, axum::response::Response), (StatusCode, String)> {
    let req = axum::http::Request::builder()
        .uri(uri.clone())
        .body(axum::body::Body::empty())
        .unwrap();
    
    // `ServeDir` implements `tower::Service` so we can call it with `tower::ServiceExt::oneshot`
    match tower::ServiceExt::oneshot(ServeDir::new(root), req).await {
        Ok(res) => Ok((res.status(), res.into_response())),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {err}"),
        )),
    }
}

