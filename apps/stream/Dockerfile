# ---- Stage 1: Builder ----
FROM rust:slim-bookworm AS builder

# ffmpeg, ffprobe ve derleme araclarini kur
RUN apt-get update && \
    apt-get install -y \
        ffmpeg \
        pkg-config \
        libssl-dev \
        && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Workspace manifest dosyalarini once kopyala (layer cache icin)
COPY Cargo.toml Cargo.lock ./

# Butun kaynak kodunu kopyala
COPY packages packages
COPY apps apps
COPY tools tools

# Sadece stream binary'sini release modunda derle
RUN cargo build --release -p stream

# ---- Stage 2: Runtime ----
FROM debian:bookworm-slim

# ffmpeg runtime icin gerekli kutuphaneler + CA sertifikalari
RUN apt-get update && \
    apt-get install -y \
        ffmpeg \
        ca-certificates \
        && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Derlenmis binary'i kopyala
COPY --from=builder /app/target/release/stream /app/stream

# Nesne deposu icin dizin
RUN mkdir -p /data/stream

EXPOSE 8100

ENV STREAM_BIND=0.0.0.0:8100
ENV STREAM_STORAGE_ROOT=/data/stream

CMD ["./stream"]