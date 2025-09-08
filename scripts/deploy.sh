#!/bin/bash

# GhostLink Deployment Script
# Automated deployment for Docker container environment behind NGINX proxy

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
ENV_FILE="$PROJECT_DIR/.env"

# Functions
log() {
    echo -e "${GREEN}[$(date +'%Y-%m-%d %H:%M:%S')] $1${NC}"
}

warn() {
    echo -e "${YELLOW}[$(date +'%Y-%m-%d %H:%M:%S')] WARNING: $1${NC}"
}

error() {
    echo -e "${RED}[$(date +'%Y-%m-%d %H:%M:%S')] ERROR: $1${NC}"
    exit 1
}

info() {
    echo -e "${BLUE}[$(date +'%Y-%m-%d %H:%M:%S')] INFO: $1${NC}"
}

# Check prerequisites
check_prerequisites() {
    log "Checking prerequisites..."
    
    command -v docker >/dev/null 2>&1 || error "Docker is required but not installed"
    command -v docker-compose >/dev/null 2>&1 || error "Docker Compose is required but not installed"
    
    # Check Docker daemon is running
    docker info >/dev/null 2>&1 || error "Docker daemon is not running"
    
    info "Prerequisites check passed"
}

# Setup environment file
setup_environment() {
    log "Setting up environment configuration..."
    
    if [ ! -f "$ENV_FILE" ]; then
        if [ -f "$PROJECT_DIR/.env.example" ]; then
            cp "$PROJECT_DIR/.env.example" "$ENV_FILE"
            info "Created .env file from .env.example"
            warn "Please edit .env file with your specific configuration before proceeding"
            echo ""
            echo "Key settings to configure:"
            echo "- DOMAIN=glink.cktechx.com"
            echo "- OIDC_CLIENT_ID, OIDC_CLIENT_SECRET, OIDC_TENANT_ID (Azure AD)"
            echo "- JWT_SECRET, SESSION_SECRET (generate strong random keys)"
            echo "- DB_PASSWORD, REDIS_PASSWORD (secure passwords)"
            echo ""
            read -p "Press Enter after configuring .env file to continue..."
        else
            error ".env.example file not found"
        fi
    else
        info "Environment file already exists"
    fi
    
    # Source environment variables
    source "$ENV_FILE"
}

# Generate secure secrets if needed
generate_secrets() {
    log "Checking and generating secure secrets..."
    
    # Generate JWT secret if default
    if grep -q "your-super-secret-jwt-key-change-this-in-production" "$ENV_FILE"; then
        JWT_SECRET=$(openssl rand -base64 32)
        sed -i "s/your-super-secret-jwt-key-change-this-in-production/$JWT_SECRET/" "$ENV_FILE"
        info "Generated new JWT secret"
    fi
    
    # Generate session secret if default
    if grep -q "session-secret-key-change-this" "$ENV_FILE"; then
        SESSION_SECRET=$(openssl rand -base64 32)
        sed -i "s/session-secret-key-change-this/$SESSION_SECRET/" "$ENV_FILE"
        info "Generated new session secret"
    fi
    
    info "Secrets configuration completed"
}

# Check SSL certificates
check_certificates() {
    log "Checking SSL certificates..."
    
    CERT_PATH="/etc/nginx/certs/cktechx.com"
    
    if [ -f "$CERT_PATH/fullchain.pem" ] && [ -f "$CERT_PATH/privkey.pem" ]; then
        info "SSL certificates found at $CERT_PATH"
        
        # Check certificate validity
        CERT_EXPIRY=$(openssl x509 -enddate -noout -in "$CERT_PATH/fullchain.pem" | cut -d= -f2)
        info "Certificate expires: $CERT_EXPIRY"
    else
        warn "SSL certificates not found at $CERT_PATH"
        warn "Make sure to place your certificates there before starting services"
    fi
}

# Build application
build_application() {
    log "Building GhostLink application..."
    
    cd "$PROJECT_DIR"
    
    # Pull latest base images
    docker-compose pull postgres redis nginx
    
    # Build GhostLink server
    docker-compose build ghostlink
    
    info "Application build completed"
}

# Initialize database
init_database() {
    log "Initializing database..."
    
    cd "$PROJECT_DIR"
    
    # Start database service
    docker-compose up -d postgres redis
    
    # Wait for database to be ready
    info "Waiting for database to be ready..."
    timeout=60
    while [ $timeout -gt 0 ]; do
        if docker-compose exec -T postgres pg_isready -U ghostlink -d ghostlink >/dev/null 2>&1; then
            break
        fi
        sleep 2
        timeout=$((timeout - 2))
    done
    
    if [ $timeout -le 0 ]; then
        error "Database failed to start within 60 seconds"
    fi
    
    info "Database initialization completed"
}

