# Docker Setup for AAEQ

This guide explains how to run AAEQ using Docker and Docker Compose for easy deployment and testing.

## Prerequisites

- Docker Engine 20.10+ or Docker Desktop
- Docker Compose v2.0+
- X11 server (for GUI display on Linux)
- WiiM/LinkPlay devices on your local network

## Quick Start

### 1. Build and Run

```bash
# Create required directories
mkdir -p docker-data/data docker-data/config

# Build and start the container
docker-compose up --build
```

### 2. Run in Background

```bash
# Start in detached mode
docker-compose up -d

# View logs
docker-compose logs -f aaeq

# Stop the container
docker-compose down
```

## Architecture

The Docker setup consists of:

- **Multi-stage build**: Optimized image size using separate build and runtime stages
- **Host networking**: Required for mDNS/SSDP device discovery on local network
- **Persistent storage**: SQLite database and config stored in volumes
- **X11 forwarding**: GUI display support on Linux hosts

## Configuration

### Environment Variables

Set these in your shell or create a `.env` file:

```bash
# Display settings (Linux)
export DISPLAY=:0

# User ID mapping (to match host permissions)
export UID=$(id -u)
export GID=$(id -g)

# Logging level
export RUST_LOG=info,aaeq=debug

# Database path (inside container)
export AAEQ_DB_PATH=/app/data/aaeq.db
```

### Custom Configuration File

To use a custom config file:

1. Create `config.toml` in `docker-data/config/`
2. Uncomment the config volume mount in `docker-compose.yml`

Example `config.toml`:

```toml
[app]
log_level = "info"
poll_interval_ms = 1000
auto_start = true
default_preset = "Flat"

[device.wiim]
debounce_ms = 300
```

## Platform-Specific Setup

### Linux

The default setup works on Linux with X11:

```bash
# Allow Docker to connect to X server
xhost +local:docker

# Run the application
docker-compose up
```

### macOS

For GUI on macOS, you need XQuartz:

```bash
# Install XQuartz
brew install --cask xquartz

# Start XQuartz and enable remote connections
open -a XQuartz
# In XQuartz preferences: Security -> Enable "Allow connections from network clients"

# Set DISPLAY
export DISPLAY=host.docker.internal:0

# Run the application
docker-compose up
```

### Windows

For Windows, you need an X server like VcXsrv or Xming:

```powershell
# Install VcXsrv via Chocolatey
choco install vcxsrv

# Start VcXsrv with "Disable access control" option

# In PowerShell, set DISPLAY
$env:DISPLAY = "host.docker.internal:0"

# Run the application
docker-compose up
```

## Networking

### Host Network Mode

The container uses `network_mode: host` to:
- Discover WiiM devices via mDNS/SSDP
- Communicate with devices on the local network
- Avoid NAT complexities

**Note**: On macOS and Windows, `host` network mode doesn't work the same way as Linux. You may need to use bridge networking and expose specific ports.

### Alternative: Bridge Network (macOS/Windows)

If you need bridge networking:

1. Edit `docker-compose.yml`:
```yaml
services:
  aaeq:
    # Remove: network_mode: host
    ports:
      - "8080:8080"  # Add any needed ports
    networks:
      - aaeq-net

networks:
  aaeq-net:
    driver: bridge
```

2. Configure WiiM devices with static IPs or DNS names

## Data Persistence

Data is stored in local directories mapped to Docker volumes:

- `./docker-data/data/` - SQLite database
- `./docker-data/config/` - Configuration files

### Backup

```bash
# Backup database
cp docker-data/data/aaeq.db docker-data/data/aaeq.db.backup

# Or use tar
tar -czf aaeq-backup-$(date +%Y%m%d).tar.gz docker-data/
```

### Restore

```bash
# Restore from backup
cp docker-data/data/aaeq.db.backup docker-data/data/aaeq.db

# Restart container
docker-compose restart
```

## Development

### Development Compose File

Create `docker-compose.dev.yml` for development with hot reload:

