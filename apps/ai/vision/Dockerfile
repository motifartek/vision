# vision — video analiz ajanı
#
# ffmpeg gerekmiyor: klip üretimini stream yapıyor, bu servis yalnızca HTTP
# konuşuyor.

FROM rust:slim-bookworm AS builder

RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY packages packages
COPY apps apps
COPY tools tools

RUN cargo build --release -p vision

FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/vision /usr/local/bin/vision

ENV VISION_BIND=0.0.0.0:8110
EXPOSE 8110
CMD ["vision"]
