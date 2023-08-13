# syntax=docker/dockerfile:1
# build stage
FROM rust:slim as builder
WORKDIR /app
# copy app src
COPY . .
# build app
RUN cargo build --release

# create release image
FROM gcr.io/distroless/cc:nonroot

ARG timezone=Asia/Bangkok
ARG e_auth="my-auth-key"

ENV LANG C.UTF-8
ENV LC_ALL C.UTF-8
ENV TZ $timezone
ENV E_AUTH $e_auth

WORKDIR /app
# copy app release
COPY --from=builder /app/target/release/easy-proxy ./

# expose default port
EXPOSE 8100

# default run entrypoint
ENTRYPOINT ["/usr/bin/dumb-init", "--", "/app/easy-proxy"]
CMD ["--authen", "${E_AUTH}"]