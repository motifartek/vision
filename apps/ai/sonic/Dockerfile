# Varsayilan CPU. GPU'yu compose.gpu.yaml acar; boylece NVIDIA karti olmayan
# makinelerde hicbir sey degismez ve `up` her yerde ayni sekilde calisir.
#
# RUNTIME_IMAGE ilk FROM'dan once tanimli olmali, yoksa asagida kullanilamaz.
ARG RUNTIME_IMAGE=ubuntu:24.04

FROM ubuntu:24.04 AS builder

# Bos = CPU. `cuda` verilirse ort CUDA saglayicisini derliyor. ONNX Runtime
# saglayiciyi bulamazsa sessizce CPU'ya doner, yani ayni ikili GPU'suz
# makinede de calisir.
ARG SONIC_FEATURES=

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

# Sonic'i derle.
#
# Cache mount'lar olmadan `COPY apps apps` katmani her kaynak degisikliginde
# bozuluyor ve *butun* bagimliliklar bastan derleniyordu: tek satirlik bir log
# duzeltmesi ~15 dakika suruyordu. Registry, git ve target dizinleri build'ler
# arasi saklaniyor, yani yalnizca degisen crate yeniden derleniyor.
#
# `target` bir cache mount oldugu icin icerigi imaj katmanina yazilmiyor —
# ikiliyi ayni RUN icinde disari kopyalamak zorundayiz, yoksa sonraki asama
# bulamaz.
RUN --mount=type=cache,target=/root/.cargo/registry,sharing=locked \
    --mount=type=cache,target=/root/.cargo/git,sharing=locked \
    --mount=type=cache,target=/app/target,sharing=locked \
    cargo build --release -p sonic ${SONIC_FEATURES:+--features "$SONIC_FEATURES"} && \
    cp /app/target/release/sonic /usr/local/bin/sonic

# ---- Stage 2: Runtime ----
# Varsayilan `ubuntu:24.04`. GPU kurulumunda compose.gpu.yaml burayi
# `nvidia/cuda:...-cudnn-runtime-ubuntu24.04` yapiyor: CUDA surucusu host'ta
# kaliyor, konteynere yalniz calisma kutuphaneleri giriyor.
FROM ${RUNTIME_IMAGE}

# `curl`, model agirliklarini indiren `fetch-models.sh` icin: ayni imaj
# `sonic-models-init` servisi olarak da kosuyor (bkz. compose.yaml).
RUN apt-get update && \
    apt-get install -y \
        ffmpeg \
        ca-certificates \
        curl \
        && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /usr/local/bin/sonic /app/sonic

# Model indirme scripti imajin icinde: agirliklar depoda tutulmadigi icin
# konteyner kendi agirliklarini kendisi cekebilmeli. Ana makinede ayni script
# `apps/ai/sonic/scripts/fetch-models.sh` olarak da calisiyor.
COPY apps/ai/sonic/scripts/fetch-models.sh /app/fetch-models.sh
# `core.autocrlf=true` olan Windows checkout'unda dosya CRLF'le geliyor ve build
# context calisma agacindan okundugu icin bu satir sonlari imaja tasiniyor.
# Shebang `#!/bin/sh\r` olunca Linux yorumlayiciyi bulamiyor ve hata "dosya yok"
# diye cikiyor — scriptin kendisi oradayken. Kopyada duzeltiyoruz.
RUN sed -i 's/\r$//' /app/fetch-models.sh && chmod +x /app/fetch-models.sh

# Model agirliklari icin dizin
RUN mkdir -p /app/models

EXPOSE 8081

ENV SONIC_PORT=8081
ENV SONIC_MODELS_DIR=/app/models

CMD ["./sonic"]
