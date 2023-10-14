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

**Base URL:** http://localhost:3100

### Endpoint: POST `/api/install`

This endpoint performs the initial setup of the application, which includes creating an initial administrative user account.

#### Request Example

```sh
curl -X POST \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "P@ssw0rd"}' \
  http://localhost:3100/api/install
```

---

### Endpoint: GET `/api/is_install`

This endpoint checks if the application has been installed.

#### Request Example

```sh
curl -X GET http://localhost:3100/api/is_install
```

#### Response Example

```json
{
  "is_install": true
}
```

---

### Endpoint: POST `/api/admin/authen`

This endpoint is used to authenticate an administrative user.

#### Request Example

```sh
curl -X POST \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "P@ssw0rd"}' \
  http://localhost:3100/api/admin/authen
```

#### Response Example

```json
{
  "user": {
    "id": "admin:uaz3q3",
    "name": "Administrator",
    "username": "admin",
    "role": "super_admin"
  },
  "jwt": {
    "type": "Bearer",
    "expires_in": 1692471676,
    "token": "eyJhbGciO...QfX0"
  }
}
```

---
### Endpoint: POST `/api/services/add`
This endpoint is used to add a new service.
#### Request Example
```json
{
  "name": "service1", // Unique allowed characters: a-z, 0-9, -, _
  "host": "myhost.com",
  "algorithm": "round_robin",
  "destination": [
    {
      "ip": "127.0.0.1",
      "port": 8080,
      "protocol": "http",
      "status": true
    },
    {
      "ip": "127.0.0.2",
      "port": 8080,
      "protocol": "http",
      "status": true
    }
  ]
}
```
---
### Endpoint: POST `/api/services/update/:id`
This endpoint is used to update a service.
#### Request Example
```json
{
  "name": "service1", // Unique allowed characters: a-z, 0-9, -, _
  "host": "myhost.com",
  "algorithm": "round_robin",
  "destination": [
    {
      "ip": "127.0.0.1",
      "port": 8080,
      "protocol": "http",
      "status": true
    }
  ]
}
```