# Easy Proxy Documentation

**Easy Proxy**, a simple proxy server designed to provide essential features for network traffic management and proxying.

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
- [ ] Caching
- [ ] Filtering
- [ ] Health Checking
- [ ] Logging and Monitoring
- Protocol Support
  - [x] HTTP
  - [ ] HTTPS
