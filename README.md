# Easy Proxy Documentation

**Easy Proxy**, a simple proxy server designed to provide essential features for network traffic management and proxying.
based on [pingora](https://github.com/cloudflare/pingora)

## Installation or Upgrade
```bash
curl -H 'Cache-Control: no-cache' -fsSL https://raw.githubusercontent.com/AssetsArt/easy-proxy/main/scripts/install.sh | bash
```

## Uninstall
```bash
curl -H 'Cache-Control: no-cache' -fsSL https://raw.githubusercontent.com/AssetsArt/easy-proxy/main/scripts/uninstall.sh | bash
```

## Features

Easy Proxy supports the following features:
- Protocol Support
  - [x] HTTP
  - [x] HTTPS
- Certificate Management
  - [x] Custom
  - [x] ACME
- Service Endpoint
  - [x] HTTP
  - [ ] HTTPS
  - [ ] WASM (WebAssembly)
  - [ ] FFI (Foreign Function Interface)
- Route Matching
  - [x] Header-based
  - [x] Host-based
- Service Matching (Path)
  - [x] Exact
  - [x] Prefix
- Modify Request
  - [x] Add headers
  - [x] Remove headers
  - [x] Rewrite path
- Load Balancing
  - [x] RoundRobin
  - [x] Random
  - [x] Consistent # Weighted Ketama consistent hashing | [pingora - consistent](https://github.com/cloudflare/pingora/blob/main/pingora-load-balancing/src/selection/consistent.rs)
  - [x] Weighted
- Middleware / Plugins Support
  - [ ] FFI (Foreign Function Interface)
  - [ ] WASM (WebAssembly)
- [ ] Health Checking
- [ ] Logging and Monitoring

## Example configuration

### Global Configuration
```yaml
proxy:
  http: "0.0.0.0:80"
  https: "0.0.0.0:443"
config_dir: "/etc/easy-proxy/proxy"
# optional
acme_store: "/etc/easy-proxy/acme.json" # auto generated
pingora:
  # https://github.com/cloudflare/pingora/blob/main/docs/user_guide/daemon.md
  daemon: true
  # https://github.com/cloudflare/pingora/blob/main/docs/user_guide/conf.md
  threads: 6
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

Can be tested and reloaded using the following commands:
```bash
$ easy-proxy -t # Test the configuration file
$ easy-proxy -r # Reload the configuration file
```

### Service and Route Configuration
```yaml
# my-config.yaml
# Select the service to be proxied
header_selector: x-easy-proxy-svc

# Services to be proxied
services:
  - name: my-service
    type: http
    algorithm: round_robin # round_robin, random, consistent, weighted
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
    type: custom # acme, custom
    # acme: # required if type is acme
    #   provider: letsencrypt # letsencrypt or buypass // optional default letsencrypt
    #   email: admin@domain.com
    key: /etc/easy-proxy/ssl/localhost.key
    cert: /etc/easy-proxy/ssl/localhost.crt
    # chain: # optional
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
    tls: # optional
      name: my-tls
      redirect: true # redirect to https default: false
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

## Use from source
```bash
# Clone the repository
$ git clone https://github.com/AssetsArt/easy-proxy.git
# Change the working directory
$ cd easy-proxy
# Build the application
$ cargo build --release
# Run the application // EASY_PROXY_CONF is the environment variable to set the configuration file path
$ EASY_PROXY_CONF=.config/easy-proxy.yaml ./target/release/easy-proxy
```
