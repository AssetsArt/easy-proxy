#!/bin/bash

set -e

# Configuration
BINARY="easy-proxy"
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/easy-proxy"
SERVICE_FILE="/etc/systemd/system/easy-proxy.service"

# Stop and disable the service
echo "Stopping and disabling $BINARY service..."
sudo systemctl stop $BINARY 2>/dev/null || echo "$BINARY service not running."
sudo systemctl disable $BINARY 2>/dev/null || echo "$BINARY service not enabled."

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

PID=$(ps aux | grep $BINARY | grep -v grep | awk '{print $2}')
if [ -n "$PID" ]; then
    echo "$BINARY is still running with PID $PID."
    kill -s SIGKILL $PID
    echo "$BINARY has been uninstalled successfully."
else
    echo "$BINARY has been uninstalled successfully."
fi

