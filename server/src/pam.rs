use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use tracing::{info, warn};

use crate::AppState;

/// Privileged Access Management system for elevation requests and logging
pub struct PamManager {
    /// Active elevation requests
    elevation_requests: Arc<RwLock<HashMap<Uuid, ElevationRequest>>>,
    /// Elevation history and audit log
    audit_log: Arc<RwLock<Vec<ElevationAuditEntry>>>,
    /// Active elevated sessions
    elevated_sessions: Arc<RwLock<HashMap<Uuid, ElevatedSession>>>,
    /// PAM configuration
    config: Arc<RwLock<PamConfig>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElevationRequest {
    pub id: Uuid,
    pub session_id: Uuid,
    pub user_id: String,
    pub user_domain: Option<String>,
    pub requested_by: String,
    pub reason: String,
    pub target_process: Option<String>,
    pub target_command: Option<String>,
    pub elevation_type: ElevationType,
    pub status: ElevationStatus,
    pub requested_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub approved_by: Option<String>,
    pub approved_at: Option<chrono::DateTime<chrono::Utc>>,
    pub denied_reason: Option<String>,
    pub auto_approved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ElevationType {
    /// Run as Administrator
    RunAsAdmin,
    /// Specific user impersonation
    RunAsUser(String),
    /// Service account elevation
    RunAsService(String),
    /// SYSTEM level access
    RunAsSystem,
    /// Domain administrator
    DomainAdmin,
    /// Local administrator
    LocalAdmin,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ElevationStatus {
    Pending,
    Approved,
    Denied,
    Expired,
    Active,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElevatedSession {
    pub session_id: Uuid,
    pub elevation_request_id: Uuid,
    pub user_id: String,
    pub elevated_user: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub processes: Vec<ElevatedProcess>,
    pub commands_executed: Vec<CommandExecution>,
    pub activity_log: Vec<ActivityEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElevatedProcess {
    pub pid: u32,
    pub process_name: String,
    pub command_line: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub ended_at: Option<chrono::DateTime<chrono::Utc>>,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandExecution {
    pub id: Uuid,
    pub command: String,
    pub working_directory: String,
    pub executed_at: chrono::DateTime<chrono::Utc>,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub activity_type: ActivityType,
    pub description: String,
    pub risk_level: RiskLevel,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActivityType {
    ProcessStart,
    ProcessEnd,
    CommandExecution,
    FileAccess,
    RegistryAccess,
    NetworkConnection,
    ServiceManipulation,
    UserAccountChange,
    SecurityPolicyChange,
    SystemConfiguration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElevationAuditEntry {
    pub id: Uuid,
    pub elevation_request_id: Uuid,
    pub session_id: Uuid,
    pub user_id: String,
    pub action: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub ip_address: String,
    pub user_agent: Option<String>,
    pub risk_score: u32,
    pub compliance_flags: Vec<String>,
    pub details: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PamConfig {
    pub auto_approve_threshold_minutes: u32,
    pub max_elevation_duration_hours: u32,
    pub require_justification: bool,
    pub require_approval_for_admin: bool,
    pub require_approval_for_system: bool,
    pub allowed_elevation_types: Vec<ElevationType>,
    pub restricted_commands: Vec<String>,
    pub high_risk_processes: Vec<String>,
    pub audit_all_commands: bool,
    pub enable_session_recording: bool,
    pub compliance_mode: ComplianceMode,
    pub notification_settings: NotificationSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComplianceMode {
    None,
    SOX,
    HIPAA,
    PCI,
    SOC2,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSettings {
    pub notify_on_elevation_request: bool,
    pub notify_on_high_risk_activity: bool,
    pub notify_on_failed_elevation: bool,
    pub email_notifications: bool,
    pub slack_webhook: Option<String>,
    pub teams_webhook: Option<String>,
}

impl PamManager {
    pub fn new() -> Self {
        Self {
            elevation_requests: Arc::new(RwLock::new(HashMap::new())),
            audit_log: Arc::new(RwLock::new(Vec::new())),
            elevated_sessions: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(Self::default_config())),
        }
    }
    
    /// Initialize PAM system
    pub async fn initialize(&self) -> Result<(), String> {
        info!("Initializing PAM (Privileged Access Management) system");
        
        // Load configuration from environment or config file
        // For now, use defaults
        
        info!("PAM system initialized successfully");
        Ok(())
    }
    
    /// Default PAM configuration
    fn default_config() -> PamConfig {
        PamConfig {
            auto_approve_threshold_minutes: 5,
            max_elevation_duration_hours: 2,
            require_justification: true,
            require_approval_for_admin: true,
            require_approval_for_system: true,
            allowed_elevation_types: vec![
                ElevationType::RunAsAdmin,
                ElevationType::LocalAdmin,
                ElevationType::RunAsService("NetworkService".to_string()),
            ],
            restricted_commands: vec![
                "format".to_string(),
                "del /f /s /q C:\\*".to_string(),
                "rm -rf /".to_string(),
                "shutdown".to_string(),
            ],
            high_risk_processes: vec![
                "cmd.exe".to_string(),
                "powershell.exe".to_string(),
                "regedit.exe".to_string(),
                "services.msc".to_string(),
            ],
            audit_all_commands: true,
            enable_session_recording: true,
            compliance_mode: ComplianceMode::SOC2,
            notification_settings: NotificationSettings {
                notify_on_elevation_request: true,
                notify_on_high_risk_activity: true,
                notify_on_failed_elevation: true,
                email_notifications: false,
                slack_webhook: None,
                teams_webhook: None,
            },
        }
    }
    
    /// Request elevation for a user
    pub async fn request_elevation(
        &self,
        session_id: Uuid,
        request: CreateElevationRequest,
    ) -> Result<ElevationRequest, String> {
        let config = self.config.read().await;
        
        // Check if elevation type is allowed
        if !config.allowed_elevation_types.iter().any(|allowed| {
            std::mem::discriminant(allowed) == std::mem::discriminant(&request.elevation_type)
        }) {
            return Err(format!("Elevation type {:?} not allowed", request.elevation_type));
        }
        
        // Check for restricted commands
        if let Some(ref command) = request.target_command {
            for restricted in &config.restricted_commands {
                if command.to_lowercase().contains(&restricted.to_lowercase()) {
                    return Err(format!("Command contains restricted pattern: {}", restricted));
                }
            }
        }
        
        let elevation_request = ElevationRequest {
            id: Uuid::new_v4(),
            session_id,
            user_id: request.user_id.clone(),
            user_domain: request.user_domain,
            requested_by: request.requested_by.clone(),
            reason: request.reason,
            target_process: request.target_process,
            target_command: request.target_command,
            elevation_type: request.elevation_type.clone(),
            status: if self.should_auto_approve(&request.elevation_type, &config).await {
                ElevationStatus::Approved
            } else {
                ElevationStatus::Pending
            },
            requested_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::minutes(config.auto_approve_threshold_minutes as i64),
            approved_by: None,
            approved_at: None,
            denied_reason: None,
            auto_approved: self.should_auto_approve(&request.elevation_type, &config).await,
        };
        
        // Store elevation request
        {
            let mut requests = self.elevation_requests.write().await;
            requests.insert(elevation_request.id, elevation_request.clone());
        }
        
        // Create audit entry
        self.create_audit_entry(
            elevation_request.id,
            session_id,
            &request.user_id,
            "elevation_requested",
            serde_json::json!({
                "elevation_type": elevation_request.elevation_type,
                "reason": elevation_request.reason,
                "auto_approved": elevation_request.auto_approved
            }),
        ).await;
        
        info!("Created elevation request {} for user {} in session {}", 
              elevation_request.id, request.user_id, session_id);
        
        Ok(elevation_request)
    }
    
    /// Check if elevation should be auto-approved
    async fn should_auto_approve(&self, elevation_type: &ElevationType, config: &PamConfig) -> bool {
        match elevation_type {
            ElevationType::RunAsAdmin | ElevationType::LocalAdmin => !config.require_approval_for_admin,
            ElevationType::RunAsSystem => !config.require_approval_for_system,
            ElevationType::DomainAdmin => false, // Always require approval for domain admin
            _ => true,
        }
    }
    
    /// Approve elevation request
    pub async fn approve_elevation(
        &self,
        request_id: Uuid,
        approver_id: String,
    ) -> Result<(), String> {
        let mut requests = self.elevation_requests.write().await;
        
        let request = requests.get_mut(&request_id)
            .ok_or_else(|| format!("Elevation request {} not found", request_id))?;
        
        if request.status != ElevationStatus::Pending {
            return Err(format!("Elevation request is not pending (status: {:?})", request.status));
        }
        
        request.status = ElevationStatus::Approved;
        request.approved_by = Some(approver_id.clone());
        request.approved_at = Some(chrono::Utc::now());
        
        // Create audit entry
        self.create_audit_entry(
            request_id,
            request.session_id,
            &request.user_id,
            "elevation_approved",
            serde_json::json!({
                "approved_by": approver_id,
                "elevation_type": request.elevation_type
            }),
        ).await;
        
        info!("Approved elevation request {} by {}", request_id, approver_id);
        Ok(())
    }
    
    /// Start elevated session
    pub async fn start_elevated_session(
        &self,
        request_id: Uuid,
    ) -> Result<ElevatedSession, String> {
        // First, get the data we need from the read lock
        let (user_id, elevation_type, session_id, status) = {
            let requests = self.elevation_requests.read().await;
            let request = requests.get(&request_id)
                .ok_or_else(|| format!("Elevation request {} not found", request_id))?;
            (
                request.user_id.clone(),
                request.elevation_type.clone(),
                request.session_id,
                request.status.clone(),
            )
        };

        if status != ElevationStatus::Approved {
            return Err(format!("Elevation request not approved (status: {:?})", status));
        }

        let config = self.config.read().await;

        let elevated_session = ElevatedSession {
            session_id: Uuid::new_v4(),
            elevation_request_id: request_id,
            user_id: user_id.clone(),
            elevated_user: self.get_elevated_user(&elevation_type),
            started_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(config.max_elevation_duration_hours as i64),
            processes: Vec::new(),
            commands_executed: Vec::new(),
            activity_log: Vec::new(),
        };
        drop(config);

        // Store elevated session
        {
            let mut sessions = self.elevated_sessions.write().await;
            sessions.insert(elevated_session.session_id, elevated_session.clone());
        }

        // Update request status
        {
            let mut requests = self.elevation_requests.write().await;
            if let Some(request) = requests.get_mut(&request_id) {
                request.status = ElevationStatus::Active;
            }
        }

        // Create audit entry
        self.create_audit_entry(
            request_id,
            session_id,
            &user_id,
            "elevated_session_started",
            serde_json::json!({
                "elevated_session_id": elevated_session.session_id,
                "elevated_user": elevated_session.elevated_user
            }),
        ).await;

        info!("Started elevated session {} for user {}",
              elevated_session.session_id, user_id);

        Ok(elevated_session)
    }
    
    /// Get elevated user based on elevation type
    fn get_elevated_user(&self, elevation_type: &ElevationType) -> String {
        match elevation_type {
            ElevationType::RunAsAdmin => "Administrator".to_string(),
            ElevationType::RunAsUser(user) => user.clone(),
            ElevationType::RunAsService(service) => service.clone(),
            ElevationType::RunAsSystem => "SYSTEM".to_string(),
            ElevationType::DomainAdmin => "Domain Administrator".to_string(),
            ElevationType::LocalAdmin => "Local Administrator".to_string(),
        }
    }
    
    /// Execute command in elevated session
    pub async fn execute_elevated_command(
        &self,
        session_id: Uuid,
        command: String,
        working_dir: Option<String>,
    ) -> Result<CommandExecution, String> {
        let mut sessions = self.elevated_sessions.write().await;
        let session = sessions.get_mut(&session_id)
            .ok_or_else(|| format!("Elevated session {} not found", session_id))?;
        
        let working_directory = working_dir.unwrap_or_else(|| std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string());
        
        let start_time = std::time::Instant::now();
        let executed_at = chrono::Utc::now();
        
        // Execute command (this would need proper elevation on Windows)
        let result = if cfg!(windows) {
            self.execute_windows_elevated(&command, &working_directory).await
        } else {
            self.execute_unix_elevated(&command, &working_directory).await
        };
        
        let duration_ms = start_time.elapsed().as_millis() as u64;
        
        let command_execution = CommandExecution {
            id: Uuid::new_v4(),
            command: command.clone(),
            working_directory,
            executed_at,
            exit_code: result.as_ref().ok().and_then(|(code, _, _)| *code),
            stdout: result.as_ref().map(|(_, stdout, _)| stdout.clone()).unwrap_or_default(),
            stderr: result.as_ref().map(|(_, _, stderr)| stderr.clone()).unwrap_or_else(|e| e.to_string()),
            duration_ms,
        };
        
        // Log command execution
        session.commands_executed.push(command_execution.clone());
        
        // Add activity entry
        let risk_level = self.assess_command_risk(&command);
        session.activity_log.push(ActivityEntry {
            timestamp: executed_at,
            activity_type: ActivityType::CommandExecution,
            description: format!("Executed command: {}", command),
            risk_level: risk_level.clone(),
            metadata: Some(serde_json::json!({
                "command": command,
                "exit_code": command_execution.exit_code,
                "duration_ms": duration_ms
            })),
        });
        
        // Create audit entry
        self.create_audit_entry(
            session.elevation_request_id,
            session_id,
            &session.user_id,
            "command_executed",
            serde_json::json!({
                "command": command,
                "exit_code": command_execution.exit_code,
                "risk_level": risk_level
            }),
        ).await;
        
        info!("Executed elevated command in session {}: {}", session_id, command);
        
        Ok(command_execution)
    }
    
    /// Execute command on Windows with elevation
    async fn execute_windows_elevated(
        &self,
        command: &str,
        working_dir: &str,
    ) -> Result<(Option<i32>, String, String), String> {
        // This would use Windows APIs for proper elevation
        // For now, simulate the execution
        warn!("Windows elevation not fully implemented - simulating command execution");
        
        let output = Command::new("cmd")
            .args(&["/C", command])
            .current_dir(working_dir)
            .output()
            .map_err(|e| format!("Failed to execute command: {}", e))?;
        
        Ok((
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
    
    /// Execute command on Unix with elevation
    async fn execute_unix_elevated(
        &self,
        command: &str,
        working_dir: &str,
    ) -> Result<(Option<i32>, String, String), String> {
        let output = Command::new("sudo")
            .args(&["-E", "sh", "-c", command])
            .current_dir(working_dir)
            .output()
            .map_err(|e| format!("Failed to execute elevated command: {}", e))?;
        
        Ok((
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
    
    /// Assess command risk level
    fn assess_command_risk(&self, command: &str) -> RiskLevel {
        let cmd_lower = command.to_lowercase();
        
        // Critical risk commands
        if cmd_lower.contains("format") ||
           cmd_lower.contains("del /f /s /q") ||
           cmd_lower.contains("rm -rf /") ||
           cmd_lower.contains("shutdown") ||
           cmd_lower.contains("reboot") {
            return RiskLevel::Critical;
        }
        
        // High risk commands
        if cmd_lower.contains("reg delete") ||
           cmd_lower.contains("net user") ||
           cmd_lower.contains("net localgroup") ||
           cmd_lower.contains("gpupdate") ||
           cmd_lower.contains("sc delete") {
            return RiskLevel::High;
        }
        
        // Medium risk commands
        if cmd_lower.contains("reg add") ||
           cmd_lower.contains("netsh") ||
           cmd_lower.contains("wmic") ||
           cmd_lower.contains("powershell") {
            return RiskLevel::Medium;
        }
        
        RiskLevel::Low
    }
    
    /// Create audit entry
    async fn create_audit_entry(
        &self,
        elevation_request_id: Uuid,
        session_id: Uuid,
        user_id: &str,
        action: &str,
        details: serde_json::Value,
    ) {
        let audit_entry = ElevationAuditEntry {
            id: Uuid::new_v4(),
            elevation_request_id,
            session_id,
            user_id: user_id.to_string(),
            action: action.to_string(),
            timestamp: chrono::Utc::now(),
            ip_address: "0.0.0.0".to_string(), // Would get from request context
            user_agent: None, // Would get from request headers
            risk_score: self.calculate_risk_score(&details),
            compliance_flags: self.get_compliance_flags(&details).await,
            details,
        };
        
        let mut audit_log = self.audit_log.write().await;
        audit_log.push(audit_entry);
    }
    
    /// Calculate risk score for audit entry
    fn calculate_risk_score(&self, details: &serde_json::Value) -> u32 {
        let mut score = 10; // Base score
        
        if let Some(elevation_type) = details.get("elevation_type") {
            score += match elevation_type.as_str() {
                Some("DomainAdmin") => 50,
                Some("RunAsSystem") => 40,
                Some("LocalAdmin") => 30,
                Some("RunAsAdmin") => 20,
                _ => 10,
            };
        }
        
        if let Some(risk_level) = details.get("risk_level") {
            score += match risk_level.as_str() {
                Some("Critical") => 40,
                Some("High") => 30,
                Some("Medium") => 20,
                Some("Low") => 10,
                _ => 0,
            };
        }
        
        score
    }
    
    /// Get compliance flags for audit entry
    async fn get_compliance_flags(&self, _details: &serde_json::Value) -> Vec<String> {
        let config = self.config.read().await;
        
        match config.compliance_mode {
            ComplianceMode::SOX => vec!["SOX-COMPLIANCE".to_string()],
            ComplianceMode::HIPAA => vec!["HIPAA-COMPLIANCE".to_string()],
            ComplianceMode::PCI => vec!["PCI-COMPLIANCE".to_string()],
            ComplianceMode::SOC2 => vec!["SOC2-COMPLIANCE".to_string()],
            ComplianceMode::Custom(ref mode) => vec![format!("{}-COMPLIANCE", mode.to_uppercase())],
            ComplianceMode::None => Vec::new(),
        }
    }
    
    /// Get elevation requests for user
    pub async fn get_user_elevation_requests(&self, user_id: &str) -> Vec<ElevationRequest> {
        let requests = self.elevation_requests.read().await;
        requests.values()
            .filter(|req| req.user_id == user_id)
            .cloned()
            .collect()
    }
    
    /// Get audit log entries
    pub async fn get_audit_log(&self, limit: Option<usize>) -> Vec<ElevationAuditEntry> {
        let audit_log = self.audit_log.read().await;
        let mut entries: Vec<_> = audit_log.clone();
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        
        if let Some(limit) = limit {
            entries.truncate(limit);
        }
        
        entries
    }
    
    /// Get PAM statistics
    pub async fn get_pam_stats(&self) -> HashMap<String, serde_json::Value> {
        let requests = self.elevation_requests.read().await;
        let sessions = self.elevated_sessions.read().await;
        let audit_log = self.audit_log.read().await;
        
        let mut stats = HashMap::new();
        stats.insert("total_requests".to_string(), serde_json::Value::Number(requests.len().into()));
        stats.insert("active_sessions".to_string(), serde_json::Value::Number(sessions.len().into()));
        stats.insert("total_audit_entries".to_string(), serde_json::Value::Number(audit_log.len().into()));
        
        // Request status breakdown
        let status_counts: HashMap<String, usize> = requests.values()
            .fold(HashMap::new(), |mut acc, req| {
                let key = format!("{:?}", req.status);
                *acc.entry(key).or_insert(0) += 1;
                acc
            });
        stats.insert("request_status_breakdown".to_string(), serde_json::to_value(status_counts).unwrap());
        
        // Elevation type breakdown
        let type_counts: HashMap<String, usize> = requests.values()
            .fold(HashMap::new(), |mut acc, req| {
                let key = format!("{:?}", req.elevation_type);
                *acc.entry(key).or_insert(0) += 1;
                acc
            });
        stats.insert("elevation_type_breakdown".to_string(), serde_json::to_value(type_counts).unwrap());
        
        stats
    }
}

/// API request types
#[derive(Debug, Deserialize)]
pub struct CreateElevationRequest {
    pub user_id: String,
    pub user_domain: Option<String>,
    pub requested_by: String,
    pub reason: String,
    pub target_process: Option<String>,
    pub target_command: Option<String>,
    pub elevation_type: ElevationType,
}

#[derive(Debug, Deserialize)]
pub struct ExecuteCommandRequest {
    pub command: String,
    pub working_directory: Option<String>,
}

/// API Handlers
/// Create elevation request
pub async fn api_request_elevation(
    State(app_state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(request): Json<CreateElevationRequest>,
) -> Response {
    match app_state.device_manager.pam_manager.request_elevation(session_id, request).await {
        Ok(elevation_request) => Json(elevation_request).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e
            }))
        ).into_response(),
    }
}

/// Approve elevation request
pub async fn api_approve_elevation(
    State(app_state): State<AppState>,
    Path(request_id): Path<Uuid>,
    Json(request): Json<serde_json::Value>,
) -> Response {
    let approver_id = match request.get("approver_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Missing approver_id"
            }))
        ).into_response(),
    };
    
    match app_state.device_manager.pam_manager.approve_elevation(request_id, approver_id).await {
        Ok(_) => Json(serde_json::json!({
            "status": "approved"
        })).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e
            }))
        ).into_response(),
    }
}

/// Start elevated session
pub async fn api_start_elevated_session(
    State(app_state): State<AppState>,
    Path(request_id): Path<Uuid>,
) -> Response {
    match app_state.device_manager.pam_manager.start_elevated_session(request_id).await {
        Ok(session) => Json(session).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e
            }))
        ).into_response(),
    }
}

/// Execute elevated command
pub async fn api_execute_elevated_command(
    State(app_state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(request): Json<ExecuteCommandRequest>,
) -> Response {
    match app_state.device_manager.pam_manager.execute_elevated_command(
        session_id,
        request.command,
        request.working_directory,
    ).await {
        Ok(execution) => Json(execution).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e
            }))
        ).into_response(),
    }
}

/// Get PAM audit log
pub async fn api_get_pam_audit_log(
    State(app_state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let limit = params.get("limit")
        .and_then(|l| l.parse::<usize>().ok());
    
    let audit_log = app_state.device_manager.pam_manager.get_audit_log(limit).await;
    Json(audit_log).into_response()
}

/// Get PAM statistics
pub async fn api_get_pam_stats(
    State(app_state): State<AppState>,
) -> impl IntoResponse {
    let stats = app_state.device_manager.pam_manager.get_pam_stats().await;
    Json(stats)
}