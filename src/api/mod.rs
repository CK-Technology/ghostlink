use axum::{
    extract::{ws::WebSocketUpgrade, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    relay::RelayServer,
    models::{Agent, Session, User},
    AppState,
};

pub fn create_api_router() -> Router<Arc<AppState>> {
    Router::new()
        // Authentication
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/refresh", post(refresh_token))
        
        // Agents
        .route("/agents", get(list_agents))
        .route("/agents/:id", get(get_agent))
        .route("/agents/:id/status", get(get_agent_status))
        
        // Sessions
        .route("/sessions", get(list_sessions))
        .route("/sessions", post(create_session))
        .route("/sessions/:id", get(get_session))
        .route("/sessions/:id/end", post(end_session))
        
        // Users
        .route("/users", get(list_users))
        .route("/users", post(create_user))
        .route("/users/:id", get(get_user))
        .route("/users/:id", post(update_user))
        
        // WebSocket relay
        .route("/relay", get(websocket_handler))
        
        // Server status
        .route("/status", get(server_status))
}

#[derive(Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    pub mfa_code: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct LoginResponse {
    pub success: bool,
    pub token: Option<String>,
    pub user: Option<User>,
    pub message: String,
    pub requires_mfa: bool,
}

#[derive(Serialize, Deserialize)]
pub struct CreateSessionRequest {
    pub agent_id: Uuid,
    pub session_type: crate::models::SessionType,
}

#[derive(Serialize, Deserialize)]
pub struct SessionResponse {
    pub session_id: Uuid,
    pub launch_url: String,
    pub status: String,
}

#[derive(Serialize, Deserialize)]
pub struct ServerStatusResponse {
    pub version: String,
    pub uptime_seconds: u64,
    pub active_agents: usize,
    pub active_sessions: usize,
    pub total_connections: usize,
}

// Authentication endpoints
async fn login(
    State(state): State<Arc<AppState>>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    // TODO: Implement proper authentication
    // 1. Validate email/password
    // 2. Check MFA if enabled
    // 3. Generate JWT token
    // 4. Return user info
    
    let response = LoginResponse {
        success: true,
        token: Some("dummy-jwt-token".to_string()),
        user: None, // TODO: Return actual user
        message: "Login successful".to_string(),
        requires_mfa: false,
    };
    
    Ok(Json(response))
}

async fn logout(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // TODO: Invalidate token
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Logged out successfully"
    })))
}

async fn refresh_token(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // TODO: Validate current token and issue new one
    Ok(Json(serde_json::json!({
        "success": true,
        "token": "new-jwt-token"
    })))
}

// Agent endpoints
async fn list_agents(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<crate::relay::AgentInfo>>, StatusCode> {
    let agents = state.relay_server.get_online_agents().await;
    Ok(Json(agents))
}

async fn get_agent(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Agent>, StatusCode> {
    // TODO: Get agent from database
    Err(StatusCode::NOT_IMPLEMENTED)
}

async fn get_agent_status(
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // TODO: Get real-time agent status
    Ok(Json(serde_json::json!({
        "online": true,
        "cpu_usage": 25.5,
        "memory_usage": 45.2,
        "disk_usage": 60.1
    })))
}

// Session endpoints
async fn list_sessions(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<Vec<Session>>, StatusCode> {
    // TODO: Get sessions from database
    Ok(Json(vec![]))
}

async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateSessionRequest>,
) -> Result<Json<SessionResponse>, StatusCode> {
    let session_id = Uuid::new_v4();
    
    // TODO: 
    // 1. Validate user permissions for agent
    // 2. Check if agent is online
    // 3. Create session in database
    // 4. Send session request to agent via relay
    
    let response = SessionResponse {
        session_id,
        launch_url: format!("/client/session/{}", session_id),
        status: "pending".to_string(),
    };
    
    Ok(Json(response))
}

async fn get_session(
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<Uuid>,
) -> Result<Json<Session>, StatusCode> {
    // TODO: Get session from database
    Err(StatusCode::NOT_IMPLEMENTED)
}

async fn end_session(
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // TODO: End session and update database
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Session ended"
    })))
}

// User endpoints
async fn list_users(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<Vec<User>>, StatusCode> {
    // TODO: Get users from database (with proper filtering)
    Ok(Json(vec![]))
}

async fn create_user(
    State(_state): State<Arc<AppState>>,
    Json(_user): Json<User>,
) -> Result<Json<User>, StatusCode> {
    // TODO: Create user in database
    Err(StatusCode::NOT_IMPLEMENTED)
}

async fn get_user(
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<Uuid>,
) -> Result<Json<User>, StatusCode> {
    // TODO: Get user from database
    Err(StatusCode::NOT_IMPLEMENTED)
}

async fn update_user(
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<Uuid>,
    Json(_user): Json<User>,
) -> Result<Json<User>, StatusCode> {
    // TODO: Update user in database
    Err(StatusCode::NOT_IMPLEMENTED)
}

// WebSocket handler
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let client_ip = params
        .get("client_ip")
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());

    ws.on_upgrade(move |socket| async move {
        if let Err(e) = state.relay_server.handle_connection(socket, client_ip).await {
            tracing::error!("WebSocket connection error: {}", e);
        }
    })
}

// Server status
async fn server_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ServerStatusResponse>, StatusCode> {
    let response = ServerStatusResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: state.start_time.elapsed().as_secs(),
        active_agents: state.relay_server.get_online_agents().await.len(),
        active_sessions: state.relay_server.get_active_sessions_count().await,
        total_connections: 0, // TODO: Track this
    };
    
    Ok(Json(response))
}