```yaml
version: '3.8'

services:
  aaeq:
    build:
      context: .
      dockerfile: Dockerfile
      target: builder  # Stop at builder stage
    volumes:
      # Mount source code for live editing
      - ./crates:/app/crates
      - ./apps:/app/apps
      - ./Cargo.toml:/app/Cargo.toml
      - ./Cargo.lock:/app/Cargo.lock
      # Cache cargo dependencies
      - cargo-cache:/usr/local/cargo/registry
      - target-cache:/app/target
    command: cargo run --bin aaeq-desktop

volumes:
  cargo-cache:
  target-cache:
```

Run with:

```bash
docker-compose -f docker-compose.yml -f docker-compose.dev.yml up
```

### Building Locally

```bash
# Build the image
docker build -t aaeq:latest .

# Run without compose
docker run -it --rm \
  --network host \
  -e DISPLAY=$DISPLAY \
  -v /tmp/.X11-unix:/tmp/.X11-unix \
  -v $(pwd)/docker-data/data:/app/data \
  aaeq:latest
```

## Troubleshooting

### GUI not displaying

```bash
# Linux: Check X11 permissions
xhost +local:docker

# Check DISPLAY variable
echo $DISPLAY

# Verify X11 socket exists
ls -la /tmp/.X11-unix/

# Test with simple X app
docker run --rm -e DISPLAY=$DISPLAY -v /tmp/.X11-unix:/tmp/.X11-unix alpine sh -c "apk add --no-cache xeyes && xeyes"
```

### Cannot find WiiM devices

```bash
# Verify network mode
docker inspect aaeq-desktop | grep NetworkMode

# Test network connectivity from container
docker exec aaeq-desktop ping <wiim-device-ip>

# Check if mDNS is working
docker exec aaeq-desktop nslookup <device-name>.local
```

### Permission denied on data directory

```bash
# Fix permissions
sudo chown -R $(id -u):$(id -g) docker-data/

# Or run with root (not recommended)
docker-compose run --user root aaeq
```

### Container won't start

```bash
# Check logs
docker-compose logs aaeq

# Verify build
docker-compose build --no-cache

# Check system resources
docker stats
```

### Database locked errors

```bash
# Stop all containers
docker-compose down

# Check for stale lock files
ls -la docker-data/data/

# Remove lock file if present
rm docker-data/data/aaeq.db-*

# Restart
docker-compose up
```

## Performance Optimization

### Build Optimizations

```dockerfile
# Use BuildKit for faster builds
export DOCKER_BUILDKIT=1
docker-compose build
```

### Reduce Image Size

The multi-stage build already optimizes size, but you can further reduce:

```bash
# Check image size
docker images aaeq

# Clean up build cache
docker builder prune
```

### Memory and CPU Limits

Add to `docker-compose.yml`:

```yaml
services:
  aaeq:
    deploy:
      resources:
        limits:
          cpus: '2.0'
          memory: 1G
        reservations:
          cpus: '0.5'
          memory: 256M
```

## Testing

### Health Checks

The container includes a health check:

```bash
# Check health status
docker inspect aaeq-desktop | grep -A 10 Health

# Manual health check
docker exec aaeq-desktop pgrep -f aaeq-desktop
```

### Integration Testing

```bash
# Run with test configuration
docker-compose -f docker-compose.yml -f docker-compose.test.yml up

# Execute tests inside container
docker exec aaeq-desktop cargo test
```

## Production Deployment

For production use:

1. Use specific version tags instead of `latest`
2. Enable restart policies: `restart: always`
3. Set up proper logging with log rotation
4. Configure monitoring and alerting
5. Use secrets management for sensitive data
6. Regular automated backups of database

Example production `docker-compose.prod.yml`:

```yaml
version: '3.8'

services:
  aaeq:
    image: aaeq:1.0.0
    restart: always
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"
    healthcheck:
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 30s
```

## Additional Resources

- [AAEQ Documentation](./README.md)
- [Quick Start Guide](./QUICKSTART.md)
- [Development Guide](./DEVELOPMENT.md)
- [WiiM API Reference](./WIIM_API_REFERENCE.md)

## Support

For issues with Docker setup:
1. Check logs: `docker-compose logs -f`
2. Verify prerequisites are installed
3. Review this documentation
4. Open an issue on GitHub with logs and system info
