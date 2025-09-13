use anyhow::Result;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use axum::{
    extract::{FromRequestParts, State},
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use crate::AppState;

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,        // Subject (user ID)
    pub email: String,      // User email
    pub role: String,       // User role
    pub exp: i64,          // Expiration time
    pub iat: i64,          // Issued at
    pub nbf: i64,          // Not before
    pub jti: String,       // JWT ID (for revocation)
    pub org_id: Option<String>, // Organization ID
}

/// JWT token response
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

/// Login request
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Refresh token request
#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

/// JWT service for token operations
pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    access_token_duration: Duration,
    refresh_token_duration: Duration,
}

impl JwtService {
    /// Create a new JWT service
    pub fn new(secret: &str) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            access_token_duration: Duration::hours(8),
            refresh_token_duration: Duration::days(30),
        }
    }

    /// Generate access token
    pub fn generate_access_token(
        &self,
        user_id: &Uuid,
        email: &str,
        role: &str,
        org_id: Option<&str>,
    ) -> Result<String> {
        let now = Utc::now();
        let exp = now + self.access_token_duration;
        
        let claims = Claims {
            sub: user_id.to_string(),
            email: email.to_string(),
            role: role.to_string(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            nbf: now.timestamp(),
            jti: Uuid::new_v4().to_string(),
            org_id: org_id.map(|s| s.to_string()),
        };

        let token = encode(&Header::default(), &claims, &self.encoding_key)?;
        Ok(token)
    }

    /// Generate refresh token
    pub fn generate_refresh_token(&self, user_id: &Uuid) -> Result<String> {
        let now = Utc::now();
        let exp = now + self.refresh_token_duration;
        
        let claims = Claims {
            sub: user_id.to_string(),
            email: String::new(), // Refresh tokens don't need email
            role: "refresh".to_string(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            nbf: now.timestamp(),
            jti: Uuid::new_v4().to_string(),
            org_id: None,
        };

        let token = encode(&Header::default(), &claims, &self.encoding_key)?;
        Ok(token)
    }

    /// Validate and decode token
    pub fn validate_token(&self, token: &str) -> Result<Claims> {
        let validation = Validation::new(Algorithm::HS256);
        let token_data = decode::<Claims>(token, &self.decoding_key, &validation)?;
        Ok(token_data.claims)
    }

    /// Generate token pair (access + refresh)
    pub fn generate_token_pair(
        &self,
        user_id: &Uuid,
        email: &str,
        role: &str,
        org_id: Option<&str>,
    ) -> Result<TokenResponse> {
        let access_token = self.generate_access_token(user_id, email, role, org_id)?;
        let refresh_token = self.generate_refresh_token(user_id)?;
        
        Ok(TokenResponse {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.access_token_duration.num_seconds(),
        })
    }

    /// Refresh access token using refresh token
    pub async fn refresh_access_token(
        &self,
        refresh_token: &str,
        db: &crate::database::DatabaseService,
    ) -> Result<TokenResponse> {
        // Validate refresh token
        let claims = self.validate_token(refresh_token)?;
        
        if claims.role != "refresh" {
            return Err(anyhow::anyhow!("Invalid refresh token"));
        }
        
        // Get user from database
        let user_id = Uuid::parse_str(&claims.sub)?;
        let user = db.get_user_by_id(user_id).await?
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;
        
        // Generate new token pair
        self.generate_token_pair(
            &user.id,
            &user.email.unwrap_or_default(),
            &user.role,
            None, // TODO: Get org_id from user
        )
    }
}

/// Authentication middleware extractor
pub struct AuthUser {
    pub user_id: Uuid,
    pub email: String,
    pub role: String,
    pub org_id: Option<String>,
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Extract Authorization header
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .ok_or(AuthError::MissingToken)?;

        // Check Bearer prefix
        if !auth_header.starts_with("Bearer ") {
            return Err(AuthError::InvalidToken);
        }

        let token = &auth_header[7..]; // Skip "Bearer "
        
        // Get JWT secret from environment or config
        let jwt_secret = std::env::var("JWT_SECRET")
            .unwrap_or_else(|_| "your-secret-key".to_string());
        
        let jwt_service = JwtService::new(&jwt_secret);
        
        // Validate token
        let claims = jwt_service
            .validate_token(token)
            .map_err(|_| AuthError::InvalidToken)?;
        
        // Check expiration
        let now = Utc::now().timestamp();
        if claims.exp < now {
            return Err(AuthError::TokenExpired);
        }
        
        // Parse user ID
        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| AuthError::InvalidToken)?;
        
        Ok(AuthUser {
            user_id,
            email: claims.email,
            role: claims.role,
            org_id: claims.org_id,
        })
    }
}

