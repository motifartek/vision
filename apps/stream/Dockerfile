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
COPY packages packages
COPY apps apps
COPY tools tools

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
