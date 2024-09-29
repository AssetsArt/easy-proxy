#!/bin/bash

set -e

# Configuration
REPO="AssetsArt/easy-proxy"  # Replace with your GitHub username/repo
BINARY="easy-proxy"
INSTALL_DIR="/usr/local/bin"

# Detect OS
OS=$(uname | tr '[:upper:]' '[:lower:]')

# Only proceed if OS is Linux
if [[ "$OS" != "linux" ]]; then
    echo "This install script currently supports Linux only."
    exit 1
fi

# Detect architecture
ARCH=$(uname -m)
case $ARCH in
    x86_64)
        ARCH="x86_64"
        ;;
    arm64 | aarch64)
        ARCH="aarch64"
        ;;
    *)
        echo "Unsupported architecture: $ARCH"
        exit 1
        ;;
esac

# Get the latest version tag from GitHub
LATEST_TAG=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$LATEST_TAG" ]; then
    echo "Failed to fetch the latest version tag."
    exit 1
fi

# Construct download URLs
BINARY_NAME="$ARCH-$BINARY-$OS"
echo "Binary name: $BINARY_NAME"
DOWNLOAD_URL="https://github.com/$REPO/releases/download/$LATEST_TAG/$BINARY_NAME"

# Download the binary
echo "Downloading $BINARY_NAME from $DOWNLOAD_URL..."
curl -L "$DOWNLOAD_URL" -o "$BINARY_NAME"

# Download checksums file
CHECKSUM_URL="https://github.com/$REPO/releases/download/$LATEST_TAG/linux-checksums.txt"
echo "Downloading checksums from $CHECKSUM_URL..."
curl -L "$CHECKSUM_URL" -o "linux-checksums.txt"

# Verify checksum
echo "Verifying checksum..."
EXPECTED_CHECKSUM=$(grep "$BINARY_NAME" linux-checksums.txt | cut -d ' ' -f 1)
ACTUAL_CHECKSUM=$(sha256sum "$BINARY_NAME" | cut -d ' ' -f 1)

if [ "$EXPECTED_CHECKSUM" != "$ACTUAL_CHECKSUM" ]; then
    echo "Checksum verification failed!"
    rm "$BINARY_NAME" linux-checksums.txt
    exit 1
fi

# Clean up checksums file
rm linux-checksums.txt

# Make the binary executable
chmod +x "$BINARY_NAME"

# Move the binary to the install directory
echo "Installing $BINARY_NAME to $INSTALL_DIR..."
sudo mv "$BINARY_NAME" "$INSTALL_DIR/$BINARY"

# Create configuration directory
CONFIG_DIR="/etc/easy-proxy"
if [ ! -d "$CONFIG_DIR" ]; then
    echo "Creating configuration directory at $CONFIG_DIR..."
    sudo mkdir -p "$CONFIG_DIR"
fi
CONFIG_DIR_PROXY="$CONFIG_DIR/proxy"
if [ ! -d "$CONFIG_DIR_PROXY" ]; then
    echo "Creating configuration directory at $CONFIG_DIR_PROXY..."
    sudo mkdir -p "$CONFIG_DIR_PROXY"
fi

# Write default configuration
CONFIG_FILE="$CONFIG_DIR/conf.yaml"
if [ ! -f "$CONFIG_FILE" ]; then
    echo "Writing default configuration to $CONFIG_FILE..."
    sudo tee "$CONFIG_FILE" > /dev/null <<EOL
proxy:
  http: "0.0.0.0:80"
  https: "0.0.0.0:443"
config_dir: "/etc/easy-proxy/proxy"
pingora:
  # https://github.com/cloudflare/pingora/blob/main/docs/user_guide/daemon.md
  daemon: true
  # https://github.com/cloudflare/pingora/blob/main/docs/user_guide/conf.md
  threads: $(nproc)
  # upstream_keepalive_pool_size: 20
  # work_stealing: true
  # error_log: /var/log/pingora/error.log
  # pid_file: /run/pingora.pid
  # upgrade_sock: /tmp/pingora_upgrade.sock
  # user: nobody
  # group: webusers
  # ca_file: /etc/ssl/certs/ca-certificates.crt
EOL
else
    echo "Configuration file already exists at $CONFIG_FILE. Skipping creation."
fi

# Create systemd service file
SERVICE_FILE="/etc/systemd/system/easy-proxy.service"
if [ ! -f "$SERVICE_FILE" ]; then
    echo "Creating systemd service file at $SERVICE_FILE..."
    sudo tee "$SERVICE_FILE" > /dev/null <<EOL
[Unit]
Description=Easy Proxy Service
After=network.target

[Service]
Type=simple
ExecStart=$INSTALL_DIR/$BINARY
Restart=on-failure

[Install]
WantedBy=multi-user.target
EOL
    # Reload systemd daemon
    echo "Reloading systemd daemon..."
    sudo systemctl daemon-reload

    # Enable and start the service
    echo "Enabling and starting easy-proxy service..."
    sudo systemctl enable easy-proxy
    sudo systemctl start easy-proxy
else
    echo "Service file already exists at $SERVICE_FILE. Skipping creation."
fi

# Verify service status
echo "Checking easy-proxy service status..."
sudo systemctl status easy-proxy

echo "Installation and setup completed successfully!"
