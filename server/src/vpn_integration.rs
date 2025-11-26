use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::AppState;

/// VPN integration manager for Tailscale and WireGuard
pub struct VpnManager {
    /// VPN configuration
    config: Arc<RwLock<VpnConfig>>,
    /// Connected VPN peers
    peers: Arc<RwLock<HashMap<String, VpnPeer>>>,
    /// VPN status
    status: Arc<RwLock<VpnStatus>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpnConfig {
    pub enabled: bool,
    pub vpn_type: VpnType,
    pub tailscale_config: Option<TailscaleConfig>,
    pub wireguard_config: Option<WireGuardConfig>,
    pub access_control: AccessControl,
    pub network_settings: NetworkSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VpnType {
    Tailscale,
    WireGuard,
    Both,  // Allow both types simultaneously
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TailscaleConfig {
    pub enabled: bool,
    pub tailnet: String,
    pub auth_key: Option<String>,
    pub hostname: Option<String>,
    pub tags: Vec<String>,
    pub accept_routes: bool,
    pub exit_node: Option<String>,
    pub ssh_enabled: bool,
    pub shields_up: bool,
    pub advertise_routes: Vec<String>,
    pub funnel_enabled: bool,
    pub serve_config: Option<TailscaleServeConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TailscaleServeConfig {
    pub enabled: bool,
    pub hostname: String,
    pub port: u16,
    pub https: bool,
    pub path: String,
    pub target_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireGuardConfig {
    pub enabled: bool,
    pub interface_name: String,
    pub private_key: String,
    pub public_key: String,
    pub listen_port: u16,
    pub address: String,
    pub dns: Vec<String>,
    pub peers: Vec<WireGuardPeer>,
    pub post_up_scripts: Vec<String>,
    pub post_down_scripts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireGuardPeer {
    pub name: String,
    pub public_key: String,
    pub endpoint: Option<String>,
    pub allowed_ips: Vec<String>,
    pub keepalive: Option<u16>,
    pub preshared_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessControl {
    pub require_vpn_for_gui: bool,
    pub require_vpn_for_sessions: bool,
    pub allowed_vpn_networks: Vec<String>,
    pub denied_networks: Vec<String>,
    pub trusted_ips: Vec<String>,
    pub rate_limiting: RateLimitConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub max_connections_per_ip: u32,
    pub max_requests_per_minute: u32,
    pub whitelist_vpn_ips: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSettings {
    pub bind_vpn_interface_only: bool,
    pub vpn_interface_name: Option<String>,
    pub fallback_to_public_ip: bool,
    pub advertise_local_services: bool,
    pub proxy_mode: ProxyMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProxyMode {
    Direct,      // Direct access only
    VpnOnly,     // VPN access only  
    Hybrid,      // VPN preferred, public fallback
    NginxProxy,  // Behind NGINX reverse proxy
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpnPeer {
    pub id: String,
    pub name: String,
    pub vpn_type: VpnType,
    pub vpn_ip: IpAddr,
    pub public_ip: Option<IpAddr>,
    pub hostname: Option<String>,
    pub tags: Vec<String>,
    pub last_seen: chrono::DateTime<chrono::Utc>,
    pub connected: bool,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub latency_ms: Option<f64>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpnStatus {
    pub tailscale_status: Option<TailscaleStatus>,
    pub wireguard_status: Option<WireGuardStatus>,
    pub vpn_ip: Option<IpAddr>,
    pub public_ip: Option<IpAddr>,
    pub connected_peers: usize,
    pub total_rx_bytes: u64,
    pub total_tx_bytes: u64,
    pub uptime_seconds: u64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TailscaleStatus {
    pub version: String,
    pub logged_in: bool,
    pub tailnet: String,
    pub self_node: TailscaleNode,
    pub health_messages: Vec<String>,
    pub magic_dns_suffix: String,
    pub cert_domains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TailscaleNode {
    pub id: String,
    pub name: String,
    pub dns_name: String,
    pub addresses: Vec<String>,
    pub endpoints: Vec<String>,
    pub relay: Option<String>,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub created: String,
    pub last_seen: String,
    pub expired: bool,
    pub key_expiry: String,
    pub machine_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireGuardStatus {
    pub interface_name: String,
    pub public_key: String,
    pub private_key: String,
    pub listen_port: u16,
    pub fwmark: Option<u32>,
    pub peers: Vec<WireGuardPeerStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireGuardPeerStatus {
    pub public_key: String,
    pub preshared_key: Option<String>,
    pub endpoint: Option<String>,
    pub allowed_ips: Vec<String>,
    pub latest_handshake: Option<String>,
    pub transfer_rx: u64,
    pub transfer_tx: u64,
    pub persistent_keepalive: Option<u16>,
}

impl VpnManager {
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(Self::default_config())),
            peers: Arc::new(RwLock::new(HashMap::new())),
            status: Arc::new(RwLock::new(VpnStatus {
                tailscale_status: None,
                wireguard_status: None,
                vpn_ip: None,
                public_ip: None,
                connected_peers: 0,
                total_rx_bytes: 0,
                total_tx_bytes: 0,
                uptime_seconds: 0,
                last_updated: chrono::Utc::now(),
            })),
        }
    }
    
    /// Default VPN configuration
    fn default_config() -> VpnConfig {
        VpnConfig {
            enabled: false,
            vpn_type: VpnType::Tailscale,
            tailscale_config: Some(TailscaleConfig {
                enabled: false,
                tailnet: "your-tailnet.ts.net".to_string(),
                auth_key: None,
                hostname: Some("ghostlink-server".to_string()),
                tags: vec!["tag:server".to_string(), "tag:ghostlink".to_string()],
                accept_routes: false,
                exit_node: None,
                ssh_enabled: true,
                shields_up: false,
                advertise_routes: vec![],
                funnel_enabled: false,
                serve_config: Some(TailscaleServeConfig {
                    enabled: false,
                    hostname: "ghostlink".to_string(),
                    port: 443,
                    https: true,
                    path: "/".to_string(),
                    target_port: 8080,
                }),
            }),
            wireguard_config: Some(WireGuardConfig {
                enabled: false,
                interface_name: "wg0".to_string(),
                private_key: "".to_string(),
                public_key: "".to_string(),
                listen_port: 51820,
                address: "10.0.0.1/24".to_string(),
                dns: vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()],
                peers: vec![],
                post_up_scripts: vec![],
                post_down_scripts: vec![],
            }),
            access_control: AccessControl {
                require_vpn_for_gui: false,
                require_vpn_for_sessions: false,
                allowed_vpn_networks: vec![
                    "100.64.0.0/10".to_string(),  // Tailscale CGNAT range
                    "10.0.0.0/8".to_string(),     // Private networks
                    "172.16.0.0/12".to_string(),
                    "192.168.0.0/16".to_string(),
                ],
                denied_networks: vec![],
                trusted_ips: vec![],
                rate_limiting: RateLimitConfig {
                    enabled: true,
                    max_connections_per_ip: 10,
                    max_requests_per_minute: 60,
                    whitelist_vpn_ips: true,
                },
            },
            network_settings: NetworkSettings {
                bind_vpn_interface_only: false,
                vpn_interface_name: None,
                fallback_to_public_ip: true,
                advertise_local_services: false,
                proxy_mode: ProxyMode::Hybrid,
            },
        }
    }
    
    /// Initialize VPN manager
    pub async fn initialize(&self) -> Result<(), String> {
        info!("Initializing VPN manager");
        
        let config = self.config.read().await.clone();
        
        if !config.enabled {
            info!("VPN integration disabled");
            return Ok(());
        }
        
        // Initialize Tailscale if configured
        if let Some(ts_config) = &config.tailscale_config {
            if ts_config.enabled {
                self.initialize_tailscale(ts_config).await?;
            }
        }
        
        // Initialize WireGuard if configured
        if let Some(wg_config) = &config.wireguard_config {
            if wg_config.enabled {
                self.initialize_wireguard(wg_config).await?;
            }
        }
        
        // Start status monitoring
        self.start_status_monitoring().await;
        
        info!("VPN manager initialized successfully");
        Ok(())
    }
    
    /// Initialize Tailscale
    async fn initialize_tailscale(&self, config: &TailscaleConfig) -> Result<(), String> {
        info!("Initializing Tailscale integration");
        
        // Check if Tailscale is installed
        if !self.is_tailscale_installed().await {
            warn!("Tailscale not found. Please install Tailscale first.");
            return Err("Tailscale not installed".to_string());
        }
        
        // Start Tailscale if not running
        if !self.is_tailscale_running().await {
            self.start_tailscale().await?;
        }
        
        // Login with auth key if provided
        if let Some(auth_key) = &config.auth_key {
            self.tailscale_login(auth_key, config).await?;
        }
        
        // Configure serve/funnel if enabled
        if let Some(serve_config) = &config.serve_config {
            if serve_config.enabled {
                self.configure_tailscale_serve(serve_config).await?;
            }
        }
        
        info!("Tailscale initialized successfully");
        Ok(())
    }
    
    /// Initialize WireGuard
    async fn initialize_wireguard(&self, config: &WireGuardConfig) -> Result<(), String> {
        info!("Initializing WireGuard integration");
        
        // Check if WireGuard is installed
        if !self.is_wireguard_installed().await {
            warn!("WireGuard not found. Please install WireGuard first.");
            return Err("WireGuard not installed".to_string());
        }
        
        // Generate config file
        self.generate_wireguard_config(config).await?;
        
        // Bring up interface
        self.start_wireguard(&config.interface_name).await?;
        
        info!("WireGuard initialized successfully");
        Ok(())
    }
    
    /// Check if Tailscale is installed
    async fn is_tailscale_installed(&self) -> bool {
        Command::new("tailscale")
            .arg("version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
    
    /// Check if Tailscale is running
    async fn is_tailscale_running(&self) -> bool {
        Command::new("tailscale")
            .arg("status")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
    
    /// Start Tailscale daemon
    async fn start_tailscale(&self) -> Result<(), String> {
        info!("Starting Tailscale daemon");
        
        let output = Command::new("sudo")
            .args(&["systemctl", "start", "tailscaled"])
            .output()
            .map_err(|e| format!("Failed to start Tailscale: {}", e))?;
        
        if !output.status.success() {
            return Err(format!("Failed to start Tailscale: {}", String::from_utf8_lossy(&output.stderr)));
        }
        
        // Wait for daemon to start
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
        
        Ok(())
    }
    
    /// Login to Tailscale with auth key
    async fn tailscale_login(&self, auth_key: &str, config: &TailscaleConfig) -> Result<(), String> {
        info!("Logging into Tailscale");
        
        let mut args = vec!["up", "--auth-key", auth_key];
        
        if let Some(hostname) = &config.hostname {
            args.extend(&["--hostname", hostname]);
        }
        
        if config.accept_routes {
            args.push("--accept-routes");
        }
        
        if config.ssh_enabled {
            args.push("--ssh");
        }
        
        if config.shields_up {
            args.push("--shields-up");
        }
        
        for route in &config.advertise_routes {
            args.extend(&["--advertise-routes", route]);
        }
        
        let output = Command::new("tailscale")
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to login to Tailscale: {}", e))?;
        
        if !output.status.success() {
            return Err(format!("Tailscale login failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        
        info!("Successfully logged into Tailscale");
        Ok(())
    }
    
    /// Configure Tailscale serve/funnel
    async fn configure_tailscale_serve(&self, config: &TailscaleServeConfig) -> Result<(), String> {
        info!("Configuring Tailscale serve on {}:{}", config.hostname, config.port);
        
        // Configure serve
        let serve_target = format!("http://127.0.0.1:{}", config.target_port);
        let serve_path = if config.https {
            format!("https://{}:{}{}", config.hostname, config.port, config.path)
        } else {
            format!("http://{}:{}{}", config.hostname, config.port, config.path)
        };
        
        let output = Command::new("tailscale")
            .args(&["serve", &serve_path, &serve_target])
            .output()
            .map_err(|e| format!("Failed to configure Tailscale serve: {}", e))?;
        
        if !output.status.success() {
            return Err(format!("Tailscale serve configuration failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        
        info!("Tailscale serve configured successfully");
        Ok(())
    }
    
    /// Check if WireGuard is installed
    async fn is_wireguard_installed(&self) -> bool {
        Command::new("wg")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
    
    /// Generate WireGuard configuration
    async fn generate_wireguard_config(&self, config: &WireGuardConfig) -> Result<(), String> {
        let config_content = self.build_wireguard_config(config);
        let config_path = format!("/etc/wireguard/{}.conf", config.interface_name);
        
        tokio::fs::write(&config_path, config_content).await
            .map_err(|e| format!("Failed to write WireGuard config: {}", e))?;
        
        info!("Generated WireGuard config at {}", config_path);
        Ok(())
    }
    
    /// Build WireGuard configuration content
    fn build_wireguard_config(&self, config: &WireGuardConfig) -> String {
        let mut content = format!(
            "[Interface]\nPrivateKey = {}\nAddress = {}\nListenPort = {}\n",
            config.private_key,
            config.address,
            config.listen_port
        );
        
        if !config.dns.is_empty() {
            content.push_str(&format!("DNS = {}\n", config.dns.join(", ")));
        }
        
        for script in &config.post_up_scripts {
            content.push_str(&format!("PostUp = {}\n", script));
        }
        
        for script in &config.post_down_scripts {
            content.push_str(&format!("PostDown = {}\n", script));
        }
        
        for peer in &config.peers {
            content.push_str(&format!(
                "\n[Peer]\n# {}\nPublicKey = {}\nAllowedIPs = {}\n",
                peer.name,
                peer.public_key,
                peer.allowed_ips.join(", ")
            ));
            
            if let Some(endpoint) = &peer.endpoint {
                content.push_str(&format!("Endpoint = {}\n", endpoint));
            }
            
            if let Some(keepalive) = peer.keepalive {
                content.push_str(&format!("PersistentKeepalive = {}\n", keepalive));
            }
            
            if let Some(psk) = &peer.preshared_key {
                content.push_str(&format!("PresharedKey = {}\n", psk));
            }
        }
        
        content
    }
    
    /// Start WireGuard interface
    async fn start_wireguard(&self, interface_name: &str) -> Result<(), String> {
        info!("Starting WireGuard interface {}", interface_name);
        
        let output = Command::new("sudo")
            .args(&["wg-quick", "up", interface_name])
            .output()
            .map_err(|e| format!("Failed to start WireGuard: {}", e))?;
        
        if !output.status.success() {
            return Err(format!("WireGuard start failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        
        info!("WireGuard interface {} started successfully", interface_name);
        Ok(())
    }
    
    /// Start status monitoring
    async fn start_status_monitoring(&self) {
        let config = self.config.clone();
        let status = self.status.clone();
        let peers = self.peers.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                let config_read = config.read().await;
                if !config_read.enabled {
                    continue;
                }
                
                // Update Tailscale status
                if let Some(ts_config) = &config_read.tailscale_config {
                    if ts_config.enabled {
                        if let Ok(ts_status) = Self::get_tailscale_status().await {
                            let mut status_write = status.write().await;
                            status_write.tailscale_status = Some(ts_status);
                            status_write.last_updated = chrono::Utc::now();
                        }
                    }
                }
                
                // Update WireGuard status
                if let Some(wg_config) = &config_read.wireguard_config {
                    if wg_config.enabled {
                        if let Ok(wg_status) = Self::get_wireguard_status(&wg_config.interface_name).await {
                            let mut status_write = status.write().await;
                            status_write.wireguard_status = Some(wg_status);
                            status_write.last_updated = chrono::Utc::now();
                        }
                    }
                }
                
                // Update peer information
                Self::update_peer_information(&peers).await;
            }
        });
    }
    
    /// Get Tailscale status
    async fn get_tailscale_status() -> Result<TailscaleStatus, String> {
        let output = Command::new("tailscale")
            .args(&["status", "--json"])
            .output()
            .map_err(|e| format!("Failed to get Tailscale status: {}", e))?;
        
        if !output.status.success() {
            return Err("Tailscale status command failed".to_string());
        }
        
        let status_json: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| format!("Failed to parse Tailscale status: {}", e))?;
        
        // Parse Tailscale status (simplified)
        Ok(TailscaleStatus {
            version: status_json["Version"].as_str().unwrap_or("unknown").to_string(),
            logged_in: status_json["BackendState"].as_str() == Some("Running"),
            tailnet: status_json["CurrentTailnet"]["Name"].as_str().unwrap_or("unknown").to_string(),
            self_node: TailscaleNode {
                id: status_json["Self"]["ID"].as_str().unwrap_or("unknown").to_string(),
                name: status_json["Self"]["DNSName"].as_str().unwrap_or("unknown").to_string(),
                dns_name: status_json["Self"]["DNSName"].as_str().unwrap_or("unknown").to_string(),
                addresses: vec![],
                endpoints: vec![],
                relay: None,
                rx_bytes: 0,
                tx_bytes: 0,
                created: "".to_string(),
                last_seen: "".to_string(),
                expired: false,
                key_expiry: "".to_string(),
                machine_status: "active".to_string(),
            },
            health_messages: vec![],
            magic_dns_suffix: ".ts.net".to_string(),
            cert_domains: vec![],
        })
    }
    
    /// Get WireGuard status
    async fn get_wireguard_status(interface_name: &str) -> Result<WireGuardStatus, String> {
        let output = Command::new("sudo")
            .args(&["wg", "show", interface_name, "dump"])
            .output()
            .map_err(|e| format!("Failed to get WireGuard status: {}", e))?;
        
        if !output.status.success() {
            return Err("WireGuard status command failed".to_string());
        }
        
        // Parse WireGuard output (simplified)
        Ok(WireGuardStatus {
            interface_name: interface_name.to_string(),
            public_key: "".to_string(),
            private_key: "".to_string(),
            listen_port: 51820,
            fwmark: None,
            peers: vec![],
        })
    }
    
    /// Update peer information
    async fn update_peer_information(_peers: &Arc<RwLock<HashMap<String, VpnPeer>>>) {
        // TODO: Implement peer discovery and status updates
    }
    
    /// Check if IP is from VPN
    pub async fn is_vpn_ip(&self, ip: &IpAddr) -> bool {
        let config = self.config.read().await;
        
        for network in &config.access_control.allowed_vpn_networks {
            if let Ok(cidr) = network.parse::<ipnetwork::IpNetwork>() {
                if cidr.contains(*ip) {
                    return true;
                }
            }
        }
        
        false
    }
    
    /// Get VPN configuration
    pub async fn get_config(&self) -> VpnConfig {
        self.config.read().await.clone()
    }
    
    /// Update VPN configuration
    pub async fn update_config(&self, new_config: VpnConfig) -> Result<(), String> {
        {
            let mut config = self.config.write().await;
            *config = new_config;
        }
        
        // Restart VPN services if needed
        self.initialize().await?;
        
        info!("VPN configuration updated");
        Ok(())
    }
    
    /// Get VPN status
    pub async fn get_status(&self) -> VpnStatus {
        self.status.read().await.clone()
    }
    
    /// Get connected peers
    pub async fn get_peers(&self) -> HashMap<String, VpnPeer> {
        self.peers.read().await.clone()
    }
}

/// API Handlers

/// Get VPN configuration
pub async fn api_get_vpn_config(
    State(app_state): State<AppState>,
) -> impl IntoResponse {
    let config = app_state.device_manager.vpn_manager.get_config().await;
    Json(config)
}

/// Update VPN configuration
pub async fn api_update_vpn_config(
    State(app_state): State<AppState>,
    Json(config): Json<VpnConfig>,
) -> Response {
    match app_state.device_manager.vpn_manager.update_config(config).await {
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

/// Get VPN status
pub async fn api_get_vpn_status(
    State(app_state): State<AppState>,
) -> impl IntoResponse {
    let status = app_state.device_manager.vpn_manager.get_status().await;
    Json(status)
}

/// Get VPN peers
pub async fn api_get_vpn_peers(
    State(app_state): State<AppState>,
) -> impl IntoResponse {
    let peers = app_state.device_manager.vpn_manager.get_peers().await;
    Json(peers)
}

/// Enable Tailscale (stub)
pub async fn api_enable_tailscale(
    State(_app_state): State<AppState>,
) -> impl IntoResponse {
    // TODO: Implement Tailscale integration
    Json(serde_json::json!({
        "status": "error",
        "message": "Tailscale integration not yet implemented"
    }))
}

/// Get WireGuard config (stub)
pub async fn api_get_wireguard_config(
    State(_app_state): State<AppState>,
) -> impl IntoResponse {
    // TODO: Implement WireGuard config generation
    Json(serde_json::json!({
        "status": "error",
        "message": "WireGuard config generation not yet implemented"
    }))
}