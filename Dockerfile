FROM rust:1.99.1-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY assets ./assets
RUN cargo build --release --bin ch7_06_remote_deployment_dockerfile

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/ch7_06_remote_deployment_dockerfile /usr/local/bin/mcp-calculator
EXPOSE 8000
CMD ["mcp-calculator"]
