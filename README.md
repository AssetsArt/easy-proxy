# Easy Proxy Documentation

**Easy Proxy**, a simple proxy server designed to provide essential features for network traffic management and proxying.
based on [pingora](https://github.com/cloudflare/pingora)

## Features

Easy Proxy supports the following features:
- Routing
  - [x] Host-based routing
  - [x] Header-based routing
  - [x] Remove headers
  - [x] Add headers
  - [x] Rewrite by path
  - [x] Path matching Exact, Prefix
- Load Balancing
  - [x] RoundRobin
  - [x] Random
  - [x] Consistent # Weighted Ketama consistent hashing | [pingora - consistent](https://github.com/cloudflare/pingora/blob/main/pingora-load-balancing/src/selection/consistent.rs)
  - [x] Weighted
- [ ] SSL Termination
- [ ] Filtering
- [ ] Health Checking
- [ ] Logging and Monitoring
- Protocol Support
  - [x] HTTP
  - [ ] HTTPS

## Example configuration

### Global Configuration
```yaml
proxy:
  addr: "0.0.0.0:8088"
config_dir: ".config/proxy"
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
  # ca_file: /etc/ssl/certs/ca-certificates.crt
```

Can be tested and reloaded using the following commands:
```bash
$ easy-proxy -t # Test the configuration file
$ easy-proxy -r # Reload the configuration file
```

### Service and Route Configuration
```yaml
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

# A list of routes to be proxied 
header_selector: x-easy-proxy-svc
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
      value: localhost:8088
    name: my-route-1
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
$ EASY_PROXY_CONF=.config/easy_proxy.yaml ./target/release/easy-proxy
```