/// Authentication error types
#[derive(Debug)]
pub enum AuthError {
    MissingToken,
    InvalidToken,
    TokenExpired,
    Unauthorized,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::MissingToken => (StatusCode::UNAUTHORIZED, "Missing authentication token"),
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid authentication token"),
            AuthError::TokenExpired => (StatusCode::UNAUTHORIZED, "Authentication token expired"),
            AuthError::Unauthorized => (StatusCode::FORBIDDEN, "Unauthorized access"),
        };
        
        (status, Json(serde_json::json!({
            "error": message
        }))).into_response()
    }
}

/// Role-based access control guard
pub fn require_role(user_role: &str, required_roles: &[&str]) -> Result<(), AuthError> {
    if required_roles.contains(&user_role) {
        Ok(())
    } else {
        Err(AuthError::Unauthorized)
    }
}

/// Authentication endpoints
pub mod endpoints {
    use super::*;
    use crate::database::DatabaseService;
    use argon2::{Argon2, PasswordHash, PasswordVerifier};
    use axum::extract::State;
    
    /// Login endpoint
    pub async fn login(
        State(app_state): State<crate::AppState>,
        Json(request): Json<LoginRequest>,
    ) -> Result<Json<TokenResponse>, AuthError> {
        // Get user from database
        let user = app_state.db
            .get_user_by_username(&request.username)
            .await
            .map_err(|_| AuthError::InvalidToken)?
            .ok_or(AuthError::InvalidToken)?;
        
        // Verify password
        if let Some(password_hash) = &user.password_hash {
            let parsed_hash = PasswordHash::new(password_hash)
                .map_err(|_| AuthError::InvalidToken)?;
            
            Argon2::default()
                .verify_password(request.password.as_bytes(), &parsed_hash)
                .map_err(|_| AuthError::InvalidToken)?;
        } else {
            return Err(AuthError::InvalidToken);
        }
        
        // Update last login
        let _ = app_state.db.update_user_last_login(user.id).await;
        
        // Generate tokens
        let jwt_service = JwtService::new(&app_state.config.jwt_secret);
        let tokens = jwt_service
            .generate_token_pair(
                &user.id,
                &user.email.unwrap_or_default(),
                &user.role,
                None,
            )
            .map_err(|_| AuthError::InvalidToken)?;
        
        Ok(Json(tokens))
    }
    
    /// Refresh token endpoint
    pub async fn refresh(
        State(app_state): State<crate::AppState>,
        Json(request): Json<RefreshRequest>,
    ) -> Result<Json<TokenResponse>, AuthError> {
        let jwt_service = JwtService::new(&app_state.config.jwt_secret);
        
        let tokens = jwt_service
            .refresh_access_token(&request.refresh_token, &app_state.db)
            .await
            .map_err(|_| AuthError::InvalidToken)?;
        
        Ok(Json(tokens))
    }
    
    /// Logout endpoint (for token blacklisting if implemented)
    pub async fn logout(
        AuthUser { user_id, .. }: AuthUser,
        State(_app_state): State<crate::AppState>,
    ) -> impl IntoResponse {
        // TODO: Implement token blacklisting
        Json(serde_json::json!({
            "message": "Logged out successfully",
            "user_id": user_id
        }))
    }
    
    /// Get current user info
    pub async fn me(
        AuthUser { user_id, email, role, org_id }: AuthUser,
    ) -> impl IntoResponse {
        Json(serde_json::json!({
            "user_id": user_id,
            "email": email,
            "role": role,
            "organization_id": org_id
        }))
    }
}