use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::collections::HashMap;
use uuid::Uuid;
use crate::{
    device_manager::{SessionRequest, DeviceRegistration},
    models::SessionType,
    AppState,
};

/// Health check endpoint
pub async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "ghostlink-server",
        "timestamp": chrono::Utc::now()
    }))
}

/// Get all connected devices
pub async fn api_get_devices(
    State(app_state): State<AppState>,
) -> impl IntoResponse {
    let devices = app_state.device_manager.get_connected_devices().await;
    Json(serde_json::json!({
        "devices": devices
    }))
}

/// Get device statistics
pub async fn api_get_stats(
    State(app_state): State<AppState>,
) -> impl IntoResponse {
    let stats = app_state.device_manager.get_stats().await;
    Json(stats)
}

/// Get sessions for a specific device
pub async fn api_get_device_sessions(
    State(app_state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Response {
    match Uuid::parse_str(&agent_id) {
        Ok(agent_uuid) => {
            let sessions = app_state.device_manager.get_device_sessions(agent_uuid).await;
            Json(serde_json::json!({
                "sessions": sessions
            })).into_response()
        },
        Err(_) => {
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": "Invalid agent ID format"
            }))).into_response()
        }
    }
}

/// Create a new session with a device
#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub session_type: SessionType,
    pub user_id: Option<String>,
}

pub async fn api_create_session(
    State(app_state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(request): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    match Uuid::parse_str(&agent_id) {
        Ok(agent_uuid) => {
            let user_id = match request.user_id {
                Some(id_str) => match Uuid::parse_str(&id_str) {
                    Ok(uuid) => uuid,
                    Err(_) => return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({
                            "error": "Invalid user ID format"
                        }))
                    ).into_response()
                },
                None => Uuid::new_v4(), // Anonymous user
            };

            let session_request = SessionRequest {
                agent_id: agent_uuid,
                session_type: request.session_type,
                user_id,
            };

            // For now, create a dummy channel. In a real implementation,
            // this would come from a WebSocket upgrade
            let (tx, _) = tokio::sync::mpsc::unbounded_channel();

            match app_state.device_manager.create_session(session_request, tx).await {
                Ok(session_id) => {
                    Json(serde_json::json!({
                        "status": "success",
                        "session_id": session_id,
                        "message": "Session created successfully"
                    })).into_response()
                },
                Err(error) => {
                    (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                        "error": error
                    }))).into_response()
                }
            }
        },
        Err(_) => {
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": "Invalid agent ID format"
            }))).into_response()
        }
    }
}

/// End a session
pub async fn api_end_session(
    State(app_state): State<AppState>,
    Path(session_id): Path<String>,
) -> Response {
    match Uuid::parse_str(&session_id) {
        Ok(session_uuid) => {
            match app_state.device_manager.end_session(session_uuid).await {
                Ok(_) => {
                    Json(serde_json::json!({
                        "status": "success",
                        "message": "Session ended successfully"
                    })).into_response()
                },
                Err(error) => {
                    (StatusCode::NOT_FOUND, Json(serde_json::json!({
                        "error": error
                    }))).into_response()
                }
            }
        },
        Err(_) => {
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": "Invalid session ID format"
            }))).into_response()
        }
    }
}

/// Device registration endpoint (for clients)
pub async fn api_register_device(
    State(app_state): State<AppState>,
    Json(registration): Json<DeviceRegistration>,
) -> Response {
    // For now, create a dummy channel. In a real implementation,
    // this would come from a WebSocket connection
    let (tx, _) = tokio::sync::mpsc::unbounded_channel();

    match app_state.device_manager.register_device(registration, tx).await {
        Ok(agent_id) => {
            Json(serde_json::json!({
                "status": "success",
                "agent_id": agent_id,
                "message": "Device registered successfully"
            })).into_response()
        },
        Err(error) => {
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": error
            }))).into_response()
        }
    }
}

/// WebSocket handler for device connections
pub async fn websocket_device_handler(
    ws: WebSocketUpgrade,
    State(app_state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let agent_id = match params.get("agent_id").cloned() {
        Some(id) => id,
        None => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": "Missing agent_id parameter"
            }))).into_response();
        }
    };
    let session_type = params.get("type").cloned().unwrap_or_else(|| "device".to_string());

    ws.on_upgrade(move |socket| async move {
        crate::relay::handle_websocket(socket, agent_id, session_type, app_state.device_manager).await;
    })
}

/// WebSocket handler for session connections (web clients)
pub async fn websocket_session_handler(
    ws: WebSocketUpgrade,
    State(app_state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let session_id = match params.get("session_id").cloned() {
        Some(id) => id,
        None => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": "Missing session_id parameter"
            }))).into_response();
        }
    };
    let session_type = params.get("type").cloned().unwrap_or_else(|| "viewer".to_string());

    ws.on_upgrade(move |socket| async move {
        crate::relay::handle_session_websocket(socket, session_id, session_type, app_state.device_manager).await;
    })
}