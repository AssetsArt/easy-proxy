# syntax=docker/dockerfile:1
# build stage
FROM ghcr.io/rust-lang/rust:nightly-bookworm-slim as builder 
RUN apt update && apt install libclang-dev -y
WORKDIR /app
# copy app src
COPY . .
# build app
RUN RUSTFLAGS="-C target-cpu=native" cargo build --release

# create release image
FROM debian:latest

ARG timezone=Asia/Bangkok

ENV LANG C.UTF-8
ENV LC_ALL C.UTF-8
ENV TZ $timezone

WORKDIR /app
# copy app release
COPY --from=builder /app/target/release/runtime ./easy-proxy

# expose default port
EXPOSE 1337 
EXPOSE 8088

# default run entrypoint
CMD ["./easy-proxy"]