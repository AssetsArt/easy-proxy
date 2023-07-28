# Easy Proxy

A simple proxy server.

## Features should be supported
- [] Load Balancing
- [] Duplicate forwarding
- [] SSL Termination
- [] Caching
- [] Content Compression
- [] Filtering
- [] Health Checking
- [] Logging and Monitoring
- Protocol Support
  - [x] HTTP
  - [] HTTPS
- [] Web UI

## Development
- ### Database
  - [Surrealdb](https://surrealdb.com/docs/integration/sdks/rust)

- ### Jwt
  ```sh
  ssh-keygen -t rsa -b 4096 -m PEM -E SHA512 -f cert/jwt.pem
  ```

## Test
```sh
curl --location 'https://domain.com/ipinfo' \
--header 'x-proxy-ip: 10.42.2.104' \
--header 'x-proxy-port: 8000' \
--header 'x-proxy-authen: my-auth-key'
```
