use axum::{
    extract::{Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Router,
};
use leptos::*;
use leptos_axum::{generate_route_list, handle_server_fns, LeptosRoutes};
use leptos_config::get_configuration;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    services::ServeDir,
    trace::TraceLayer,
};
use tracing::{info, warn};

mod config;
mod models;
mod relay;
mod web;

use crate::{
    config::AppConfig,
    models::{Agent, Session},
    relay::handle_websocket,
    web::app::App,
};

// Application state for the server
#[derive(Clone)]
pub struct AppState {
    pub devices: Arc<RwLock<HashMap<String, Agent>>>,
    pub sessions: Arc<RwLock<HashMap<String, Session>>>,
    pub config: AppConfig,
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
    let app_state = AppState {
        devices: Arc::new(RwLock::new(HashMap::new())),
        sessions: Arc::new(RwLock::new(HashMap::new())),
        config: config.clone(),
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
        .route("/ws", get(websocket_handler))
        .route("/health", get(health_check))
        .route("/register", post(api_register_device))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive()),
        );

    // Build web GUI routes (for atlas.cktechx.com - admin interface)
    let web_routes = Router::new()
        .leptos_routes(&leptos_options, routes, App)
        .fallback(file_and_error_handler)
        .route("/api/devices", get(api_get_devices))
        .route("/api/devices/:id/connect", post(api_connect_device))
        .route("/api/*fn_name", post(handle_server_fns))
        .nest_service("/pkg", ServeDir::new("target/site/pkg"))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive()),
        )
        .with_state(leptos_options.clone());

    // Combine routes with prefixes for nginx routing
    let app = Router::new()
        .nest("/relay", relay_routes) // relay.cktechx.com/relay/*
        .merge(web_routes) // atlas.cktechx.com/*
        .with_state(app_state);

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

// API Handlers
async fn api_get_devices() -> impl IntoResponse {
    // TODO: Get real devices from state
    axum::Json(serde_json::json!({
        "devices": [
            {
                "id": "device-001",
                "name": "Windows Workstation",
                "os": "Windows 11",
                "status": "online",
                "last_seen": "2025-07-02T10:30:00Z",
                "ip_address": "192.168.1.100"
            },
            {
                "id": "device-002", 
                "name": "Ubuntu Server",
                "os": "Ubuntu 22.04",
                "status": "online",
                "last_seen": "2025-07-02T10:29:45Z",
                "ip_address": "192.168.1.50"
            }
        ]
    }))
}

async fn api_connect_device(
    axum::extract::Path(device_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    info!("Initiating connection to device: {}", device_id);
    axum::Json(serde_json::json!({
        "status": "success",
        "message": "Connection initiated",
        "session_id": uuid::Uuid::new_v4()
    }))
}

// Health check endpoint
async fn health_check() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "status": "healthy",
        "service": "atlasconnect-server",
        "timestamp": chrono::Utc::now()
    }))
}

// Device registration endpoint (for clients)
async fn api_register_device(
    axum::Json(device_info): axum::Json<serde_json::Value>,
) -> impl IntoResponse {
    info!("Device registration: {:?}", device_info);
    
    let device_id = uuid::Uuid::new_v4().to_string();
    
    axum::Json(serde_json::json!({
        "status": "success",
        "device_id": device_id,
        "message": "Device registered successfully"
    }))
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let device_id = params.get("device_id").cloned();
    let session_type = params.get("type").cloned().unwrap_or_else(|| "control".to_string());
    
    ws.on_upgrade(move |socket| handle_websocket(socket, device_id, session_type))
}
