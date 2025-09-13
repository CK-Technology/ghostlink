use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use ring::{rand, signature};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use std::collections::HashMap;
use axum::{
    extract::{State, Json, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};

/// Device registration request
#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceRegistrationRequest {
    pub device_name: String,
    pub hostname: String,
    pub platform: String,
    pub architecture: String,
    pub os_version: String,
    pub agent_version: String,
    pub public_key: String,  // Base64 encoded public key
    pub capabilities: HashMap<String, bool>,
    pub organization_code: Option<String>,  // Optional org pairing code
}

/// Device registration response
#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceRegistrationResponse {
    pub device_id: Uuid,
    pub certificate: String,  // Base64 encoded certificate
    pub relay_url: String,
    pub api_key: String,
    pub expires_at: DateTime<Utc>,
}

/// Device certificate structure
#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceCertificate {
    pub device_id: Uuid,
    pub public_key: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub issuer: String,
    pub signature: String,  // Signature of the certificate data
    pub capabilities: HashMap<String, bool>,
    pub metadata: HashMap<String, String>,
}

/// Device authentication token
#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceAuthToken {
    pub device_id: Uuid,
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

/// Certificate authority for device certificates
pub struct DeviceCertificateAuthority {
    private_key: signature::Ed25519KeyPair,
    public_key: Vec<u8>,
    issuer_name: String,
}

impl DeviceCertificateAuthority {
    /// Create a new certificate authority
    pub fn new(issuer_name: String) -> Result<Self> {
        let rng = rand::SystemRandom::new();
        let pkcs8_bytes = signature::Ed25519KeyPair::generate_pkcs8(&rng)?;
        let key_pair = signature::Ed25519KeyPair::from_pkcs8(pkcs8_bytes.as_ref())?;
        let public_key = key_pair.public_key().as_ref().to_vec();
        
        Ok(Self {
            private_key: key_pair,
            public_key,
            issuer_name,
        })
    }
    
    /// Load CA from stored keys
    pub fn from_keys(private_key_pem: &str, issuer_name: String) -> Result<Self> {
        let private_key_bytes = BASE64.decode(private_key_pem)?;
        let key_pair = signature::Ed25519KeyPair::from_pkcs8(&private_key_bytes)?;
        let public_key = key_pair.public_key().as_ref().to_vec();
        
        Ok(Self {
            private_key: key_pair,
            public_key,
            issuer_name,
        })
    }
    
    /// Issue a device certificate
    pub fn issue_certificate(
        &self,
        device_id: Uuid,
        device_public_key: &str,
        capabilities: HashMap<String, bool>,
        validity_days: i64,
    ) -> Result<DeviceCertificate> {
        let now = Utc::now();
        let expires_at = now + Duration::days(validity_days);
        
        let cert_data = DeviceCertificate {
            device_id,
            public_key: device_public_key.to_string(),
            issued_at: now,
            expires_at,
            issuer: self.issuer_name.clone(),
            signature: String::new(), // Will be filled after signing
            capabilities,
            metadata: HashMap::new(),
        };
        
        // Serialize certificate data for signing
        let cert_json = serde_json::to_string(&cert_data)?;
        
        // Sign the certificate
        let signature = self.private_key.sign(cert_json.as_bytes());
        let signature_base64 = BASE64.encode(signature.as_ref());
        
        // Create final certificate with signature
        let mut signed_cert = cert_data;
        signed_cert.signature = signature_base64;
        
        Ok(signed_cert)
    }
    
