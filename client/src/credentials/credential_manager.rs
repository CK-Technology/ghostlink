use crate::error::{GhostLinkError, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::{debug, error, info, warn};
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::{SaltString, rand_core::OsRng as ArgonOsRng}};
use base64::{Engine as _, engine::general_purpose};

/// ScreenConnect-style credential management system
pub struct CredentialManager {
    /// File path for encrypted credential storage
    storage_path: PathBuf,
    /// Master encryption key derived from user authentication
    master_key: Option<[u8; 32]>,
    /// In-memory credential cache
    credential_cache: HashMap<String, CredentialEntry>,
    /// Security configuration
    security_config: SecurityConfig,
}

/// Individual credential entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialEntry {
    /// Unique identifier
    pub id: String,
    /// Credential name/description
    pub name: String,
    /// Target system/application
    pub target: String,
    /// Credential type
    pub credential_type: CredentialType,
    /// Username
    pub username: String,
    /// Encrypted password/secret
    pub encrypted_password: Vec<u8>,
    /// Additional metadata
    pub metadata: CredentialMetadata,
    /// Access control
    pub access_control: AccessControl,
    /// Audit information
    pub audit_info: AuditInfo,
}

/// Types of credentials supported
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CredentialType {
    /// Username/password combination
    UserPassword,
    /// SSH private key
    SshPrivateKey,
    /// API key or token
    ApiKey,
    /// Certificate-based authentication
    Certificate,
    /// Multi-factor authentication codes
    MfaToken,
    /// Database connection string
    DatabaseConnection,
    /// Service account credentials
    ServiceAccount,
    /// Custom credential type
    Custom(String),
}

/// Credential metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialMetadata {
    /// Creation timestamp
    pub created_at: u64,
    /// Last modified timestamp
    pub modified_at: u64,
    /// Last used timestamp
    pub last_used_at: Option<u64>,
    /// Usage count
    pub usage_count: u64,
    /// Expiration date (if applicable)
    pub expires_at: Option<u64>,
    /// Additional tags
    pub tags: Vec<String>,
    /// Notes/description
    pub notes: String,
    /// Integration information
    pub integration: Option<IntegrationInfo>,
}

/// Integration with external systems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationInfo {
    /// Integration type (AD, Azure, LDAP, etc.)
    pub integration_type: String,
    /// External system identifier
    pub external_id: String,
    /// Sync settings
    pub sync_enabled: bool,
    /// Last sync time
    pub last_sync: Option<u64>,
}

/// Access control for credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessControl {
    /// Owner user ID
    pub owner: String,
    /// Allowed users/groups
    pub allowed_users: Vec<String>,
    /// Allowed groups
    pub allowed_groups: Vec<String>,
    /// Permission level required
    pub required_permission: PermissionLevel,
    /// Access restrictions
    pub restrictions: AccessRestrictions,
}

/// Permission levels for credential access
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum PermissionLevel {
    /// Can only view credential existence
    View,
    /// Can use credential for connections
    Use,
    /// Can modify credential details
    Modify,
    /// Full administrative access
    Admin,
}

/// Access restrictions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessRestrictions {
    /// Allowed IP addresses/ranges
    pub allowed_ips: Vec<String>,
    /// Time-based restrictions
    pub time_restrictions: Vec<TimeRestriction>,
    /// Geographic restrictions
    pub geo_restrictions: Vec<String>,
    /// Requires additional approval
    pub requires_approval: bool,
    /// Maximum usage count
    pub max_usage_count: Option<u64>,
}

/// Time-based access restrictions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRestriction {
    /// Days of week (0=Sunday, 6=Saturday)
    pub allowed_days: Vec<u8>,
    /// Start time (24-hour format, minutes from midnight)
    pub start_time: u16,
    /// End time (24-hour format, minutes from midnight)
    pub end_time: u16,
    /// Timezone
    pub timezone: String,
}

