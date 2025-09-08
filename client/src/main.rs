use clap::{Parser, Subcommand};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn, error};

mod error;
mod agent;
mod capture;
mod config;
mod connection;
mod service;
mod session;
mod input;

use error::{Result, GhostLinkError};

use crate::{
    agent::Agent,
    config::ClientConfig,
    service::ServiceManager,
};

#[derive(Parser)]
#[command(name = "atlasconnect-client")]
#[command(about = "AtlasConnect Client Agent - ScreenConnect-like remote access client")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the client agent (connects to server)
    Start {
        /// Server URL to connect to
        #[arg(short, long, default_value = "wss://relay.cktechx.com")]
        server: String,
        
        /// Device name override
        #[arg(short, long)]
        name: Option<String>,
    },
    
    /// Install as system service
    Install {
        /// Server URL to connect to
        #[arg(short, long, default_value = "wss://relay.cktechx.com")]
        server: String,
    },
    
    /// Uninstall system service
    Uninstall,
    
    /// Show service status
    Status,
    
    /// Generate device info
    Info,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Start { server, name } => {
            info!("🚀 Starting AtlasConnect Client Agent");
            start_agent(server, name).await?;
        }
        
        Commands::Install { server } => {
            info!("📦 Installing AtlasConnect as system service");
            ServiceManager::install(&server)?;
            info!("✅ Service installed successfully");
        }
        
        Commands::Uninstall => {
            info!("🗑️ Uninstalling AtlasConnect service");
            ServiceManager::uninstall()?;
            info!("✅ Service uninstalled successfully");
        }
        
        Commands::Status => {
            let status = ServiceManager::status()?;
            println!("Service Status: {}", status);
        }
        
        Commands::Info => {
            show_device_info();
        }
    }

    Ok(())
}

async fn start_agent(server_url: String, device_name: Option<String>) -> Result<()> {
    let config = ClientConfig::new(server_url, device_name)?;
    
    info!("Device ID: {}", config.agent_id);
    info!("Hostname: {}", config.hostname);
    info!("Connecting to: {}", config.server_url);
    
    // Create and start the agent
    let mut agent = Agent::new(config)?;
    
    // Set up signal handling for graceful shutdown
    let shutdown_signal = tokio::spawn(async {
        tokio::signal::ctrl_c().await.unwrap();
        info!("Received shutdown signal");
    });
    
    // Start the agent with error recovery
    tokio::select! {
        result = agent.start() => {
            match result {
                Ok(()) => info!("Agent stopped normally"),
                Err(e) => error!("Agent error: {}", e),
            }
        }
        _ = shutdown_signal => {
            info!("Shutting down agent...");
            let _ = agent.shutdown().await;
        }
    }
    
    Ok(())
}

fn show_device_info() {
    use sysinfo::System;
    
    let mut sys = System::new_all();
    sys.refresh_all();
    
    println!("=== AtlasConnect Device Information ===");
    println!("Hostname: {}", System::host_name().unwrap_or_else(|| "Unknown".to_string()));
    println!("OS: {} {}", System::name().unwrap_or_else(|| "Unknown".to_string()), System::os_version().unwrap_or_else(|| "Unknown".to_string()));
    println!("Architecture: {}", System::cpu_arch().unwrap_or_else(|| "Unknown".to_string()));
    println!("Total Memory: {:.2} GB", sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0);
    println!("CPU Count: {}", sys.cpus().len());
    
    #[cfg(windows)]
    println!("Platform: Windows");
    
    #[cfg(target_os = "linux")]
    println!("Platform: Linux");
    
    #[cfg(target_os = "macos")]
    println!("Platform: macOS");
}
