FROM rust:slim-bookworm AS builder
WORKDIR /app

RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev protobuf-compiler curl && \
    rm -rf /var/lib/apt/lists/*

COPY . .

RUN cargo build --release -p toolbox

FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/toolbox /app/toolbox

ENV RUST_LOG=info
CMD ["/app/toolbox"]
