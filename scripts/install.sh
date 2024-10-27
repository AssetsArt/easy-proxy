#!/bin/bash

set -e

# Configuration
REPO="AssetsArt/easy-proxy"  # Replace with your GitHub username/repo
BINARY="easy-proxy"
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/easy-proxy"
SERVICE_FILE="/etc/systemd/system/easy-proxy.service"
START_SCRIPT="$CONFIG_DIR/scripts/start.sh"
STOP_SCRIPT="$CONFIG_DIR/scripts/stop.sh"
RESTART_SCRIPT="$CONFIG_DIR/scripts/restart.sh"
IS_CREATE_SERVICE=true

# Parse arguments
if [ "$1" == "--no-service" ]; then
    IS_CREATE_SERVICE=false
fi

# Detect OS and architecture
OS=$(uname | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)
case $ARCH in
    x86_64) ARCH="x86_64" ;;
    arm64 | aarch64) ARCH="aarch64" ;;
    *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

# Ensure only Linux is supported
if [[ "$OS" != "linux" ]]; then
    echo "This install script supports Linux only."
    exit 1
fi

# Detect OS type (gnu or musl)
OS_TYPE=$(ldd --version 2>&1 | grep -q musl && echo "musl" || echo "gnu")

# Fetch latest release tag from GitHub
LATEST_TAG=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
if [ -z "$LATEST_TAG" ]; then
    echo "Failed to fetch the latest version."
    exit 1
fi

# Construct download URL
BINARY_NAME="$BINARY-$ARCH-$OS-$OS_TYPE"
DOWNLOAD_URL="https://github.com/$REPO/releases/download/$LATEST_TAG/$BINARY_NAME"
CHECKSUM_URL="https://github.com/$REPO/releases/download/$LATEST_TAG/linux-checksums.txt"

# Download binary and checksum
curl -L "$DOWNLOAD_URL" -o "$BINARY_NAME"
curl -L "$CHECKSUM_URL" -o "linux-checksums.txt"

# Verify checksum
EXPECTED_CHECKSUM=$(grep "$BINARY_NAME" linux-checksums.txt | cut -d ' ' -f 1)
ACTUAL_CHECKSUM=$(sha256sum "$BINARY_NAME" | cut -d ' ' -f 1)
if [ "$EXPECTED_CHECKSUM" != "$ACTUAL_CHECKSUM" ]; then
    echo "Checksum verification failed!"
    rm "$BINARY_NAME" linux-checksums.txt
    exit 1
fi
rm linux-checksums.txt

# Install binary
chmod +x "$BINARY_NAME"
sudo mv "$BINARY_NAME" "$INSTALL_DIR/$BINARY"

# Restart service if running
if systemctl is-active --quiet easy-proxy; then
    echo "Restarting easy-proxy service..."
    sudo systemctl restart easy-proxy
fi

# Create configuration directory and default config
sudo mkdir -p "$CONFIG_DIR/proxy"
CONFIG_FILE="$CONFIG_DIR/conf.yaml"
if [ ! -f "$CONFIG_FILE" ]; then
    echo "Creating default configuration..."
    sudo tee "$CONFIG_FILE" > /dev/null <<EOL
proxy:
  http: "0.0.0.0:80"
  https: "0.0.0.0:443"
config_dir: "/etc/easy-proxy/proxy"
pingora:
  daemon: true
  threads: $(nproc)
  grace_period_seconds: 60
  graceful_shutdown_timeout_seconds: 10
EOL
fi

# Create start, stop, and restart scripts
sudo mkdir -p "$CONFIG_DIR/scripts"
echo "Creating start/stop/restart scripts..."

sudo tee "$START_SCRIPT" > /dev/null <<EOL
#!/bin/bash
mkdir -p /var/log/easy-proxy
$INSTALL_DIR/$BINARY >> /var/log/easy-proxy/easy-proxy.log 2>&1
PID=\$(ps aux | grep $BINARY | grep -v grep | awk '{print \$2}')
if [ -n "\$PID" ]; then
    echo "easy-proxy is running with PID \$PID."
else
    echo "easy-proxy is not running."
fi
EOL

sudo tee "$STOP_SCRIPT" > /dev/null <<EOL
#!/bin/bash
PID=\$(ps aux | grep $INSTALL_DIR/$BINARY | grep -v grep | awk '{print \$2}')
if [ -n "\$PID" ]; then
    echo "Stopping easy-proxy with PID \$PID..."
    kill -s SIGTERM \$PID
else
    echo "easy-proxy is not running."
fi
EOL

sudo tee "$RESTART_SCRIPT" > /dev/null <<EOL
#!/bin/bash
$STOP_SCRIPT && $START_SCRIPT
EOL

sudo chmod +x "$START_SCRIPT" "$STOP_SCRIPT" "$RESTART_SCRIPT"

# Create systemd service if required
if [ "$IS_CREATE_SERVICE" = true ]; then
    if [ ! -f "$SERVICE_FILE" ]; then
        echo "Creating systemd service file..."
        sudo tee "$SERVICE_FILE" > /dev/null <<EOL
[Unit]
Description=Easy Proxy Service
After=network.target

[Service]
Type=simple
ExecStart=$START_SCRIPT
ExecStop=$STOP_SCRIPT
ExecRestart=$RESTART_SCRIPT
ExecReload=$INSTALL_DIR/$BINARY -r
Restart=on-failure
RestartSec=0
KillMode=process

[Install]
WantedBy=multi-user.target
EOL
        sudo systemctl daemon-reload
        sudo systemctl enable --now easy-proxy
        echo "Installation and setup completed successfully!"
    else
        echo "Service file already exists. Restarting service..."
        # sudo systemctl restart easy-proxy
        # check config file $INSTALL_DIR/$BINARY -t
        if $INSTALL_DIR/$BINARY -t; then
            sudo systemctl restart easy-proxy
            echo "Installation and setup completed successfully!"
        else
            echo "easy-proxy configuration is invalid. Please check the configuration file."
        fi
    fi
fi
