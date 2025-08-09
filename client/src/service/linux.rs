use anyhow::Result;
use std::fs;
use std::process::Command;
use tracing::{info, error};

const SERVICE_NAME: &str = "atlasconnect-agent";
const SERVICE_DESCRIPTION: &str = "AtlasConnect Remote Access Agent";

pub fn install_service(server_url: &str) -> Result<()> {
    info!("Installing systemd service for AtlasConnect");
    
    let exe_path = std::env::current_exe()?;
    let service_content = format!(
        r#"[Unit]
Description={}
After=network.target
StartLimitIntervalSec=0

[Service]
Type=simple
Restart=always
RestartSec=5
User=root
ExecStart={} start --server {}
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
"#,
        SERVICE_DESCRIPTION,
        exe_path.display(),
        server_url
    );
    
    let service_path = format!("/etc/systemd/system/{}.service", SERVICE_NAME);
    
    // Write service file
    fs::write(&service_path, service_content)?;
    
    // Reload systemd and enable service
    run_command("systemctl", &["daemon-reload"])?;
    run_command("systemctl", &["enable", SERVICE_NAME])?;
    
    info!("Service installed at: {}", service_path);
    info!("To start the service: sudo systemctl start {}", SERVICE_NAME);
    
    Ok(())
}

pub fn uninstall_service() -> Result<()> {
    info!("Uninstalling systemd service");
    
    // Stop and disable service
    let _ = run_command("systemctl", &["stop", SERVICE_NAME]);
    let _ = run_command("systemctl", &["disable", SERVICE_NAME]);
    
    // Remove service file
    let service_path = format!("/etc/systemd/system/{}.service", SERVICE_NAME);
    if std::path::Path::new(&service_path).exists() {
        fs::remove_file(&service_path)?;
    }
    
    // Reload systemd
    run_command("systemctl", &["daemon-reload"])?;
    
    info!("Service uninstalled successfully");
    Ok(())
}

pub fn service_status() -> Result<String> {
    let output = Command::new("systemctl")
        .args(&["is-active", SERVICE_NAME])
        .output()?;
    
    if output.status.success() {
        Ok("Running".to_string())
    } else {
        let status_output = Command::new("systemctl")
            .args(&["status", SERVICE_NAME])
            .output()?;
        
        let status = String::from_utf8_lossy(&status_output.stdout);
        if status.contains("could not be found") {
            Ok("Not installed".to_string())
        } else if status.contains("inactive") {
            Ok("Stopped".to_string())
        } else {
            Ok("Unknown".to_string())
        }
    }
}

fn run_command(command: &str, args: &[&str]) -> Result<()> {
    let output = Command::new(command)
        .args(args)
        .output()?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("Command failed: {} {:?} - {}", command, args, stderr);
        return Err(anyhow::anyhow!("Command failed: {}", stderr));
    }
    
    Ok(())
}
