# Easy Proxy Documentation

**Easy Proxy**, a simple proxy server designed to provide essential features for network traffic management and proxying.
based on [pingora](https://github.com/cloudflare/pingora)

## Features

Easy Proxy supports the following features:
- Routing
  - [x] Host-based routing
  - [x] Remove headers
  - [x] Add headers
  - [x] Rewrite by path
  - [x] Path matching Exact, Prefix
- Services
  - [x] Ip and Port
  - [x] Weight
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
providers:
  - name: files
    path: examples
    watch: true
pingora:
  # https://github.com/cloudflare/pingora/blob/main/docs/user_guide/daemon.md
  daemon: true
  # https://github.com/cloudflare/pingora/blob/main/docs/user_guide/conf.md
  threads: 4
  # work_stealing: true
  # error_log: /var/log/pingora/error.log
  # pid_file: /run/pingora.pid
  # upgrade_sock: /tmp/pingora_upgrade.sock
  # user: nobody
  # group: webusers
  # ca_file: /etc/ssl/certs/ca-certificates.crt
```

### Service and Route Configuration
```yaml
# Description: Service proxy configuration
services:
  - name: backend_service
    algorithm: round_robin # round_robin, random, consistent, weighted
    endpoints:
      - ip: 127.0.0.1
        port: 3002
        weight: 1 # Optional
        
routes:
  - host: localhost:8088
    del_headers:
      - accept
    headers:
      - name: x-custom-header # no case sensitive
        value: "123" # olny string
    paths:
      - pathType: Exact # Exact, Prefix
        path: /api/v1
        service:
          rewrite: /service/v1 # Optional
          name: backend_service
      - pathType: Prefix # Exact, Prefix
        path: /api/v2
        service:
          rewrite: /service/v2 # Optional
          name: backend_service
```