/// Audit information for compliance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditInfo {
    /// Created by user
    pub created_by: String,
    /// Last modified by user
    pub modified_by: String,
    /// Access log entries
    pub access_log: Vec<AccessLogEntry>,
    /// Change history
    pub change_history: Vec<ChangeLogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessLogEntry {
    /// Timestamp of access
    pub timestamp: u64,
    /// User who accessed
    pub user_id: String,
    /// Action performed
    pub action: String,
    /// Source IP address
    pub source_ip: String,
    /// User agent
    pub user_agent: String,
    /// Success/failure
    pub success: bool,
    /// Additional details
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeLogEntry {
    /// Timestamp of change
    pub timestamp: u64,
    /// User who made change
    pub user_id: String,
    /// Type of change
    pub change_type: String,
    /// Old values (encrypted)
    pub old_values: HashMap<String, String>,
    /// New values (encrypted)
    pub new_values: HashMap<String, String>,
    /// Reason for change
    pub reason: String,
}

/// Security configuration
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Require master password
    pub require_master_password: bool,
    /// Password complexity requirements
    pub password_complexity: PasswordComplexity,
    /// Encryption settings
    pub encryption_settings: EncryptionSettings,
    /// Session timeout
    pub session_timeout_minutes: u32,
    /// Audit logging enabled
    pub audit_logging: bool,
    /// Backup settings
    pub backup_settings: BackupSettings,
}

#[derive(Debug, Clone)]
pub struct PasswordComplexity {
    pub min_length: u8,
    pub require_uppercase: bool,
    pub require_lowercase: bool,
    pub require_numbers: bool,
    pub require_symbols: bool,
    pub max_age_days: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct EncryptionSettings {
    pub algorithm: String,
    pub key_derivation: String,
    pub key_iterations: u32,
    pub salt_length: u8,
}

#[derive(Debug, Clone)]
pub struct BackupSettings {
    pub enabled: bool,
    pub backup_directory: PathBuf,
    pub backup_interval_hours: u32,
    pub retain_backups: u32,
    pub encrypt_backups: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            require_master_password: true,
            password_complexity: PasswordComplexity {
                min_length: 12,
                require_uppercase: true,
                require_lowercase: true,
                require_numbers: true,
                require_symbols: true,
                max_age_days: Some(90),
            },
            encryption_settings: EncryptionSettings {
                algorithm: "AES-256-GCM".to_string(),
                key_derivation: "Argon2id".to_string(),
                key_iterations: 100_000,
                salt_length: 32,
            },
            session_timeout_minutes: 30,
            audit_logging: true,
            backup_settings: BackupSettings {
                enabled: true,
                backup_directory: PathBuf::from("./credential_backups"),
                backup_interval_hours: 24,
                retain_backups: 30,
                encrypt_backups: true,
            },
        }
    }
}

/// Encrypted credential storage format
#[derive(Serialize, Deserialize)]
struct CredentialStorage {
    /// Format version
    version: u32,
    /// Salt for key derivation
    salt: Vec<u8>,
    /// Nonce for encryption
    nonce: Vec<u8>,
    /// Encrypted credential data
    encrypted_data: Vec<u8>,
    /// Integrity hash
    integrity_hash: String,
    /// Metadata
    metadata: StorageMetadata,
}

#[derive(Serialize, Deserialize)]
struct StorageMetadata {
    created_at: u64,
    modified_at: u64,
    credential_count: u32,
    encryption_algorithm: String,
    key_derivation_algorithm: String,
}

impl CredentialManager {
    /// Create new credential manager
    pub fn new(storage_path: PathBuf) -> Self {
        Self {
            storage_path,
            master_key: None,
            credential_cache: HashMap::new(),
            security_config: SecurityConfig::default(),
        }
    }
    
    /// Initialize with master password
    pub async fn initialize(&mut self, master_password: &str) -> Result<()> {
        info!("Initializing credential manager");
        
        // Derive master key from password
        self.master_key = Some(self.derive_master_key(master_password).await?);
        
        // Load existing credentials if storage file exists
        if self.storage_path.exists() {
            self.load_credentials().await?;
        } else {
            info!("Creating new credential storage");
            self.save_credentials().await?;
        }
        
        info!("Credential manager initialized with {} credentials", self.credential_cache.len());
        Ok(())
    }
    
