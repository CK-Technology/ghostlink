# GhostLink Server Dockerfile
# Multi-stage build for optimized production image with embedded Leptos frontend
# Designed for deployment behind NGINX or Traefik proxy

FROM rust:1.75-bookworm as builder

# Install build dependencies for full GhostLink feature set
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libx11-dev \
    libxcb1-dev \
    libxcb-shm0-dev \
    libxcb-xfixes0-dev \
    libxrandr-dev \
    libxss-dev \
    libasound2-dev \
    libpulse-dev \
    libwayland-dev \
    libxkbcommon-dev \
    libglib2.0-dev \
    libgtk-3-dev \
    curl \
    wget \
    build-essential \
    cmake \
    && rm -rf /var/lib/apt/lists/*

# Install Node.js for frontend tooling
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*

# Install Rust tooling for Leptos
RUN cargo install trunk cargo-leptos --locked

# Create app directory
WORKDIR /app

# Copy workspace configuration
COPY Cargo.toml Cargo.lock ./
COPY server ./server
COPY client ./client
COPY shared ./shared

# Build dependencies first (for Docker layer caching)
RUN mkdir -p server/src client/src shared/src && \
    echo "fn main() {}" > server/src/main.rs && \
    echo "fn main() {}" > client/src/main.rs && \
    echo "// lib" > shared/src/lib.rs && \
    cargo build --release --workspace

# Remove dummy files and copy real source
RUN rm -rf server/src client/src shared/src
COPY server/src ./server/src
COPY client/src ./client/src
COPY shared/src ./shared/src

# Build the complete GhostLink server with Leptos frontend
WORKDIR /app/server
RUN cargo leptos build --release

# Runtime stage with necessary tools for PAM, terminal, and system integration
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libx11-6 \
    libxcb1 \
    libxcb-shm0 \
    libxcb-xfixes0 \
    libxrandr2 \
    libxss1 \
    libasound2 \
    libpulse0 \
    libwayland-client0 \
    libxkbcommon0 \
    curl \
    wget \
    sudo \
    openssh-client \
    wireguard-tools \
    iproute2 \
    iptables \
    iputils-ping \
    net-tools \
    procps \
    psmisc \
    && rm -rf /var/lib/apt/lists/*

# Create system user for security
RUN useradd -m -u 1001 -s /bin/bash ghostlink && \
    usermod -aG sudo ghostlink && \
    echo "ghostlink ALL=(ALL) NOPASSWD:ALL" > /etc/sudoers.d/ghostlink

# Create application directories
RUN mkdir -p /app/assets /app/toolbox /app/logs /app/config /app/data && \
    chown -R ghostlink:ghostlink /app

# Copy built application
COPY --from=builder --chown=ghostlink:ghostlink /app/server/target/release/ghostlink-server /usr/local/bin/
COPY --from=builder --chown=ghostlink:ghostlink /app/server/target/site /app/site

# Copy application assets and toolbox
COPY --chown=ghostlink:ghostlink assets/ /app/assets/
COPY --chown=ghostlink:ghostlink config.toml.example /app/config/config.toml

# Create toolbox directory with common tools
RUN mkdir -p /app/toolbox/{sysinternals,nirsoft,custom} && \
    chown -R ghostlink:ghostlink /app/toolbox

# Set executable permissions
RUN chmod +x /usr/local/bin/ghostlink-server

# Switch to non-root user
USER ghostlink

# Set working directory
WORKDIR /app

# Environment variables for container deployment
ENV RUST_LOG=info \
    LEPTOS_SITE_ROOT=/app/site \
    LEPTOS_SITE_ADDR=0.0.0.0:3000 \
    GHOSTLINK_ASSETS_PATH=/app/assets \
    GHOSTLINK_TOOLBOX_PATH=/app/toolbox \
    GHOSTLINK_CONFIG_PATH=/app/config/config.toml \
    GHOSTLINK_DATA_PATH=/app/data \
    GHOSTLINK_LOGS_PATH=/app/logs

# Health check endpoint
HEALTHCHECK --interval=30s --timeout=10s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:3000/api/health || exit 1

# Expose application port (not HTTPS - let proxy handle TLS termination)
EXPOSE 3000

# Volume for persistent data
VOLUME ["/app/data", "/app/logs", "/app/toolbox/custom"]

# Start the GhostLink server
ENTRYPOINT ["ghostlink-server"]
