use axum::{
    extract::ws::{Message, WebSocket},
    response::Response,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use uuid::Uuid;
use crate::device_manager::DeviceManager;

// Handle WebSocket connections for devices
pub async fn handle_websocket(
    socket: WebSocket, 
    agent_id: Option<String>, 
    session_type: String,
    device_manager: Arc<DeviceManager>
) {
    let device_id = agent_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    
    info!("New WebSocket connection: agent_id={}, session_type={}", device_id, session_type);
    
    let (mut sender, mut receiver) = socket.split();
    
    // Create channels for communication
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    
    // Spawn task to handle outgoing messages
    let tx_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if sender.send(message).await.is_err() {
                break;
            }
        }
    });
    
    // Handle incoming messages
    let device_id_clone = device_id.clone();
    let rx_task = tokio::spawn(async move {
        while let Some(message) = receiver.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    info!("Received text message from {}: {}", device_id_clone, text);
                    // Handle text messages (commands, status updates, etc.)
                    if let Err(e) = handle_text_message(&device_id_clone, &text, &tx).await {
                        error!("Error handling text message: {}", e);
                    }
                }
                Ok(Message::Binary(data)) => {
                    info!("Received binary message from {} ({} bytes)", device_id_clone, data.len());
                    // Handle binary messages (screen data, file transfers, etc.)
                    if let Err(e) = handle_binary_message(&device_id_clone, data, &tx).await {
                        error!("Error handling binary message: {}", e);
                    }
                }
                Ok(Message::Ping(data)) => {
                    if tx.send(Message::Pong(data)).is_err() {
                        break;
                    }
                }
                Ok(Message::Pong(_)) => {
                    // Pong received
                }
                Ok(Message::Close(_)) => {
                    info!("WebSocket connection closed for device: {}", device_id_clone);
                    break;
                }
                Err(e) => {
                    error!("WebSocket error for device {}: {}", device_id_clone, e);
                    break;
                }
            }
        }
    });
    
    // Wait for either task to complete
    tokio::select! {
        _ = tx_task => {
            warn!("TX task completed for device: {}", device_id);
        }
        _ = rx_task => {
            warn!("RX task completed for device: {}", device_id);
        }
    }
    
    info!("WebSocket handler completed for device: {}", device_id);
}

async fn handle_text_message(
    device_id: &str,
    message: &str,
    tx: &mpsc::UnboundedSender<Message>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Parse JSON message
    let parsed: serde_json::Value = serde_json::from_str(message)?;
    
    match parsed.get("type").and_then(|v| v.as_str()) {
        Some("register") => {
            info!("Device registration: {}", device_id);
            let response = serde_json::json!({
                "type": "register_response",
                "status": "success",
                "device_id": device_id
            });
            tx.send(Message::Text(response.to_string()))?;
        }
        Some("heartbeat") => {
            // Respond to heartbeat
            let response = serde_json::json!({
                "type": "heartbeat_response",
                "timestamp": chrono::Utc::now()
            });
            tx.send(Message::Text(response.to_string()))?;
        }
        Some("screen_info") => {
            info!("Received screen info from device: {}", device_id);
            // Store screen information
        }
        _ => {
            warn!("Unknown message type from device {}: {}", device_id, message);
        }
    }
    
    Ok(())
}

async fn handle_binary_message(
    device_id: &str,
    data: Vec<u8>,
    tx: &mpsc::UnboundedSender<Message>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Handle different types of binary data based on first byte or header
    if data.is_empty() {
        return Ok(());
    }
    
    match data[0] {
        0x01 => {
            // Screen capture data - forward to all active sessions for this device
            info!("Received screen capture from device: {} ({} bytes)", device_id, data.len());
            if let Err(e) = forward_screen_data(device_id, &data[1..]).await {
                error!("Failed to forward screen data: {}", e);
            }
        }
        0x02 => {
            // Input event data - process and send to device if needed
            info!("Received input event from device: {}", device_id);
            if let Err(e) = process_input_event(device_id, &data[1..]).await {
                error!("Failed to process input event: {}", e);
            }
        }
        0x03 => {
            // File transfer data - handle file operations
            info!("Received file transfer data from device: {} ({} bytes)", device_id, data.len());
            if let Err(e) = handle_file_transfer(device_id, &data[1..]).await {
                error!("Failed to handle file transfer: {}", e);
            }
        }
        _ => {
            warn!("Unknown binary message type from device: {}", device_id);
        }
    }
    
    Ok(())
}

