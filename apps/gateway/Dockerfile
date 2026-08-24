# 1. Builder Stage
FROM rust:slim-bookworm AS builder

# Protobuf compiler ve reqwest/OpenSSL için gerekli bağımlılıkları kur
RUN apt-get update && \
    apt-get install -y protobuf-compiler libprotobuf-dev pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

# Çalışma dizinini ayarla
WORKDIR /app

# Workspace root dosyalarını kopyala
COPY Cargo.toml Cargo.lock ./

# Gateway projesini kopyala
COPY packages packages
COPY apps apps
COPY tools tools




# Gateway'i release modunda derle
RUN cargo build --release -p gateway

# 2. Runtime Stage
FROM debian:bookworm-slim

# HTTPS istekleri (reqwest) için sertifikaları kur
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Derlenen binary'i builder aşamasından kopyala
COPY --from=builder /app/target/release/gateway /app/gateway

# Gateway portunu dışarı aç
EXPOSE 8000

# Uygulamayı çalıştır
CMD ["./gateway"]
