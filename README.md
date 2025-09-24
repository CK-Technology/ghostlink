# GhostLink

![GhostLink Logo](assets/logo/ghostlink-logo.png)

![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-000000?logo=rust\&logoColor=white)
![Zero Trust Architecture](https://img.shields.io/badge/Zero%20Trust-Enabled-blueviolet?style=flat-square)
![End-to-End Encryption](https://img.shields.io/badge/End--to--End%20Encryption-Active-brightgreen)
![Cross-Platform](https://img.shields.io/badge/Cross--Platform-Windows%20%7C%20macOS%20%7C%20Linux-orange)
![Wayland/X11 Support](https://img.shields.io/badge/Linux%20Desktop-Wayland%20%7C%20X11-informational)
![Web Portal](https://img.shields.io/badge/Web%20Portal-Integrated-yellow)
![Enterprise Ready](https://img.shields.io/badge/Enterprise-Ready-darkblue)
![Version](https://img.shields.io/badge/Version-0.2.0--dev-blue)

## DISCLAIMER

⚠️ **EXPERIMENTAL LIBRARY - FOR LAB/PERSONAL USE** ⚠️

This is an experimental library under active development. It is
intended for research, learning, and personal projects. The API is subject
to change!

---

GhostLink is an all-in-one remote access and support platform engineered for security-driven teams, MSPs, and IT departments. Written in Rust, GhostLink merges a modern zero trust relay, a powerful web management portal, and ultra-fast cross-platform agents—all in a single, self-hosted server.

**🎉 v0.2.0 Status: Foundational components implemented and compiling successfully!**

---

## Implementation Status (v0.2.0)

### ✅ Completed
- **Core Agent Architecture**: Client agent orchestrator with session management and heartbeat system
- **Cross-Platform Screen Capture**: Enum-based abstraction supporting Wayland, X11, Windows, and macOS
- **Input Control System**: Platform-specific input handling with async trait implementations  
- **Relay Message Protocol**: WebSocket-based communication between client and server
- **Session Management**: Multi-session support with type-based routing (Console, Backstage, Adhoc)
- **Connection Management**: Robust WebSocket connection handling with reconnection logic
- **Configuration System**: Flexible TOML-based configuration for client and server
- **Hardware Encoding Support**: Framework for NVENC, QSV, and VideoToolbox (stubs implemented)

### 🔄 In Progress
- Complete capture loop implementation (lifetime management for async tasks)
- Server-side session coordination and device management
- Web portal integration with backend services
- Hardware-accelerated encoding implementations

### 📋 Upcoming
- Authentication and device registration
- Web portal UI improvements
- Database integration
- Deployment and containerization
- Testing and documentation

---

## Key Features

* **Unified Server**

  * One container or binary: Web Portal + Relay
  * Easy deployment and scaling
  * Authenticate every device and session
  * Certificate-based device identity
  * Encrypted relay communications
* **High-Performance Remote Access**

  * RustDesk-level performance with screen capture
  * WebSocket-based real-time communication
  * Hardware-accelerated encoding when available
* **ScreenConnect-Like Experience**

  * Browser-based management portal (Leptos + WASM)
  * Persistent agent/client service
  * Launch sessions directly from web interface
  * Device management and monitoring
* **Self-Hosted & Enterprise Ready**

  * Single binary deployment
  * Docker containerization
  * Nginx reverse proxy configuration
  * Database integration for user management

---

## Architecture

GhostLink follows a ScreenConnect-inspired architecture with modern performance:

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Admin Web     │────▶│  nginx Reverse  │────▶│   GhostLink     │
│   Interface     │     │     Proxy       │     │     Server      │
│atlas.cktechx.com│     │                 │     │   (Port 8443)   │
└─────────────────┘     │                 │     │                 │
                        │                 │     │  ┌─────────────┐ │
┌─────────────────┐     │                 │     │  │ Web Portal  │ │
│  Client Agents  │────▶│                 │────▶│  │   (Leptos)  │ │
│relay.cktechx.com│     │                 │     │  └─────────────┘ │
└─────────────────┘     └─────────────────┘     │  ┌─────────────┐ │
                                                │  │   Relay     │ │
                                                │  │  Component  │ │
                                                │  └─────────────┘ │
                                                └─────────────────┘
```

### Components

1. **GhostLink Server** (`server/`)
   - Embedded Leptos web frontend (WASM)
   - WebSocket relay for client connections  
   - REST API for device management
   - Session orchestration

2. **GhostLink Client** (`client/`)
   - Persistent agent/service
   - Cross-platform screen capture
   - Secure communication with server
   - Auto-reconnection and recovery

3. **Nginx Configuration**
   - `atlas.cktechx.com` → Web GUI access
   - `relay.cktechx.com` → Client relay endpoint
   - SSL termination and security headers

---

## Quick Start

### 1. Server Deployment

#### Using Docker (Recommended)
```bash
# Build the server image
docker build -t ghostlink-server .

# Run with docker-compose
docker-compose up -d
```

#### Manual Installation
```bash
# Install cargo-leptos for building web frontend
cargo install cargo-leptos

# Build the server
cd server
cargo leptos build --release

# Run the server
./target/release/ghostlink-server
```

### 2. Nginx Configuration

Copy the provided nginx configurations:

```bash
# Copy relay configuration
sudo cp nginx/relay.cktechx.com.conf /etc/nginx/sites-available/
sudo ln -s /etc/nginx/sites-available/relay.cktechx.com.conf /etc/nginx/sites-enabled/

# Copy web GUI configuration  
sudo cp nginx/atlas.cktechx.com.conf /etc/nginx/sites-available/
sudo ln -s /etc/nginx/sites-available/atlas.cktechx.com.conf /etc/nginx/sites-enabled/

# Test and reload nginx
sudo nginx -t
sudo systemctl reload nginx
```

### 3. Client Installation

#### Windows
```powershell
# Download and install as service
.\ghostlink-client.exe install --server wss://relay.cktechx.com
```

#### Linux
```bash
# Install as systemd service
sudo ./ghostlink-client install --server wss://relay.cktechx.com
```

#### Manual Mode
```bash
# Run directly (for testing)
./ghostlink-client start --server wss://relay.cktechx.com
```

---

## Usage

1. **Access Web Portal**: Navigate to `https://atlas.cktechx.com`
2. **View Connected Devices**: See all online agents in the dashboard
3. **Launch Sessions**: Click "Connect" to start remote control
4. **File Transfers**: Drag and drop files between devices
5. **Multi-User Support**: Multiple admins can manage devices

---

## Configuration

### Server Configuration (`config.toml`)

```toml
[server]
host = "0.0.0.0"
port = 8443
database_url = "sqlite:./data/ghostlink.db"

[security]
jwt_secret = "your-secret-key-here"
session_timeout = 3600
max_concurrent_sessions = 100

[relay]
heartbeat_interval = 30
connection_timeout = 300
max_clients = 1000
```

### Client Configuration

Clients auto-configure on first connection, but you can override:

```bash
# Custom device name
ghostlink-client start --name "Production-Server-01"

# Different server endpoint
ghostlink-client start --server wss://relay.example.com
```

---

## Development

### Prerequisites

- Rust 1.75+ (edition 2024)
- cargo-leptos for web frontend
- Node.js (for additional tooling)

### Building

```bash
# Install tools
cargo install cargo-leptos trunk wasm-pack

# Build server with web frontend
cd server
cargo leptos build

# Build client
cd ../client  
cargo build --release

# Run development server
cargo leptos watch
```

### Architecture Details

- **Frontend**: Leptos with WebAssembly for near-native performance
- **Backend**: Axum web framework with WebSocket support
- **Database**: SQLx with PostgreSQL/SQLite support
- **Security**: Ring cryptography, JWT authentication
- **Networking**: Tokio async runtime, rustls for TLS

---

## Security Features

* **Zero Trust Architecture**: Every connection authenticated and encrypted
* **Certificate Pinning**: Clients validate server certificates  
* **End-to-End Encryption**: Screen data encrypted in transit
* **Session Isolation**: Each remote session is cryptographically isolated
* **Audit Logging**: All connections and actions logged
* **Role-Based Access**: Admin, Operator, and Viewer roles

---

## Performance

GhostLink is designed for high performance:

- **Rust Performance**: Near-zero overhead async runtime
- **Efficient Encoding**: Hardware-accelerated screen capture when available  
- **WebAssembly Frontend**: 60fps UI with minimal bandwidth
- **Connection Pooling**: Reuse connections for multiple sessions
- **Adaptive Quality**: Dynamic adjustment based on network conditions

---

## License

Licensed under the MIT License. See `LICENSE` file for details.

---

## Support

- **Documentation**: [Coming Soon]
- **Issues**: GitHub Issues
- **Community**: [Discord/Slack - Coming Soon]
* **End-to-End Encryption**

  * All traffic and file transfers encrypted by default
* **Persistent Agents & Instant Access**

  * Lightweight agent for 24/7 unattended or on-demand remote access
* **Modern Web Portal**

  * Manage endpoints, users, sessions, RBAC, and audit logs in-browser
* **Wayland & X11 Support**

  * Native for all major Linux desktops
* **Fast File Transfer & Clipboard Sync**

  * Secure, efficient, cross-platform
* **Multi-Factor Authentication**

  * Supports MFA, SSO, OIDC, and SAML
* **Proxy Friendly**

  * Deploy behind NGINX, Traefik, or HAProxy

---

## Why GhostLink?

* **Security-First:** Zero trust, E2E encryption, full auditability
* **Enterprise & MSP Ready:** Role-based access, multi-tenant options, SSO
* **Linux-Native:** Real support for Wayland and X11 (not an afterthought)
* **Simple Deployment:** Single binary or container for full stack

---

## Deployment Overview

GhostLink Server (Web Portal + Relay) handles:

* Client relay connections
* User authentication
* Web-based management

GhostLink Agents run on endpoints (Windows, macOS, Linux).

---

## Coming Soon

* Mobile clients (iOS/Android)
* Headless Linux agent (CLI)
* Branding/white-label support
* API and webhooks

---

## Quick Start

### Development Setup

1. **Prerequisites**
   ```bash
   # Install Rust 2024 edition
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   rustup default stable

   # Install PostgreSQL
   # Ubuntu/Debian: apt install postgresql postgresql-contrib
   # macOS: brew install postgresql
   # Or use Docker: docker run --name postgres -e POSTGRES_PASSWORD=password -p 5432:5432 -d postgres:16
   ```

2. **Clone and Build**
   ```bash
   git clone <your-repo>
   cd ghostlink
   
   # Copy example config
   cp config.toml.example config.toml
   
   # Edit config.toml with your database settings
   
   # Build and run
   cargo build --release
   cargo run
   ```

3. **Access Web Portal**
   - Open http://localhost:8080 in your browser
   - WebSocket relay available at ws://localhost:8080/api/v1/relay

### Production Deployment

#### Option 1: Docker Compose (Recommended)
```bash
# Clone repository
git clone <your-repo>
cd ghostlink

# Set environment variables
export DB_PASSWORD=your-secure-password
export JWT_SECRET=your-super-secret-jwt-key

# Start services
docker-compose up -d

# Access via Nginx reverse proxy
# Web portal: http://localhost
# API: http://localhost/api/v1
```

#### Option 2: Manual Deployment with Nginx

1. **Build the server**
   ```bash
   cargo build --release
   ```

2. **Setup Nginx**
   ```bash
   # Copy Nginx configurations
   sudo cp nginx/ghostlink.conf /etc/nginx/sites-available/
   sudo cp nginx/ghostlink-common.conf /etc/nginx/conf.d/
   
   # Enable site
   sudo ln -s /etc/nginx/sites-available/ghostlink.conf /etc/nginx/sites-enabled/
   
   # Test and reload Nginx
   sudo nginx -t
   sudo systemctl reload nginx
   ```

3. **Run AtlasConnect Server**
   ```bash
   # Create systemd service (optional)
   sudo cp scripts/ghostlink.service /etc/systemd/system/
   sudo systemctl enable ghostlink
   sudo systemctl start ghostlink
   ```

### Architecture Overview

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Web Browser   │    │   GhostLink     │    │   PostgreSQL    │
│                 │    │     Server      │    │    Database     │
│  - Dashboard    │◄──►│                 │◄──►│                 │
│  - Session UI   │    │  - Web API      │    │  - Users        │
│                 │    │  - WebSocket    │    │  - Agents       │
└─────────────────┘    │  - Relay        │    │  - Sessions     │
                       └─────────────────┘    └─────────────────┘
                                │
                                │ WebSocket
                                ▼
                       ┌─────────────────┐
                       │   GhostLink     │
                       │     Agents      │
                       │                 │
                       │  - Windows      │
                       │  - macOS        │
                       │  - Linux        │
                       └─────────────────┘
```

### Client Development

The native clients will connect to the WebSocket relay endpoint and implement:

- **Agent Registration**: Register with server and maintain heartbeat
- **Screen Capture**: Efficient screen capture and compression
- **Input Handling**: Mouse/keyboard input relay
- **File Transfer**: Secure file transfer capabilities
- **Session Management**: Handle session requests from web portal

### API Documentation

#### REST API Endpoints

- `POST /api/v1/auth/login` - User authentication
- `GET /api/v1/agents` - List connected agents
- `POST /api/v1/sessions` - Create new session
- `GET /api/v1/status` - Server status

#### WebSocket Messages

- `Authenticate` - Client authentication
- `AgentRegister` - Agent registration
- `SessionRequest` - Request new session
- `ScreenFrame` - Screen capture data
- `ScreenControl` - Input events

### Configuration

Key configuration options in `config.toml`:

```toml
[server]
host = "127.0.0.1"
port = 8080

[database]
url = "postgresql://user:pass@localhost/ghostlink"

[security]
jwt_secret = "your-secret-key"
session_timeout_minutes = 480

[relay]
max_relay_connections = 1000
heartbeat_interval_seconds = 30
```

### Security Features

- **Zero Trust Architecture**: Every connection authenticated and encrypted
- **End-to-End Encryption**: All session data encrypted with unique keys
- **JWT Authentication**: Secure token-based authentication
- **Role-Based Access Control**: Granular permissions system
- **Audit Logging**: Complete audit trail of all activities
- **Rate Limiting**: Protection against abuse via Nginx

---

> GhostLink — Secure, modern remote access. Built for the era of zero trust.

