FROM debian:11-slim

WORKDIR /app

COPY target/x86_64-unknown-linux-gnu/release/easy-proxy .

CMD ["./easy-proxy", "--authen", "${E_AUTH}"]