    /// Verify a device certificate
    pub fn verify_certificate(&self, certificate: &DeviceCertificate) -> Result<bool> {
        // Check expiration
        if certificate.expires_at < Utc::now() {
            return Ok(false);
        }
        
        // Prepare certificate data for verification (without signature)
        let mut cert_copy = certificate.clone();
        let signature_base64 = cert_copy.signature.clone();
        cert_copy.signature = String::new();
        
        let cert_json = serde_json::to_string(&cert_copy)?;
        let signature_bytes = BASE64.decode(&signature_base64)?;
        
        // Verify signature
        let public_key = signature::UnparsedPublicKey::new(
            &signature::ED25519,
            &self.public_key,
        );
        
        match public_key.verify(cert_json.as_bytes(), &signature_bytes) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

/// Device registration service
pub struct DeviceRegistrationService {
    ca: DeviceCertificateAuthority,
    db: crate::database::DatabaseService,
}

impl DeviceRegistrationService {
    pub fn new(ca: DeviceCertificateAuthority, db: crate::database::DatabaseService) -> Self {
        Self { ca, db }
    }
    
    /// Register a new device
    pub async fn register_device(
        &self,
        request: DeviceRegistrationRequest,
    ) -> Result<DeviceRegistrationResponse> {
        // Generate device ID
        let device_id = Uuid::new_v4();
        
        // Issue certificate (valid for 1 year)
        let certificate = self.ca.issue_certificate(
            device_id,
            &request.public_key,
            request.capabilities.clone(),
            365,
        )?;
        
        // Generate API key for the device
        let api_key = Self::generate_api_key();
        
        // Store device in database
        let agent = crate::models::Agent {
            id: device_id,
            organization_id: None, // TODO: Resolve from organization_code
            name: request.device_name,
            hostname: Some(request.hostname),
            platform: request.platform,
            architecture: Some(request.architecture),
            os_version: Some(request.os_version),
            agent_version: Some(request.agent_version),
            public_key: Some(request.public_key),
            last_seen: None,
            status: "registered".to_string(),
            connection_info: sqlx::types::Json(HashMap::new()),
            capabilities: sqlx::types::Json(request.capabilities),
            settings: sqlx::types::Json(HashMap::new()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        self.db.create_agent(&agent).await?;
        
        // Create response
        Ok(DeviceRegistrationResponse {
            device_id,
            certificate: BASE64.encode(serde_json::to_vec(&certificate)?),
            relay_url: std::env::var("RELAY_URL")
                .unwrap_or_else(|_| "wss://relay.ghostlink.local/ws".to_string()),
            api_key,
            expires_at: certificate.expires_at,
        })
    }
    
    /// Verify device authentication
    pub async fn verify_device(
        &self,
        device_id: Uuid,
        certificate: &str,
    ) -> Result<bool> {
        // Decode certificate
        let cert_bytes = BASE64.decode(certificate)?;
        let cert: DeviceCertificate = serde_json::from_slice(&cert_bytes)?;
        
        // Verify certificate matches device ID
        if cert.device_id != device_id {
            return Ok(false);
        }
        
        // Verify certificate signature and expiration
        self.ca.verify_certificate(&cert)
    }
    
    /// Renew device certificate
    pub async fn renew_certificate(
        &self,
        device_id: Uuid,
        old_certificate: &str,
    ) -> Result<DeviceCertificate> {
        // Verify old certificate
        if !self.verify_device(device_id, old_certificate).await? {
            return Err(anyhow::anyhow!("Invalid or expired certificate"));
        }
        
        // Get device from database
        let agent = self.db.get_agent_by_id(device_id).await?
            .ok_or_else(|| anyhow::anyhow!("Device not found"))?;
        
        // Issue new certificate
        let new_cert = self.ca.issue_certificate(
            device_id,
            &agent.public_key.unwrap_or_default(),
            agent.capabilities.0,
            365, // 1 year validity
        )?;
        
        Ok(new_cert)
    }
    
    /// Generate a secure API key
    fn generate_api_key() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let key: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        BASE64.encode(&key)
    }
}

/// API endpoints for device registration
pub mod endpoints {
    use super::*;
    
    /// Register a new device
    pub async fn register_device(
        State(app_state): State<crate::AppState>,
        Json(request): Json<DeviceRegistrationRequest>,
    ) -> Result<Json<DeviceRegistrationResponse>, StatusCode> {
        // Create CA (in production, load from secure storage)
        let ca = DeviceCertificateAuthority::new("GhostLink CA".to_string())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        let service = DeviceRegistrationService::new(ca, app_state.db);
        
        match service.register_device(request).await {
            Ok(response) => Ok(Json(response)),
            Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
    
    /// Verify device certificate
    pub async fn verify_device(
        State(app_state): State<crate::AppState>,
        Path(device_id): Path<Uuid>,
        Json(certificate): Json<String>,
    ) -> impl IntoResponse {
        let ca = DeviceCertificateAuthority::new("GhostLink CA".to_string())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        let service = DeviceRegistrationService::new(ca, app_state.db);
        
        match service.verify_device(device_id, &certificate).await {
            Ok(valid) => Ok(Json(serde_json::json!({
                "valid": valid,
                "device_id": device_id
            }))),
            Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
    
    /// Renew device certificate
    pub async fn renew_certificate(
        State(app_state): State<crate::AppState>,
        Path(device_id): Path<Uuid>,
        Json(old_certificate): Json<String>,
    ) -> Result<Json<DeviceCertificate>, StatusCode> {
        let ca = DeviceCertificateAuthority::new("GhostLink CA".to_string())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        let service = DeviceRegistrationService::new(ca, app_state.db);
        
        match service.renew_certificate(device_id, &old_certificate).await {
            Ok(new_cert) => Ok(Json(new_cert)),
            Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
    
    /// Get device information
    pub async fn get_device_info(
        State(app_state): State<crate::AppState>,
        Path(device_id): Path<Uuid>,
    ) -> Result<impl IntoResponse, StatusCode> {
        match app_state.db.get_agent_by_id(device_id).await {
            Ok(Some(agent)) => Ok(Json(agent)),
            Ok(None) => Err(StatusCode::NOT_FOUND),
            Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
    
    /// List all registered devices
    pub async fn list_devices(
        State(app_state): State<crate::AppState>,
        crate::auth::jwt::AuthUser { org_id, .. }: crate::auth::jwt::AuthUser,
    ) -> Result<impl IntoResponse, StatusCode> {
        let org_uuid = org_id.and_then(|id| Uuid::parse_str(&id).ok());
        
        match app_state.db.get_connected_agents(org_uuid).await {
            Ok(agents) => Ok(Json(agents)),
            Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
}