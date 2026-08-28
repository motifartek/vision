FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
# Kurulum gereksinimleri (sonic dahil tüm servisler için ortak)
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev build-essential clang lld cmake && \
    rm -rf /var/lib/apt/lists/*

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY . .
# Workspace içindeki tüm crate'leri RAM kısıtına takılmamak için sınırlı paralellikle derliyoruz
RUN cargo build --release --workspace --jobs 2 && \
    strip target/release/vision && \
    strip target/release/stream && \
    strip target/release/orchestrator && \
    strip target/release/humanizer && \
    strip target/release/gateway && \
    strip target/release/toolbox && \
    strip target/release/sonic

# ----------------- VISION -----------------
FROM debian:trixie-slim AS vision
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /usr/local/bin
COPY --from=builder /app/target/release/vision .
ENV VISION_BIND=0.0.0.0:8110
EXPOSE 8110
CMD ["./vision"]

# ----------------- STREAM -----------------
FROM debian:trixie-slim AS stream
RUN apt-get update && apt-get install -y ffmpeg ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /usr/local/bin
COPY --from=builder /app/target/release/stream .
ENV STREAM_BIND=0.0.0.0:8100
EXPOSE 8100
CMD ["./stream"]

# ----------------- ORCHESTRATOR -----------------
FROM debian:trixie-slim AS orchestrator
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /usr/local/bin
COPY --from=builder /app/target/release/orchestrator .
CMD ["./orchestrator"]

# ----------------- HUMANIZER -----------------
FROM debian:trixie-slim AS humanizer
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /usr/local/bin
COPY --from=builder /app/target/release/humanizer .
ENV HUMANIZER_BIND=0.0.0.0:8115
EXPOSE 8115
CMD ["./humanizer"]

# ----------------- GATEWAY -----------------
FROM debian:trixie-slim AS gateway
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /usr/local/bin
COPY --from=builder /app/target/release/gateway .
EXPOSE 8000
CMD ["./gateway"]

# ----------------- TOOLBOX -----------------
FROM debian:trixie-slim AS toolbox
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/toolbox .
ENV RUST_LOG=info
CMD ["./toolbox"]

# ----------------- SONIC -----------------
FROM debian:trixie-slim AS sonic
RUN apt-get update && apt-get install -y ffmpeg ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/sonic .
RUN mkdir -p /app/models
EXPOSE 8081
ENV SONIC_PORT=8081
ENV SONIC_MODELS_DIR=/app/models
CMD ["./sonic"]
