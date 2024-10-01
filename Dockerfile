# syntax=docker/dockerfile:1
FROM debian:latest
RUN apt-get update && apt-get install -y ca-certificates tzdata
RUN cp /usr/share/zoneinfo/Asia/Bangkok /etc/localtime
ENV LANG C.UTF-8
ENV LC_ALL C.UTF-8
ENV TZ=Asia/Bangkok

WORKDIR /app

# install easy-proxy
RUN curl -fsSL https://raw.githubusercontent.com/AssetsArt/easy-proxy/main/scripts/install.sh | bash -s -- --no-service

# default run entrypoint
CMD ["/usr/local/bin/easy-proxy"]
