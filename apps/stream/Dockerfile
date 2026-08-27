<<<<<<< HEAD
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
=======
# stream — video alımı, hareket analizi, klip üretimi
#
# ffmpeg hem derleme hem çalıştırma aşamasında gerekmiyor: yalnızca çalışma
# aşamasında, alt süreç olarak çağrılıyor.

FROM rust:slim-bookworm AS builder

RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
>>>>>>> f491502c5faca5ab535093d137310c684fca7a50
COPY packages packages
COPY apps apps
COPY tools tools

<<<<<<< HEAD
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
=======
RUN cargo build --release -p stream

FROM debian:bookworm-slim

# ffmpeg/ffprobe zorunlu: optics bunları alt süreç olarak çalıştırıyor.
RUN apt-get update && \
    apt-get install -y ffmpeg ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/stream /usr/local/bin/stream

ENV STREAM_BIND=0.0.0.0:8100
EXPOSE 8100
CMD ["stream"]
>>>>>>> f491502c5faca5ab535093d137310c684fca7a50
