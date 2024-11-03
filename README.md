# Easy Proxy Documentation

**Easy Proxy** is a simple proxy server designed to provide essential features for network traffic management and proxying. It is based on [Pingora](https://github.com/cloudflare/pingora).

## Installation or Upgrade

To install or upgrade Easy Proxy, run the following command:

```bash
curl -H 'Cache-Control: no-cache' -fsSL https://raw.githubusercontent.com/AssetsArt/easy-proxy/main/scripts/install.sh | bash
```

## Uninstall

To uninstall Easy Proxy, execute:

```bash
curl -H 'Cache-Control: no-cache' -fsSL https://raw.githubusercontent.com/AssetsArt/easy-proxy/main/scripts/uninstall.sh | bash
```

## Features

Easy Proxy supports the following features:

### Protocol Support
- [x] **HTTP**
- [x] **HTTPS**

### Certificate Management
- [x] **Custom Certificates**
- [x] **ACME (Automated Certificate Management Environment)**

### Service Endpoint
- [x] **HTTP**
- [ ] **HTTPS**
- [ ] **WASM (WebAssembly)**
- [ ] **FFI (Foreign Function Interface)**

### Route Matching
- [x] **Header-based Matching**
- [x] **Host-based Matching**

### Service Matching (Path)
- [x] **Exact Match**
- [x] **Prefix Match**

### Modify Request
- [x] **Add Headers**
- [x] **Remove Headers**
- [x] **Rewrite Path**

### Load Balancing
- [x] **Round Robin**
- [x] **Random**
- [x] **Consistent Hashing**
  - **Weighted Ketama Consistent Hashing** | [Pingora - Consistent](https://github.com/cloudflare/pingora/blob/main/pingora-load-balancing/src/selection/consistent.rs)
- [x] **Weighted**

### Middleware / Plugins Support
- [ ] **FFI (Foreign Function Interface)**
- [ ] **WASM (WebAssembly)**

### Additional Features
- [ ] **Health Checking**
- [ ] **Logging and Monitoring**

## Example Configuration

### Global Configuration

```yaml
proxy:
  http: "0.0.0.0:80"
  https: "0.0.0.0:443"
config_dir: "/etc/easy-proxy/proxy"
# Optional
acme_store: "/etc/easy-proxy/acme.json" # Automatically generated

pingora:
  # Refer to Pingora's daemon documentation: https://github.com/cloudflare/pingora/blob/main/docs/user_guide/daemon.md
  daemon: true
  # Refer to Pingora's configuration documentation: https://github.com/cloudflare/pingora/blob/main/docs/user_guide/conf.md
  threads: 6
  # Optional settings (uncomment to use)
  # upstream_keepalive_pool_size: 20
  # work_stealing: true
  # error_log: /var/log/pingora/error.log
  # pid_file: /run/pingora.pid
  # upgrade_sock: /tmp/pingora_upgrade.sock
  # user: nobody
  # group: webusers
  grace_period_seconds: 60
  graceful_shutdown_timeout_seconds: 10
  # ca_file: /etc/ssl/certs/ca-certificates.crt
```

### Service and Route Configuration

```yaml
# my-config.yaml

# Select the service to be proxied based on the specified header
header_selector: x-easy-proxy-svc

# Services to be proxied
services:
  - name: my-service
    type: http
    algorithm: round_robin # Options: round_robin, random, consistent, weighted
    endpoints:
      - ip: 127.0.0.1
        port: 3000
        weight: 10 # Optional
      - ip: 127.0.0.1
        port: 3001
        weight: 1 # Optional

# TLS Configuration
tls:
  - name: my-tls
    type: custom # Options: acme, custom
    # If type is 'acme', the following fields are required:
    # acme:
    #   provider: letsencrypt # Options: letsencrypt, buypass (default: letsencrypt)
    #   email: admin@domain.com
    key: /etc/easy-proxy/ssl/localhost.key
    cert: /etc/easy-proxy/ssl/localhost.crt
    # Optional chain certificates
    # chain:
    #   - /etc/easy-proxy/ssl/chain.pem

# Routes to be proxied
routes:
  - route:
      type: header
      value: service-1
    name: my-route-header-1
    paths:
      - pathType: Exact
        path: /
        service:
          rewrite: /rewrite
          name: my-service
          
  - route:
      type: host
      value: localhost
    name: my-route-1
    tls: # Optional TLS settings for this route
      name: my-tls
      redirect: true # Redirect to HTTPS (default: false)
    remove_headers:
      - cookie
    add_headers:
      - name: x-custom-header
        value: "123"
      - name: x-real-ip
        value: "$CLIENT_IP"
    paths:
      - pathType: Exact
        path: /
        service:
          name: my-service
      - pathType: Exact
        path: /api/v1
        service:
          rewrite: /rewrite
          name: my-service
      - pathType: Prefix
        path: /api/prefix
        service:
          rewrite: /prefix
          name: my-service
```

## Testing and Reloading the Service

You can test and reload the service using the following commands:

```bash
$ easy-proxy -t    # Test the configuration file
$ easy-proxy -r    # Reload the configuration file
```

### systemd Service Commands

Manage the Easy Proxy service with the following `service` commands:

```bash
$ service easy-proxy start
$ service easy-proxy stop
$ service easy-proxy restart
# Restart includes the global configuration and ensures zero downtime
# Note: `grace_period_seconds` and `graceful_shutdown_timeout_seconds` are used to wait for existing connections to close
$ service easy-proxy reload    # Reload the configuration file without including the global configuration
$ service easy-proxy status
```

**Details:**
- **Restart:** Applies the global configuration and ensures zero downtime by gracefully handling existing connections.
- **Reload:** Reloads the configuration file without affecting the global configuration, allowing for quick updates without restarting the entire service.

## Use from Source

If you prefer to build Easy Proxy from source, follow these steps:

1. **Clone the Repository:**

    ```bash
    git clone https://github.com/AssetsArt/easy-proxy.git
    ```

2. **Change the Working Directory:**

    ```bash
    cd easy-proxy
    ```

3. **Build the Application:**

    ```bash
    cargo build --release
    ```

4. **Run the Application:**

    ```bash
    # EASY_PROXY_CONF is the environment variable to set the configuration file path
    EASY_PROXY_CONF=.config/easy-proxy.yaml ./target/release/easy-proxy
    ```

**Note:** Ensure that you have [Rust](https://www.rust-lang.org/tools/install) installed on your system to build Easy Proxy from source.

---
