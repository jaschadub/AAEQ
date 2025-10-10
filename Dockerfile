# Multi-stage build for AAEQ
# Stage 1: Build the application
FROM rust:1.83-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    # Required for egui/eframe GUI
    libx11-dev \
    libxcursor-dev \
    libxrandr-dev \
    libxi-dev \
    libgl1-mesa-dev \
    libgtk-3-dev \
    libfontconfig1-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy Cargo files for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps ./apps

# Build the application in release mode
RUN cargo build --release --bin aaeq-desktop

# Stage 2: Runtime image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libsqlite3-0 \
    libssl3 \
    # GUI runtime dependencies
    libx11-6 \
    libxcursor1 \
    libxrandr2 \
    libxi6 \
    libgl1 \
    libgtk-3-0 \
    libfontconfig1 \
    # Network utilities for testing
    curl \
    iputils-ping \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 aaeq

# Set working directory
WORKDIR /app

# Copy the built binary from builder stage
COPY --from=builder /app/target/release/aaeq-desktop /usr/local/bin/aaeq-desktop

# Create directories for data persistence
RUN mkdir -p /app/data /app/config && chown -R aaeq:aaeq /app

# Switch to non-root user
USER aaeq

# Set environment variables
ENV RUST_LOG=info,aaeq=debug
ENV AAEQ_DB_PATH=/app/data/aaeq.db

# Expose any ports if needed (currently none for local operation)
# EXPOSE 8080

# Health check (optional - checks if the process is running)
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD pgrep -f aaeq-desktop || exit 1

# Default command
CMD ["aaeq-desktop"]
