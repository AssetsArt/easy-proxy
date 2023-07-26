FROM debian:stable-slim

WORKDIR /app

COPY target/release/easy-proxy .

CMD ["sh", "-c", "./easy-proxy --authen ${E_AUTH}"]
