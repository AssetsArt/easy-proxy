FROM debian:stable-slim

WORKDIR /app

COPY target/x86_64-unknown-linux-gnu/release/easy-proxy .

CMD ["sh", "-c", "./easy-proxy --authen ${E_AUTH}"]
