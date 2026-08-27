# MotifAI Dokümantasyon

MotifAI projesinin mimari kararları, teknik spesifikasyonları ve servis tanımları bu dizinde yer almaktadır.

## 0. Genel Bakış (Overview)
- [Prompt Sistemi · Prompt System](prompt-system-overview.md) - Modele giden metnin nasıl üretildiği, düzenlendiği ve doğrulandığı; iletişim grafikleriyle, TR ve EN.

## 1. Servis Özellikleri (Features)
Uygulamanın temel mikroservislerinin ne işe yaradığı, nasıl kurulduğu ve API uç noktaları bu belgelerde açıklanmıştır:
- [Stream Servisi](features/stream-service.md) - Video alma ve dinamik kare örnekleme servisi.
- [Sonic Servisi](features/sonic-service.md) - Ses kanalı üzerinden 527 sınıflı olay algılama ve İSG kural motoru servisi (eski adıyla Inference).

## 2. Mimari Kararlar ve Benchmarklar (Architecture)
Sistemin donanım, performans ve gözlemlenebilirlik altyapısının test sonuçları ve planları:
- [Gözlemlenebilirlik (Observability)](architecture/observability.md) - OTel, Grafana, Loki ve Prometheus altyapısı kurulumu.
- [Stream Benchmark](architecture/stream-benchmark.md) - Stream servisinin %100 recall ve 78x azaltma ile gerçek zamanlı kare işleme analizi.
- [Stream Phase Plan](architecture/stream-phase-plan.md) - Stream servisi için 8 aşamalı geliştirme yol haritası.
- [VLM Benchmark](architecture/vlm-benchmark.md) - Multimodal LLM (Qwen, Gemma vb.) modellerinin 778 görsel üzerinde yapılan test sonuçları ve model seçim kararı.
- [Prompt Sistemi Tasarımı](architecture/prompt-system.md) - Prompt kataloğu, override katmanı ve güvenilmez bölge için karar kayıtları ve faz planı.

> Not: Benchmark test scriptleri (`benchmark.py` vb.) `tools/bench` dizini altında bulunmaktadır.