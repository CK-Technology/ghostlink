use clap::{Parser, Subcommand};
use tracing::{info, warn, error};

mod error;
mod agent;
mod capture;
mod config;
mod connection;
mod service;
mod session;
mod input;

mod toolbox;

use error::Result;

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
    
    /// Launch session window (called by web GUI)
    Session {
        /// Session ID to connect to
        #[arg(short, long)]
        session_id: String,
        
        /// Server URL
        #[arg(short = 'u', long, default_value = "wss://relay.cktechx.com")]
        server_url: String,
        
        /// Authentication token
        #[arg(short, long)]
        token: String,
    },
    
    /// Manage toolbox (add/remove tools)
    Toolbox {
        #[command(subcommand)]
        action: ToolboxAction,
    },
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
        
        Commands::Session { session_id, server_url, token } => {
            info!("🖥️ Launching session window: {}", session_id);
            launch_session_window(session_id, server_url, token).await?;
        }
        
        Commands::Toolbox { action } => {
            handle_toolbox_action(action).await?;
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

#[derive(Subcommand)]
enum ToolboxAction {
    /// List all available tools
    List {
        /// Filter by category
        #[arg(short, long)]
        category: Option<String>,
    },
    
    /// Add a new tool
    Add {
        /// Tool name
        name: String,
        /// Command to execute
        command: String,
        /// Tool description
        #[arg(short, long)]
        description: Option<String>,
    },
    
    /// Remove a tool
    Remove {
        /// Tool name or ID
        tool: String,
    },
    
    /// Execute a tool (used internally by session window)
    Run {
        /// Tool name or ID
        tool: String,
        /// Arguments to pass to the tool
        #[arg(last = true)]
        args: Vec<String>,
    },
}

async fn launch_session_window(session_id: String, server_url: String, token: String) -> Result<()> {
    info!("Launching session window for {} via {}", session_id, server_url);
    
    // Initialize toolbox for this session
    use crate::toolbox::{ToolboxManager, ToolboxConfig};
    use crate::session::SessionWindow;
    
    let toolbox_config = ToolboxConfig::default();
    let toolbox = ToolboxManager::new(toolbox_config).await?;
    
    // Create ScreenConnect-style session window
    let session_window = SessionWindow::new(session_id.clone(), server_url.clone(), token.clone(), toolbox).await?;
    
    info!("Session window created with tabs: Start, General, Timeline, Messages, Commands, Notes");
    
    #[cfg(feature = "viewer")]
    {
        // Launch desktop window with tabbed interface + toolbox
        launch_tabbed_session_window(session_window).await?;
    }
    
    #[cfg(not(feature = "viewer"))]
    {
        // Console mode - show session info and demonstrate functionality
        info!("GUI not available, session window requires 'viewer' feature");
        
        // Demonstrate session window capabilities
        session_window.send_message("Session started from console".to_string(), true).await?;
        session_window.add_note("Console session example".to_string(), "System".to_string(), false).await?;
        
        let summary = session_window.get_session_summary().await;
        for (key, value) in summary {
            println!("{}: {}", key, value);
        }
        
        // Keep session alive
        tokio::signal::ctrl_c().await.unwrap();
        info!("Session terminated by user");
    }
    
    Ok(())
}

#[cfg(feature = "viewer")]
async fn launch_tabbed_session_window(
    mut session_window: crate::session::SessionWindow
) -> Result<()> {
    use crate::session::SessionTab;
    
    info!("Launching ScreenConnect-style tabbed session window");
    
    // Demonstrate tab functionality
    session_window.switch_tab(SessionTab::General).await;
    info!("Switched to General tab");
    
    session_window.switch_tab(SessionTab::Commands).await;
    info!("Switched to Commands tab");
    
    // Execute example command with ScreenConnect-style modifiers
    let cmd_result = session_window.execute_command(
        "systeminfo".to_string(),
        Some(30), // #timeout 30
        Some(16384), // #maxlength 16384
        Some("cmd".to_string())
    ).await?;
    
    info!("Command executed: {} ({}ms)", cmd_result.command, cmd_result.duration_ms);
    
    // Enable backstage mode (like ScreenConnect)
    session_window.enable_backstage_mode().await?;
    info!("Backstage mode enabled - user won't see remote control");
    
    // Launch a tool from toolbox
    session_window.launch_tool("System Info".to_string(), vec![]).await?;
    
    // Add session note
    session_window.add_note(
        "Troubleshooting user's system - investigating slow performance".to_string(),
        "Technician".to_string(),
        false
    ).await?;
    
    // Switch to Timeline to show activity
    session_window.switch_tab(SessionTab::Timeline).await;
    info!("Session timeline updated with all activities");
    
    // TODO: Integrate with Tauri/winit for actual GUI:
    // - Tabbed interface (Start/General/Timeline/Messages/Commands/Notes)
    // - Remote desktop display in Start tab
    // - Toolbox sidebar with double-click tool launch
    // - Control buttons (blank screen, suspend input, file transfer)
    // - Real-time command execution in Commands tab
    // - Chat in Messages tab
    
    info!("Session window ready - GUI implementation pending");
    
    // Keep window alive
    tokio::signal::ctrl_c().await.unwrap();
    info!("Session window closed");
    
    Ok(())
}

async fn handle_toolbox_action(action: ToolboxAction) -> Result<()> {
    use crate::toolbox::{ToolboxManager, ToolboxConfig, Tool, ToolCategory};
    use uuid::Uuid;
    
    let config = ToolboxConfig::default();
    let mut toolbox = ToolboxManager::new(config).await?;
    
    match action {
        ToolboxAction::List { category } => {
            let tools = if let Some(cat_str) = category {
                let category = match cat_str.to_lowercase().as_str() {
                    "system" => ToolCategory::System,
                    "network" => ToolCategory::Network,
                    "security" => ToolCategory::Security,
                    "monitoring" => ToolCategory::Monitoring,
                    "development" => ToolCategory::Development,
                    "custom" => ToolCategory::Custom,
                    _ => {
                        warn!("Unknown category: {}", cat_str);
                        return Ok(());
                    }
                };
                toolbox.list_tools_by_category(&category)
            } else {
                toolbox.list_tools()
            };
            
            if tools.is_empty() {
                println!("No tools found");
            } else {
                println!("Available tools:");
                for tool in tools {
                    println!("  {} - {} ({})", tool.name, tool.description, tool.command);
                }
            }
        }
        
        ToolboxAction::Add { name, command, description } => {
            let tool = Tool {
                id: Uuid::new_v4(),
                name: name.clone(),
                description: description.unwrap_or_else(|| format!("Custom tool: {}", name)),
                command,
                icon_path: None,
                category: ToolCategory::Custom,
                version: "1.0.0".to_string(),
                checksum: "manual".to_string(),
                is_portable: true,
                requires_admin: false,
                auto_update: false,
                server_managed: false,
            };
            
            toolbox.add_tool(tool).await?;
            info!("Added tool: {}", name);
        }
        
        ToolboxAction::Remove { tool } => {
            if let Ok(uuid) = Uuid::parse_str(&tool) {
                toolbox.remove_tool(&uuid)?;
                info!("Removed tool: {}", tool);
            } else {
                let tools = toolbox.list_tools();
                if let Some(tool_id) = tools.iter().find(|t| t.name == tool).map(|t| t.id) {
                    drop(tools); // Release the borrow
                    toolbox.remove_tool(&tool_id)?;
                    info!("Removed tool: {}", tool);
                } else {
                    warn!("Tool not found: {}", tool);
                }
            }
        }
        
        ToolboxAction::Run { tool, args } => {
            let tool_id = if let Ok(uuid) = Uuid::parse_str(&tool) {
                uuid
            } else {
                let tools = toolbox.list_tools();
                if let Some(found_tool) = tools.iter().find(|t| t.name == tool) {
                    found_tool.id
                } else {
                    warn!("Tool not found: {}", tool);
                    return Ok(());
                }
            };
            
            match toolbox.execute_tool(&tool_id, args).await {
                Ok(output) => {
                    println!("{}", output);
                }
                Err(e) => {
                    error!("Tool execution failed: {}", e);
                }
            }
        }
    }
    
    Ok(())
}