    /// Add new credential
    pub async fn add_credential(
        &mut self,
        name: String,
        target: String,
        credential_type: CredentialType,
        username: String,
        password: String,
        user_id: String,
    ) -> Result<String> {
        if self.master_key.is_none() {
            return Err(GhostLinkError::Other("Credential manager not initialized".to_string()));
        }
        
        let id = uuid::Uuid::new_v4().to_string();
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        
        // Encrypt password
        let encrypted_password = self.encrypt_data(password.as_bytes()).await?;
        
        let credential = CredentialEntry {
            id: id.clone(),
            name,
            target,
            credential_type,
            username,
            encrypted_password,
            metadata: CredentialMetadata {
                created_at: timestamp,
                modified_at: timestamp,
                last_used_at: None,
                usage_count: 0,
                expires_at: None,
                tags: Vec::new(),
                notes: String::new(),
                integration: None,
            },
            access_control: AccessControl {
                owner: user_id.clone(),
                allowed_users: vec![user_id.clone()],
                allowed_groups: Vec::new(),
                required_permission: PermissionLevel::Use,
                restrictions: AccessRestrictions {
                    allowed_ips: Vec::new(),
                    time_restrictions: Vec::new(),
                    geo_restrictions: Vec::new(),
                    requires_approval: false,
                    max_usage_count: None,
                },
            },
            audit_info: AuditInfo {
                created_by: user_id,
                modified_by: user_id.clone(),
                access_log: Vec::new(),
                change_history: Vec::new(),
            },
        };
        
        self.credential_cache.insert(id.clone(), credential);
        self.save_credentials().await?;
        
        info!("Added new credential: {} ({})", name, id);
        Ok(id)
    }
    
    /// Get credential by ID
    pub async fn get_credential(&mut self, id: &str, user_id: &str, source_ip: &str) -> Result<CredentialEntry> {
        let credential = self.credential_cache.get_mut(id)
            .ok_or_else(|| GhostLinkError::Other("Credential not found".to_string()))?;
        
        // Check access permissions
        self.check_access_permissions(credential, user_id)?;
        
        // Update usage tracking
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        credential.metadata.last_used_at = Some(timestamp);
        credential.metadata.usage_count += 1;
        
        // Log access
        credential.audit_info.access_log.push(AccessLogEntry {
            timestamp,
            user_id: user_id.to_string(),
            action: "get_credential".to_string(),
            source_ip: source_ip.to_string(),
            user_agent: "GhostLink".to_string(),
            success: true,
            details: "Credential accessed".to_string(),
        });
        
        // Save updated metadata
        self.save_credentials().await?;
        
        Ok(credential.clone())
    }
    
    /// Get decrypted password for credential
    pub async fn get_password(&mut self, id: &str, user_id: &str, source_ip: &str) -> Result<String> {
        let credential = self.get_credential(id, user_id, source_ip).await?;
        
        // Decrypt password
        let decrypted_bytes = self.decrypt_data(&credential.encrypted_password).await?;
        let password = String::from_utf8(decrypted_bytes)
            .map_err(|e| GhostLinkError::Other(format!("Invalid password encoding: {}", e)))?;
        
        info!("Password retrieved for credential: {}", credential.name);
        Ok(password)
    }
    
    /// Update credential
    pub async fn update_credential(
        &mut self,
        id: &str,
        updates: CredentialUpdate,
        user_id: &str,
    ) -> Result<()> {
        let credential = self.credential_cache.get_mut(id)
            .ok_or_else(|| GhostLinkError::Other("Credential not found".to_string()))?;
        
        // Check modify permissions
        if credential.access_control.required_permission > PermissionLevel::Modify {
            return Err(GhostLinkError::Other("Insufficient permissions to modify credential".to_string()));
        }
        
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        
        // Apply updates
        if let Some(name) = updates.name {
            credential.name = name;
        }
        if let Some(username) = updates.username {
            credential.username = username;
        }
        if let Some(password) = updates.password {
            credential.encrypted_password = self.encrypt_data(password.as_bytes()).await?;
        }
        if let Some(notes) = updates.notes {
            credential.metadata.notes = notes;
        }
        if let Some(tags) = updates.tags {
            credential.metadata.tags = tags;
        }
        
        credential.metadata.modified_at = timestamp;
        credential.audit_info.modified_by = user_id.to_string();
        
        self.save_credentials().await?;
        info!("Updated credential: {} ({})", credential.name, id);
        
        Ok(())
    }
    
    /// Delete credential
    pub async fn delete_credential(&mut self, id: &str, user_id: &str) -> Result<()> {
        let credential = self.credential_cache.get(id)
            .ok_or_else(|| GhostLinkError::Other("Credential not found".to_string()))?;
        
        // Check admin permissions
        if credential.access_control.required_permission > PermissionLevel::Admin {
            return Err(GhostLinkError::Other("Insufficient permissions to delete credential".to_string()));
        }
        
        let name = credential.name.clone();
        self.credential_cache.remove(id);
        self.save_credentials().await?;
        
        info!("Deleted credential: {} ({})", name, id);
        Ok(())
    }
    
