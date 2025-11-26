use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use tracing::{info, debug};

use crate::AppState;

/// Connection banner and branding manager
pub struct BrandingManager {
    /// Active connection banners
    banners: Arc<RwLock<HashMap<Uuid, ConnectionBanner>>>,
    /// Global branding configuration
    global_config: Arc<RwLock<BrandingConfig>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionBanner {
    pub id: Uuid,
    pub session_id: Uuid,
    pub banner_type: BannerType,
    pub title: String,
    pub message: String,
    pub company_name: String,
    pub company_logo: Option<String>,
    pub support_info: SupportInfo,
    pub display_settings: DisplaySettings,
    pub security_notice: Option<SecurityNotice>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub acknowledgment_required: bool,
    pub acknowledged_by: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BannerType {
    /// Standard connection banner
    Connection,
    /// Security warning banner  
    Security,
    /// Maintenance notification
    Maintenance,
    /// Legal/compliance notice
    Legal,
    /// Custom branded banner
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportInfo {
    pub phone: Option<String>,
    pub email: Option<String>,
    pub website: Option<String>,
    pub hours: Option<String>,
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplaySettings {
    pub position: BannerPosition,
    pub size: BannerSize,
    pub theme: BannerTheme,
    pub auto_hide_seconds: Option<u32>,
    pub transparency: f32,
    pub always_on_top: bool,
    pub show_minimize_button: bool,
    pub show_close_button: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BannerPosition {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
    Center,
    Custom { x: i32, y: i32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BannerSize {
    Small,    // 300x150
    Medium,   // 400x200
    Large,    // 500x300
    FullWidth, // Full screen width
    Custom { width: u32, height: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BannerTheme {
    Professional, // Corporate blue theme
    Security,     // Red security theme
    Success,      // Green success theme
    Warning,      // Yellow warning theme
    Dark,         // Dark mode theme
    Light,        // Light mode theme
    Custom { 
        background_color: String,
        text_color: String,
        border_color: String,
        accent_color: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityNotice {
    pub classification: SecurityClassification,
    pub warning_text: String,
    pub compliance_info: Option<String>,
    pub audit_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityClassification {
    Public,
    Internal,
    Confidential,
    Restricted,
    TopSecret,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrandingConfig {
    pub company_name: String,
    pub company_logo: Option<String>,
    pub primary_color: String,
    pub secondary_color: String,
    pub accent_color: String,
    pub font_family: String,
    pub custom_css: Option<String>,
    pub favicon: Option<String>,
    pub watermark: Option<WatermarkConfig>,
    pub footer_text: Option<String>,
    pub terms_of_service_url: Option<String>,
    pub privacy_policy_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatermarkConfig {
    pub enabled: bool,
    pub text: String,
    pub position: WatermarkPosition,
    pub opacity: f32,
    pub font_size: u32,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WatermarkPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
    Tiled,
}

impl BrandingManager {
    pub fn new() -> Self {
        Self {
            banners: Arc::new(RwLock::new(HashMap::new())),
            global_config: Arc::new(RwLock::new(Self::default_branding_config())),
        }
    }
    
    /// Initialize with default branding configuration
    pub async fn initialize(&self) -> Result<(), String> {
        info!("Initializing branding manager");
        
        // Load configuration from database/file if available
        // For now, use defaults
        
        info!("Branding manager initialized successfully");
        Ok(())
    }
    
    /// Default branding configuration
    fn default_branding_config() -> BrandingConfig {
        BrandingConfig {
            company_name: "GhostLink Remote Access".to_string(),
            company_logo: Some("/assets/logo.png".to_string()),
            primary_color: "#0d6efd".to_string(),
            secondary_color: "#6c757d".to_string(),
            accent_color: "#198754".to_string(),
            font_family: "system-ui, -apple-system, sans-serif".to_string(),
            custom_css: None,
            favicon: Some("/assets/favicon.ico".to_string()),
            watermark: Some(WatermarkConfig {
                enabled: false,
                text: "GhostLink Remote Session".to_string(),
                position: WatermarkPosition::BottomRight,
                opacity: 0.5,
                font_size: 12,
                color: "#666666".to_string(),
            }),
            footer_text: Some("Powered by GhostLink".to_string()),
            terms_of_service_url: None,
            privacy_policy_url: None,
        }
    }
    
    /// Create connection banner for session
    pub async fn create_connection_banner(&self, session_id: Uuid, banner_request: CreateBannerRequest) -> Result<ConnectionBanner, String> {
        let banner = ConnectionBanner {
            id: Uuid::new_v4(),
            session_id,
            banner_type: banner_request.banner_type,
            title: banner_request.title,
            message: banner_request.message,
            company_name: banner_request.company_name,
            company_logo: banner_request.company_logo,
            support_info: banner_request.support_info,
            display_settings: banner_request.display_settings,
            security_notice: banner_request.security_notice,
            created_at: chrono::Utc::now(),
            expires_at: banner_request.expires_at,
            acknowledgment_required: banner_request.acknowledgment_required,
            acknowledged_by: Vec::new(),
        };
        
        // Store banner
        {
            let mut banners = self.banners.write().await;
            banners.insert(banner.id, banner.clone());
        }
        
        info!("Created connection banner {} for session {}", banner.id, session_id);
        Ok(banner)
    }
    
    /// Get connection banner for session
    pub async fn get_session_banner(&self, session_id: Uuid) -> Option<ConnectionBanner> {
        let banners = self.banners.read().await;
        banners.values()
            .find(|banner| banner.session_id == session_id)
            .cloned()
    }
    
    /// Acknowledge banner
    pub async fn acknowledge_banner(&self, banner_id: Uuid, user_id: String) -> Result<(), String> {
        let mut banners = self.banners.write().await;
        
        if let Some(banner) = banners.get_mut(&banner_id) {
            if !banner.acknowledged_by.contains(&user_id) {
                banner.acknowledged_by.push(user_id.clone());
                info!("Banner {} acknowledged by user {}", banner_id, user_id);
            }
            Ok(())
        } else {
            Err(format!("Banner {} not found", banner_id))
        }
    }
    
    /// Get global branding configuration
    pub async fn get_branding_config(&self) -> BrandingConfig {
        self.global_config.read().await.clone()
    }
    
    /// Update global branding configuration
    pub async fn update_branding_config(&self, config: BrandingConfig) -> Result<(), String> {
        {
            let mut global_config = self.global_config.write().await;
            *global_config = config;
        }
        
        info!("Updated global branding configuration");
        Ok(())
    }
    
    /// Create default connection banner
    pub async fn create_default_connection_banner(&self, session_id: Uuid) -> Result<ConnectionBanner, String> {
        let config = self.get_branding_config().await;
        
        let request = CreateBannerRequest {
            banner_type: BannerType::Connection,
            title: "Remote Connection Established".to_string(),
            message: format!("You are now connected to a remote session managed by {}.", config.company_name),
            company_name: config.company_name,
            company_logo: config.company_logo,
            support_info: SupportInfo {
                phone: Some("+1-555-SUPPORT".to_string()),
                email: Some("support@ghostlink.com".to_string()),
                website: Some("https://support.ghostlink.com".to_string()),
                hours: Some("24/7".to_string()),
                timezone: Some("UTC".to_string()),
            },
            display_settings: DisplaySettings {
                position: BannerPosition::TopCenter,
                size: BannerSize::Medium,
                theme: BannerTheme::Professional,
                auto_hide_seconds: Some(10),
                transparency: 0.95,
                always_on_top: true,
                show_minimize_button: true,
                show_close_button: true,
            },
            security_notice: Some(SecurityNotice {
                classification: SecurityClassification::Internal,
                warning_text: "This session may be monitored and recorded for security purposes.".to_string(),
                compliance_info: Some("Session complies with corporate security policy v2.1".to_string()),
                audit_required: false,
            }),
            expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(8)),
            acknowledgment_required: false,
        };
        
        self.create_connection_banner(session_id, request).await
    }
    
    /// Generate CSS for custom theming
    pub async fn generate_theme_css(&self) -> String {
        let config = self.get_branding_config().await;
        
        format!(
            r#"
            :root {{
                --brand-primary: {};
                --brand-secondary: {};
                --brand-accent: {};
                --brand-font: {};
            }}
            
            .ghostlink-banner {{
                font-family: var(--brand-font);
                background: var(--brand-primary);
                border: 2px solid var(--brand-accent);
                border-radius: 8px;
                box-shadow: 0 4px 12px rgba(0,0,0,0.15);
                color: white;
                padding: 16px;
                max-width: 500px;
                z-index: 10000;
            }}
            
            .ghostlink-banner .banner-title {{
                font-size: 18px;
                font-weight: bold;
                margin-bottom: 8px;
                color: white;
            }}
            
            .ghostlink-banner .banner-message {{
                font-size: 14px;
                line-height: 1.4;
                margin-bottom: 12px;
                opacity: 0.95;
            }}
            
            .ghostlink-banner .support-info {{
                font-size: 12px;
                border-top: 1px solid rgba(255,255,255,0.2);
                padding-top: 8px;
                margin-top: 8px;
            }}
            
            .ghostlink-banner .security-notice {{
                background: rgba(220, 53, 69, 0.1);
                border: 1px solid #dc3545;
                border-radius: 4px;
                padding: 8px;
                margin-top: 8px;
                font-size: 11px;
            }}
            
            .ghostlink-watermark {{
                position: fixed;
                pointer-events: none;
                user-select: none;
                opacity: {};
                font-size: {}px;
                color: {};
                font-family: var(--brand-font);
                font-weight: bold;
                text-shadow: 1px 1px 2px rgba(0,0,0,0.5);
            }}
            
            {}
            "#,
            config.primary_color,
            config.secondary_color, 
            config.accent_color,
            config.font_family,
            config.watermark.as_ref().map(|w| w.opacity).unwrap_or(0.5),
            config.watermark.as_ref().map(|w| w.font_size).unwrap_or(12),
            config.watermark.as_ref().map(|w| w.color.as_str()).unwrap_or("#666666"),
            config.custom_css.unwrap_or_default()
        )
    }
    
    /// Clean up expired banners
    pub async fn cleanup_expired_banners(&self) {
        let now = chrono::Utc::now();
        let mut banners = self.banners.write().await;
        
        let expired_banners: Vec<Uuid> = banners
            .iter()
            .filter_map(|(id, banner)| {
                if let Some(expires_at) = banner.expires_at {
                    if expires_at < now {
                        Some(*id)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        
        for banner_id in expired_banners {
            banners.remove(&banner_id);
            debug!("Removed expired banner {}", banner_id);
        }
    }
}

/// API request types

#[derive(Debug, Deserialize)]
pub struct CreateBannerRequest {
    pub banner_type: BannerType,
    pub title: String,
    pub message: String,
    pub company_name: String,
    pub company_logo: Option<String>,
    pub support_info: SupportInfo,
    pub display_settings: DisplaySettings,
    pub security_notice: Option<SecurityNotice>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub acknowledgment_required: bool,
}

/// API Handlers

/// Create connection banner
pub async fn api_create_banner(
    State(app_state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(request): Json<CreateBannerRequest>,
) -> Response {
    match app_state.device_manager.branding_manager.create_connection_banner(session_id, request).await {
        Ok(banner) => Json(banner).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e
            }))
        ).into_response(),
    }
}

/// Get session banner
pub async fn api_get_session_banner(
    State(app_state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Response {
    match app_state.device_manager.branding_manager.get_session_banner(session_id).await {
        Some(banner) => Json(banner).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "No banner found for session"
            }))
        ).into_response(),
    }
}

/// Acknowledge banner
pub async fn api_acknowledge_banner(
    State(app_state): State<AppState>,
    Path(banner_id): Path<Uuid>,
    Json(request): Json<serde_json::Value>,
) -> Response {
    let user_id = match request.get("user_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Missing user_id"
            }))
        ).into_response(),
    };
    
    match app_state.device_manager.branding_manager.acknowledge_banner(banner_id, user_id).await {
        Ok(_) => Json(serde_json::json!({
            "status": "acknowledged"
        })).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e
            }))
        ).into_response(),
    }
}

/// Get branding configuration
pub async fn api_get_branding_config(
    State(app_state): State<AppState>,
) -> impl IntoResponse {
    let config = app_state.device_manager.branding_manager.get_branding_config().await;
    Json(config)
}

/// Update branding configuration
pub async fn api_update_branding_config(
    State(app_state): State<AppState>,
    Json(config): Json<BrandingConfig>,
) -> Response {
    match app_state.device_manager.branding_manager.update_branding_config(config).await {
        Ok(_) => Json(serde_json::json!({
            "status": "updated"
        })).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e
            }))
        ).into_response(),
    }
}

/// Generate theme CSS
pub async fn api_get_theme_css(
    State(app_state): State<AppState>,
) -> Response {
    let css = app_state.device_manager.branding_manager.generate_theme_css().await;
    (
        StatusCode::OK,
        [("content-type", "text/css")],
        css
    ).into_response()
}