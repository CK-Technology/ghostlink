use std::sync::Arc;
use std::time::Instant;
use axum::{
    routing::get,
    Router,
    http::Method,
};
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
    services::ServeDir,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use anyhow::Result;

mod config;
mod models;
mod relay;
mod api;

use config::ServerConfig;
use relay::RelayServer;

#[derive(Clone)]
pub struct AppState {
    pub config: ServerConfig,
    pub relay_server: Arc<RelayServer>,
    pub start_time: Instant,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "atlasconnect=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = ServerConfig::load().unwrap_or_else(|_| {
        tracing::warn!("Could not load config, using defaults");
        ServerConfig::default()
    });

    tracing::info!("Starting AtlasConnect Server v{}", env!("CARGO_PKG_VERSION"));
    tracing::info!("Server configuration: {}:{}", config.server.host, config.server.port);

    // Initialize relay server
    let relay_server = Arc::new(RelayServer::new(config.relay.max_relay_connections));
    
    // Start cleanup task
    relay_server.clone().start_cleanup_task();

    // Create application state
    let state = Arc::new(AppState {
        config: config.clone(),
        relay_server,
        start_time: Instant::now(),
    });

    // Create CORS layer
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers(Any)
        .allow_origin(Any);

    // Build application router
    let app = Router::new()
        .route("/", get(|| async { "AtlasConnect Server" }))
        .nest("/api/v1", api::create_api_router())
        .fallback_service(
            ServeDir::new(
                config.server.static_files_dir
                    .clone()
                    .unwrap_or_else(|| std::path::PathBuf::from("./web"))
            )
        )
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(cors)
        )
        .with_state(state);

    // Create listener
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", config.server.host, config.server.port))
        .await?;

    tracing::info!("🚀 AtlasConnect Server listening on http://{}:{}", config.server.host, config.server.port);
    tracing::info!("🔐 Zero trust relay active");
    tracing::info!("🌐 Web portal available at /");
    tracing::info!("📡 WebSocket relay endpoint: /api/v1/relay");

    // Start server
    axum::serve(listener, app).await?;

    Ok(())
}