/// Forward screen capture data to all viewers of this device
async fn forward_screen_data(device_id: &str, frame_data: &[u8]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // TODO: Get all active sessions for this device from device manager
    // TODO: Forward frame data to each session's websocket
    info!("Forwarding {} bytes of screen data from device {}", frame_data.len(), device_id);
    Ok(())
}

/// Process input events (mouse/keyboard from viewers)
async fn process_input_event(device_id: &str, input_data: &[u8]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // TODO: Parse input event and forward to device
    info!("Processing input event for device {}", device_id);
    Ok(())
}

/// Handle file transfer operations
async fn handle_file_transfer(device_id: &str, file_data: &[u8]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // TODO: Handle file upload/download operations
    info!("Handling file transfer for device {} ({} bytes)", device_id, file_data.len());
    Ok(())
}

// Handle WebSocket connections for session clients (web browsers)
pub async fn handle_session_websocket(
    socket: WebSocket,
    session_id: Option<String>,
    session_type: String,
    device_manager: Arc<DeviceManager>
) {
    let session_id = session_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    
    info!("New session WebSocket connection: session_id={}, type={}", session_id, session_type);
    
    let (mut sender, mut receiver) = socket.split();
    
    // Create channels for communication
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    
    // Spawn task to handle outgoing messages
    let tx_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if sender.send(message).await.is_err() {
                break;
            }
        }
    });
    
    // Handle incoming messages from session
    let session_id_clone = session_id.clone();
    let device_manager_clone = device_manager.clone();
    let rx_task = tokio::spawn(async move {
        while let Some(message) = receiver.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    info!("Received text message from session {}: {}", session_id_clone, text);
                    if let Err(e) = handle_session_text_message(&session_id_clone, &text, &tx, &device_manager_clone).await {
                        error!("Error handling session text message: {}", e);
                    }
                }
                Ok(Message::Binary(data)) => {
                    info!("Received binary message from session {} ({} bytes)", session_id_clone, data.len());
                    if let Err(e) = handle_session_binary_message(&session_id_clone, data, &device_manager_clone).await {
                        error!("Error handling session binary message: {}", e);
                    }
                }
                Ok(Message::Ping(data)) => {
                    if tx.send(Message::Pong(data)).is_err() {
                        break;
                    }
                }
                Ok(Message::Pong(_)) => {}
                Ok(Message::Close(_)) => {
                    info!("Session WebSocket connection closed: {}", session_id_clone);
                    break;
                }
                Err(e) => {
                    error!("Session WebSocket error {}: {}", session_id_clone, e);
                    break;
                }
            }
        }
    });
    
    // Wait for either task to complete
    tokio::select! {
        _ = tx_task => {
            warn!("Session TX task completed: {}", session_id);
        }
        _ = rx_task => {
            warn!("Session RX task completed: {}", session_id);
        }
    }
    
    info!("Session WebSocket handler completed: {}", session_id);
}

async fn handle_session_text_message(
    session_id: &str,
    message: &str,
    _tx: &mpsc::UnboundedSender<Message>,
    _device_manager: &Arc<DeviceManager>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Parse JSON message
    let parsed: serde_json::Value = serde_json::from_str(message)?;
    
    match parsed.get("type").and_then(|v| v.as_str()) {
        Some("control_request") => {
            info!("Control request from session: {}", session_id);
            // Handle control requests to devices
        }
        Some("session_info") => {
            info!("Session info from: {}", session_id);
            // Store session information
        }
        _ => {
            warn!("Unknown session message type: {}", message);
        }
    }
    
    Ok(())
}

async fn handle_session_binary_message(
    session_id: &str,
    data: Vec<u8>,
    device_manager: &Arc<DeviceManager>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if data.is_empty() {
        return Ok(());
    }
    
    // Parse session ID and forward input events
    if let Ok(session_uuid) = Uuid::parse_str(session_id) {
        match data[0] {
            0x02 => {
                // Input event data from session - forward to device
                info!("Forwarding input event from session: {}", session_id);
                if let Err(e) = device_manager.forward_input_event(session_uuid, data).await {
                    error!("Failed to forward input event: {}", e);
                }
            }
            _ => {
                warn!("Unknown binary message type from session: {}", session_id);
            }
        }
    }
    
    Ok(())
}
