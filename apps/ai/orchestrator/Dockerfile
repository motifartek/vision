# ---- Stage 1: Builder ----
FROM rust:slim-bookworm AS builder

RUN apt-get update && \
    apt-get install -y \
        pkg-config \
        libssl-dev \
        && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY packages packages
COPY apps apps
COPY tools tools

RUN cargo build --release -p orchestrator

# ---- Stage 2: Runtime ----
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y \
        ca-certificates \
        && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/orchestrator /app/orchestrator

CMD ["./orchestrator"]