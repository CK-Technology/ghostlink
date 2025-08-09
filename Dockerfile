# GhostLink Server Dockerfile
# Multi-stage build for optimized production image with embedded Leptos frontend

FROM rust:1.75-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install cargo-leptos for building the web frontend
RUN cargo install cargo-leptos --locked

# Create app directory
WORKDIR /app

# Copy workspace configuration
COPY Cargo.toml Cargo.lock ./
COPY server ./server
COPY client ./client

# Build the server with embedded Leptos frontend
WORKDIR /app/server
RUN cargo leptos build --release

# Runtime stage - use distroless for security
FROM gcr.io/distroless/cc-debian12

# Copy binary and assets from builder
COPY --from=builder /app/server/target/release/ghostlink-server /app/
COPY --from=builder /app/server/target/site /app/target/site

# Copy configuration template
COPY config.toml.example /app/config.toml

WORKDIR /app

# Expose port 8443 (GhostLink server)
EXPOSE 8443

# Start the server
ENTRYPOINT ["./ghostlink-server"]
