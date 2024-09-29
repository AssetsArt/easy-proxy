# syntax=docker/dockerfile:1
# build stage
FROM ghcr.io/rust-lang/rust:nightly-slim AS builder
ENV DEBIAN_FRONTEND noninteractive
# install for build
RUN set -eux; \
    apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        g++ \
        cmake \
        libssl-dev \
        make \
        pkg-config \
        perl

WORKDIR /app
# copy app src
COPY . .
# build app
RUN cargo build --release

# create release image
FROM debian:latest
RUN apt-get update && apt-get install -y ca-certificates tzdata
RUN cp /usr/share/zoneinfo/Asia/Bangkok /etc/localtime
ENV LANG C.UTF-8
ENV LC_ALL C.UTF-8
ENV TZ=Asia/Bangkok

WORKDIR /app
# copy app release
COPY --from=builder /app/target/release/easy-proxy ./easy-proxy

# default run entrypoint
CMD ["./easy-proxy"]