# Start services
start_services() {
    log "Starting GhostLink services..."
    
    cd "$PROJECT_DIR"
    
    # Determine which profiles to use
    PROFILES=""
    
    # Check for VPN configuration
    if [ -n "${TAILSCALE_AUTH_KEY:-}" ]; then
        PROFILES="$PROFILES --profile vpn"
        info "VPN profile enabled (Tailscale)"
    fi
    
    # Start all services
    if [ -n "$PROFILES" ]; then
        docker-compose $PROFILES up -d
    else
        docker-compose up -d
    fi
    
    info "Services started successfully"
}

# Verify deployment
verify_deployment() {
    log "Verifying deployment..."
    
    cd "$PROJECT_DIR"
    
    # Check service health
    sleep 10
    
    info "Checking service health..."
    
    # Check GhostLink server
    if curl -f http://localhost:3000/api/health >/dev/null 2>&1; then
        info "✓ GhostLink server is responding"
    else
        warn "✗ GhostLink server health check failed"
    fi
    
    # Check NGINX
    if curl -f -k https://localhost/health >/dev/null 2>&1; then
        info "✓ NGINX proxy is responding"
    else
        warn "✗ NGINX proxy health check failed"
    fi
    
    # Show running containers
    echo ""
    info "Running containers:"
    docker-compose ps
    
    echo ""
    info "Service logs can be viewed with:"
    info "  docker-compose logs -f ghostlink"
    info "  docker-compose logs -f nginx"
}

# Show deployment information
show_deployment_info() {
    log "Deployment completed successfully!"
    
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "🚀 GhostLink Remote Access Platform"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""
    echo "🌐 Web Interface: https://glink.cktechx.com"
    echo "🔐 Authentication: Microsoft Entra ID (Azure AD) OIDC"
    echo "📡 Client Relay: https://glink.cktechx.com/relay"
    echo ""
    echo "✨ Features Available:"
    echo "  • ScreenConnect-style web GUI with Leptos/WASM"
    echo "  • RustDesk-style direct IP connection capability"
    echo "  • ScreenConnect-style toolbox (SysInternals, NirSoft, custom tools)"
    echo "  • Connection banners and branding system"
    echo "  • PAM (Privileged Access Management) with Windows elevation"
    echo "  • ScreenConnect-style terminal interface"
    echo "  • VPN integration (Tailscale/WireGuard support)"
    echo "  • NGINX reverse proxy with OIDC authentication"
    echo ""
    echo "📊 Monitoring & Management:"
    echo "  • Health checks: https://glink.cktechx.com/api/health"
    echo "  • PAM audit logs: Available in web interface"
    echo "  • Terminal command history: Available in web interface"
    echo "  • Toolbox execution logs: Available in web interface"
    echo ""
    echo "🔧 Management Commands:"
    echo "  • View logs: docker-compose logs -f [service]"
    echo "  • Restart services: docker-compose restart"
    echo "  • Update application: docker-compose build ghostlink && docker-compose up -d"
    echo "  • Backup data: docker-compose exec postgres pg_dump ghostlink > backup.sql"
    echo ""
    echo "📝 Configuration:"
    echo "  • Environment: ./.env"
    echo "  • NGINX config: ./nginx/sites/glink.cktechx.com.conf"
    echo "  • Application config: ./docker/config.toml"
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
}

# Main deployment flow
main() {
    echo ""
    echo "🚀 GhostLink Deployment Script"
    echo "================================"
    echo ""
    
    check_prerequisites
    setup_environment
    generate_secrets
    check_certificates
    build_application
    init_database
    start_services
    verify_deployment
    show_deployment_info
    
    echo ""
    log "Deployment completed successfully! 🎉"
}

# Handle script arguments
case "${1:-deploy}" in
    "deploy")
        main
        ;;
    "stop")
        log "Stopping GhostLink services..."
        cd "$PROJECT_DIR"
        docker-compose down
        info "Services stopped"
        ;;
    "restart")
        log "Restarting GhostLink services..."
        cd "$PROJECT_DIR"
        docker-compose restart
        info "Services restarted"
        ;;
    "logs")
        cd "$PROJECT_DIR"
        docker-compose logs -f "${2:-ghostlink}"
        ;;
    "status")
        cd "$PROJECT_DIR"
        docker-compose ps
        ;;
    "update")
        log "Updating GhostLink application..."
        cd "$PROJECT_DIR"
        docker-compose build ghostlink
        docker-compose up -d ghostlink
        info "Application updated"
        ;;
    "backup")
        log "Creating database backup..."
        cd "$PROJECT_DIR"
        BACKUP_FILE="ghostlink-backup-$(date +%Y%m%d-%H%M%S).sql"
        docker-compose exec -T postgres pg_dump -U ghostlink ghostlink > "$BACKUP_FILE"
        info "Database backup saved to: $BACKUP_FILE"
        ;;
    *)
        echo "Usage: $0 {deploy|stop|restart|logs|status|update|backup}"
        echo ""
        echo "Commands:"
        echo "  deploy  - Full deployment (default)"
        echo "  stop    - Stop all services"
        echo "  restart - Restart all services"
        echo "  logs    - View service logs (specify service name as 2nd arg)"
        echo "  status  - Show service status"
        echo "  update  - Update and restart application"
        echo "  backup  - Create database backup"
        exit 1
        ;;
esac