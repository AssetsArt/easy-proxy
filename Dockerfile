# syntax=docker/dockerfile:1
# build stage
FROM rust:slim as builder
WORKDIR /app
# copy app src
COPY . .
# build app
RUN RUSTFLAGS="-C target-cpu=native" cargo build --release

# create release image
FROM gcr.io/distroless/cc:nonroot

ARG timezone=Asia/Bangkok

ENV LANG C.UTF-8
ENV LC_ALL C.UTF-8
ENV TZ $timezone

WORKDIR /app
# copy app release
COPY --from=builder /app/target/release/runtime ./easy-proxy

# expose default port
EXPOSE 1337 
EXPOSE 8080

# default run entrypoint
CMD ["./easy-proxy"]