    /// List credentials accessible to user
    pub fn list_credentials(&self, user_id: &str) -> Vec<CredentialSummary> {
        self.credential_cache
            .values()
            .filter(|cred| self.has_access_permission(cred, user_id))
            .map(|cred| CredentialSummary {
                id: cred.id.clone(),
                name: cred.name.clone(),
                target: cred.target.clone(),
                credential_type: cred.credential_type.clone(),
                username: cred.username.clone(),
                last_used: cred.metadata.last_used_at,
                usage_count: cred.metadata.usage_count,
                tags: cred.metadata.tags.clone(),
            })
            .collect()
    }
    
    /// Search credentials
    pub fn search_credentials(&self, query: &str, user_id: &str) -> Vec<CredentialSummary> {
        let query_lower = query.to_lowercase();
        self.list_credentials(user_id)
            .into_iter()
            .filter(|cred| {
                cred.name.to_lowercase().contains(&query_lower) ||
                cred.target.to_lowercase().contains(&query_lower) ||
                cred.username.to_lowercase().contains(&query_lower) ||
                cred.tags.iter().any(|tag| tag.to_lowercase().contains(&query_lower))
            })
            .collect()
    }
    
    /// Check access permissions
    fn check_access_permissions(&self, credential: &CredentialEntry, user_id: &str) -> Result<()> {
        if !self.has_access_permission(credential, user_id) {
            return Err(GhostLinkError::Other("Access denied to credential".to_string()));
        }
        
        // Check additional restrictions
        let restrictions = &credential.access_control.restrictions;
        
        // Check usage count limits
        if let Some(max_usage) = restrictions.max_usage_count {
            if credential.metadata.usage_count >= max_usage {
                return Err(GhostLinkError::Other("Credential usage limit exceeded".to_string()));
            }
        }
        
        // TODO: Implement IP, time, and geographic restrictions
        
        Ok(())
    }
    
    /// Check if user has access permission
    fn has_access_permission(&self, credential: &CredentialEntry, user_id: &str) -> bool {
        credential.access_control.owner == user_id ||
        credential.access_control.allowed_users.contains(&user_id.to_string())
        // TODO: Add group permission checks
    }
    
    /// Encrypt data using master key
    async fn encrypt_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        let master_key = self.master_key
            .ok_or_else(|| GhostLinkError::Other("Master key not available".to_string()))?;
        
        let key = Key::<Aes256Gcm>::from_slice(&master_key);
        let cipher = Aes256Gcm::new(key);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        
        let ciphertext = cipher.encrypt(&nonce, data)
            .map_err(|e| GhostLinkError::Other(format!("Encryption failed: {}", e)))?;
        
        // Prepend nonce to ciphertext
        let mut result = nonce.to_vec();
        result.extend_from_slice(&ciphertext);
        
