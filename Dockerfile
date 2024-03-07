# syntax=docker/dockerfile:1
# build stage
FROM ghcr.io/rust-lang/rust:nightly-alpine as builder
RUN set -eux; \
    apk add --no-cache \
    libressl-dev \
    musl-dev \
    gcc \
    clang \
    pkgconfig

WORKDIR /app
# copy app src
COPY . .
# build app
RUN cargo build --release

# create release image
FROM alpine:latest
RUN apk add --no-cache ca-certificates tzdata
RUN cp /usr/share/zoneinfo/Asia/Bangkok /etc/localtime
ENV LANG C.UTF-8
ENV LC_ALL C.UTF-8
ENV TZ=Asia/Bangkok

WORKDIR /app
# copy app release
COPY --from=builder /app/target/release/runtime ./easy-proxy
COPY .config .config

# default run entrypoint
CMD ["./easy-proxy"]