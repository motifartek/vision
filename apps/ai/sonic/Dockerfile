FROM ubuntu:24.04 AS builder

RUN apt-get update && \
    apt-get install -y \
        curl \
        pkg-config \
        libssl-dev \
        build-essential \
        clang \
        lld \
        && \
    rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY packages packages
COPY apps apps
COPY tools tools

# Sonic'i derle
RUN cargo build --release -p sonic

# ---- Stage 2: Runtime ----
FROM ubuntu:24.04

RUN apt-get update && \
    apt-get install -y \
        ffmpeg \
        ca-certificates \
        && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/sonic /app/sonic

# Model agirliklari icin dizin
RUN mkdir -p /app/models

EXPOSE 8081

ENV SONIC_PORT=8081
ENV SONIC_MODELS_DIR=/app/models

CMD ["./sonic"]
