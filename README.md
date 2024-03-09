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

Can be tested and reloaded using the following commands:
```bash
$ easy-proxy -t # Test the configuration file
$ easy-proxy -r # Reload the configuration file
```

### Service and Route Configuration
```yaml
# Select the service to be proxied
service_selector:
  header: x-easy-proxy-svc # from header key "x-easy-proxy-svc"

# Services to be proxied
services:
  - name: backend_service
    algorithm: round_robin # round_robin, random, consistent, weighted
    endpoints:
      - ip: 127.0.0.1
        port: 3002
        weight: 1 # Optional
        
# A list of routes to be proxied 
routes:
  - host: mydomain.com
    del_headers:
      - accept
    add_headers:
      - name: x-custom-header # no case sensitive
        value: "123" # olny string
    paths:
      - pathType: Exact # Exact, Prefix
        path: /api/v1
        service:
          rewrite: /old_service/v1 # Optional
          name: backend_service
      - pathType: Prefix # Exact, Prefix
        path: /api/v1
        service:
          rewrite: /service/v1 # Optional
          name: backend_service
  - header: svc.service1 # from header key "x-easy-proxy-svc"
    paths:
      - pathType: Prefix # Exact, Prefix
        path: /svc/v1
        service:
          name: backend_service
```

## Use from docker-compose

```yaml
# docker-compose.yml
version: '3.8'
services:
  easy-proxy:
    image: ghcr.io/assetsart/easy-proxy:latest
    ports:
      - 8088:8088
    volumes:
      - ./examples:/app/examples
    networks:
      - gateway
  http-echo-1:
    image: hashicorp/http-echo:latest
    command: ["-text", "Hello, World 3002"]
    ports:
      - "3002:5678"
    networks:
      - gateway
  http-echo-2:
    image: hashicorp/http-echo:latest
    command: ["-text", "Hello, World 3003"]
    ports:
      - "3003:5678"
    networks:
      - gateway
networks:
  gateway: {}
```
```bash
$ docker-compose up
```
```bash
$ curl -H "Host: mydomain.com" http://localhost:8088/api/v1
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
$ EASY_PROXY_CONF=.config/easy_proxy.yaml ./target/release/runtime
```