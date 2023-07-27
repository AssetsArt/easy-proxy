# syntax=docker/dockerfile:1
# build stage
FROM rust:slim as builder
WORKDIR /app
# copy app src
COPY . .
# build app
RUN cargo build --release

# create release image
FROM debian:stable-slim
ARG	timezone=Asia/Bangkok

ENV	LANG C.UTF-8
ENV	LC_ALL C.UTF-8
ENV	TZ $timezone
ENV E_AUTH="SomeYourSecret"

# Update OS
RUN	apt-get update && apt-get -y full-upgrade \
    && apt-get -y install dumb-init locales tzdata net-tools ca-certificates \
    && apt-get clean

# Change locale
RUN echo $timezone > /etc/timezone \
    && cp /usr/share/zoneinfo/$timezone /etc/localtime

WORKDIR /app
# copy app release
COPY --from=builder /app/target/release/easy-proxy ./
# expose default port
EXPOSE 8100

# default run entrypoint
ENTRYPOINT ["/usr/bin/dumb-init", "--"]
CMD ["./easy-proxy", "--authen", "${E_AUTH}"]