        Ok(result)
    }
    
    /// Decrypt data using master key
    async fn decrypt_data(&self, encrypted_data: &[u8]) -> Result<Vec<u8>> {
        if encrypted_data.len() < 12 {
            return Err(GhostLinkError::Other("Invalid encrypted data length".to_string()));
        }
        
        let master_key = self.master_key
            .ok_or_else(|| GhostLinkError::Other("Master key not available".to_string()))?;
        
        let key = Key::<Aes256Gcm>::from_slice(&master_key);
        let cipher = Aes256Gcm::new(key);
        
        let (nonce_bytes, ciphertext) = encrypted_data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        
        let plaintext = cipher.decrypt(nonce, ciphertext)
            .map_err(|e| GhostLinkError::Other(format!("Decryption failed: {}", e)))?;
        
        Ok(plaintext)
    }
    
    /// Derive master key from password
    async fn derive_master_key(&self, password: &str) -> Result<[u8; 32]> {
        let salt = SaltString::generate(&mut ArgonOsRng);
        let argon2 = Argon2::default();
        
        let password_hash = argon2.hash_password(password.as_bytes(), &salt)
            .map_err(|e| GhostLinkError::Other(format!("Key derivation failed: {}", e)))?;
        
        let hash_bytes = password_hash.hash
            .ok_or_else(|| GhostLinkError::Other("Failed to extract hash".to_string()))?
            .as_bytes();
        
        if hash_bytes.len() < 32 {
            return Err(GhostLinkError::Other("Derived key too short".to_string()));
        }
        
        let mut key = [0u8; 32];
        key.copy_from_slice(&hash_bytes[..32]);
        
        Ok(key)
    }
    
    /// Load credentials from storage
    async fn load_credentials(&mut self) -> Result<()> {
        let mut file = File::open(&self.storage_path)
            .map_err(|e| GhostLinkError::Other(format!("Failed to open credential storage: {}", e)))?;
        
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)
            .map_err(|e| GhostLinkError::Other(format!("Failed to read credential storage: {}", e)))?;
        
        let storage: CredentialStorage = bincode::deserialize(&contents)
            .map_err(|e| GhostLinkError::Other(format!("Failed to deserialize credentials: {}", e)))?;
        
        // Decrypt credential data
        let mut encrypted_data = storage.nonce.clone();
        encrypted_data.extend_from_slice(&storage.encrypted_data);
        
        let decrypted_data = self.decrypt_data(&encrypted_data).await?;
        
        let credentials: HashMap<String, CredentialEntry> = bincode::deserialize(&decrypted_data)
            .map_err(|e| GhostLinkError::Other(format!("Failed to deserialize credential data: {}", e)))?;
        
        self.credential_cache = credentials;
        info!("Loaded {} credentials from storage", self.credential_cache.len());
        
        Ok(())
    }
    
    /// Save credentials to storage
    async fn save_credentials(&self) -> Result<()> {
        if self.master_key.is_none() {
            return Err(GhostLinkError::Other("Master key not available".to_string()));
        }
        
        // Serialize credentials
        let credential_data = bincode::serialize(&self.credential_cache)
            .map_err(|e| GhostLinkError::Other(format!("Failed to serialize credentials: {}", e)))?;
        
        // Encrypt credential data
        let encrypted_data = self.encrypt_data(&credential_data).await?;
        let (nonce, ciphertext) = encrypted_data.split_at(12);
        
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let storage = CredentialStorage {
            version: 1,
            salt: vec![0; 32], // TODO: Store actual salt
            nonce: nonce.to_vec(),
            encrypted_data: ciphertext.to_vec(),
            integrity_hash: "placeholder".to_string(), // TODO: Implement integrity hash
            metadata: StorageMetadata {
                created_at: timestamp,
                modified_at: timestamp,
                credential_count: self.credential_cache.len() as u32,
                encryption_algorithm: "AES-256-GCM".to_string(),
                key_derivation_algorithm: "Argon2id".to_string(),
            },
        };
        
        let serialized = bincode::serialize(&storage)
            .map_err(|e| GhostLinkError::Other(format!("Failed to serialize storage: {}", e)))?;
        
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.storage_path)
            .map_err(|e| GhostLinkError::Other(format!("Failed to create credential storage: {}", e)))?;
        
        file.write_all(&serialized)
            .map_err(|e| GhostLinkError::Other(format!("Failed to write credential storage: {}", e)))?;
        
        debug!("Saved {} credentials to storage", self.credential_cache.len());
        Ok(())
    }
}

/// Summary information for credential listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialSummary {
    pub id: String,
    pub name: String,
    pub target: String,
    pub credential_type: CredentialType,
    pub username: String,
    pub last_used: Option<u64>,
    pub usage_count: u64,
    pub tags: Vec<String>,
}

/// Update structure for credential modifications
#[derive(Debug, Default)]
pub struct CredentialUpdate {
    pub name: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub notes: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_credential_manager() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("credentials.dat");
        
        let mut manager = CredentialManager::new(storage_path);
        manager.initialize("test_password").await.unwrap();
        
        let id = manager.add_credential(
            "Test Server".to_string(),
            "server.example.com".to_string(),
            CredentialType::UserPassword,
            "testuser".to_string(),
            "testpass".to_string(),
            "user123".to_string(),
        ).await.unwrap();
        
        let credential = manager.get_credential(&id, "user123", "127.0.0.1").await.unwrap();
        assert_eq!(credential.name, "Test Server");
        
        let password = manager.get_password(&id, "user123", "127.0.0.1").await.unwrap();
        assert_eq!(password, "testpass");
    }
}