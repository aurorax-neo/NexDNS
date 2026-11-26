ARG BASE_IMAGE=alpine:3.21
ARG BUILDER_IMAGE=rust:alpine

FROM ${BUILDER_IMAGE} AS builder
RUN apk add --no-cache musl-dev gcc cmake make perl
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY backend/ backend/
COPY frontend/dist/ frontend/dist/
WORKDIR /app/backend
RUN cargo build --release --locked

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
