#!/bin/bash

set -e

# Configuration
BINARY="easy-proxy"
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/easy-proxy"
SERVICE_FILE="/etc/systemd/system/easy-proxy.service"

# Stop the service if it's running
echo "Stopping $BINARY service..."
sudo systemctl stop $BINARY || true

# Disable the service
echo "Disabling $BINARY service..."
sudo systemctl disable $BINARY || true

# Remove the binary
if [ -f "$INSTALL_DIR/$BINARY" ]; then
    echo "Removing binary from $INSTALL_DIR/$BINARY..."
    sudo rm "$INSTALL_DIR/$BINARY"
else
    echo "Binary not found at $INSTALL_DIR/$BINARY."
fi

# Remove the configuration directory
if [ -d "$CONFIG_DIR" ]; then
    echo "Removing configuration directory $CONFIG_DIR..."
    sudo rm -rf "$CONFIG_DIR"
else
    echo "Configuration directory not found at $CONFIG_DIR."
fi

# Remove the systemd service file
if [ -f "$SERVICE_FILE" ]; then
    echo "Removing systemd service file $SERVICE_FILE..."
    sudo rm "$SERVICE_FILE"
    echo "Reloading systemd daemon..."
    sudo systemctl daemon-reload
else
    echo "Service file not found at $SERVICE_FILE."
fi

echo "$BINARY has been uninstalled successfully."
