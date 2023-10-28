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

**Postman URL:** https://documenter.getpostman.com/view/5547449/2s9YRGxUKB

**Swagger URL:** http://localhost:1337/apidoc

### Benchmarks on M1 pro 10 core 16GB | 2023-10-28
```sh 
command: `drill --benchmark benchmark.yml --stats -q`

| Metric                                  | Easy Proxy            | Directly to the backend |
|-----------------------------------------|-----------------------|-------------------------|
| Concurrency                             | 150                   | 150                     |
| Iterations                              | 100000                | 100000                  |
| Rampup                                  | 1                     | 1                       |
| Base URL                                | http://127.0.0.1:8088 | http://127.0.0.1:3002   |
| Landing page Total requests             | 100000                | 100000                  |
| Landing page Successful requests        | 100000                | 100000                  |
| Landing page Failed requests            | 0                     | 0                       |
| Landing page Median time per request    | 2ms                   | 1ms                     |
| Landing page Average time per request   | 2ms                   | 1ms                     |
| Landing page Sample standard deviation  | 2ms                   | 1ms                     |
| Landing page 99.0'th percentile         | 7ms                   | 3ms                     |
| Landing page 99.5'th percentile         | 8ms                   | 4ms                     |
| Landing page 99.9'th percentile         | 18ms                  | 10ms                    |
| Time taken for tests                    | 1.9 seconds           | 1.8 seconds             |
| Total requests                          | 100000                | 100000                  |
| Successful requests                     | 100000                | 100000                  |
| Failed requests                         | 0                     | 0                       |
| Requests per second                     | 51622.21 [#/sec]      | 57096.73 [#/sec]        |
