# Easy Proxy Documentation

**Easy Proxy**, a simple proxy server designed to provide essential features for network traffic management and proxying.

## Features

Easy Proxy supports the following features:

- [x] Load Balancing
  - Algorithm
  - [x] round_robin
  - [ ] least_connection
  - [ ] ip_hash
- [ ] SSL Termination
- [ ] Caching
- [ ] Filtering
- [ ] Health Checking
- [ ] Logging and Monitoring
- Protocol Support
  - [x] HTTP
  - [ ] HTTPS

## Development

To contribute or use Easy Proxy, follow these steps:

1. Clone the repository:
   ```sh
   git clone https://github.com/Aitthi/easy-proxy.git
   ```
2. Change to the project directory:
   ```sh
   cd easy-proxy
   ```
3. Generate an RSA certificate for JWT:
   ```sh
   openssl genrsa -out ./config/jwt/private.key 4096
   ```
   ```sh
   openssl rsa -in ./config/jwt/private.key -pubout -outform PEM -out ./config/jwt/public.key
   ```
4. Run the application:
   ```sh
   cargo run
   ```
   Or, to automatically rebuild and restart:
   ```sh
   cargo watch -q -c -x 'run'
   ```

## API Documentation

The API documentation outlines the endpoints and how to interact with Easy Proxy.

**Base URL:** http://localhost:1337/apidoc