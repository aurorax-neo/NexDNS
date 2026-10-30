ARG RUST_IMAGE=rust:alpine
ARG BASE_IMAGE=alpine:latest

FROM ${RUST_IMAGE} AS builder
RUN apk add --no-cache musl-dev gcc
WORKDIR /app
RUN mkdir -p /root/.cargo && \
    echo '[source.crates-io]' > /root/.cargo/config.toml && \
    echo 'replace-with = "rsproxy-sparse"' >> /root/.cargo/config.toml && \
    echo '[source.rsproxy-sparse]' >> /root/.cargo/config.toml && \
    echo 'registry = "sparse+https://rsproxy.cn/index/"' >> /root/.cargo/config.toml && \
    echo '[net]' >> /root/.cargo/config.toml && \
    echo 'git-fetch-with-cli = true' >> /root/.cargo/config.toml
COPY Cargo.toml ./
COPY backend/Cargo.toml backend/
COPY backend/src backend/src
COPY frontend/dist frontend/dist
WORKDIR /app/backend
RUN cargo build --release

FROM ${BASE_IMAGE}
WORKDIR /app
RUN apk add --no-cache ca-certificates tzdata && \
    ln -snf /usr/share/zoneinfo/Asia/Shanghai /etc/localtime && \
    echo "Asia/Shanghai" > /etc/timezone
ENV TZ=Asia/Shanghai
COPY --from=builder /app/target/release/nexdns /app/nexdns
EXPOSE 53/udp 53/tcp 3000/tcp 8053/tcp
ENTRYPOINT ["/app/nexdns"]
CMD ["--config", "/app/data/config.yaml"]
