use axum::{
    extract::ws::{Message, WebSocket},
    response::Response,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use uuid::Uuid;

// Handle WebSocket connections for the relay
pub async fn handle_websocket(socket: WebSocket, device_id: Option<String>, session_type: String) {
    let device_id = device_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    
    info!("New WebSocket connection: device_id={}, session_type={}", device_id, session_type);
    
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
            // Screen capture data
            info!("Received screen capture from device: {} ({} bytes)", device_id, data.len());
            // Process and forward to connected viewers
        }
        0x02 => {
            // Input event data
            info!("Received input event from device: {}", device_id);
            // Process input events
        }
        0x03 => {
            // File transfer data
            info!("Received file transfer data from device: {} ({} bytes)", device_id, data.len());
            // Handle file transfer
        }
        _ => {
            warn!("Unknown binary message type from device: {}", device_id);
        }
    }
    
    Ok(())
}
