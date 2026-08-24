# Observability Stack (EN)

This document explains the architecture and usage of the OpenTelemetry (OTel), Prometheus, Loki, Tempo, and Grafana observability stack integrated into the MotifAI project.

## What is it and What does it do?

Observability allows us to understand the internal state of our system through its external outputs. In multi-microservice architectures like MotifAI (Gateway, Stream, Inference, etc.), pinpointing the root cause of issues is difficult. This stack centralizes the "Three Pillars of Observability":

1. **Metrics (Prometheus):** Indicates how hard your system is working. Provides numerical data such as requests per second (RPS), error rates, and response times.
2. **Logs (Loki):** Text-based records of events occurring in the system.
3. **Traces (Tempo):** Chronological waterfall graphs that show the journey of a user's action across different services and how long each step took.

With this stack, if the Gateway returns a 500 Internal Server Error, we can see within seconds if the root cause was a timeout in the Inference service.

## Architecture and Components

* **motif-observer:** A Rust crate located under packages/observer. It is the shared OpenTelemetry initializer used by all services.
* **OTel Collector:** Collects raw data from services (via gRPC/HTTP OTLP) and distributes it to the respective backends (Loki, Tempo, Prometheus).
* **Grafana:** The main UI exposing all this data, running on port 5000.

## How to Use?

To start the system in a local development environment:

`ash
docker compose -f platform/docker/compose.yaml build
docker compose -f platform/docker/compose.yaml up -d
`

Navigate to http://localhost:5000 in your browser and open the "MotifAI - Overview" dashboard.
To add metrics to new code, simply use the xum_prometheus layer in your Rust service and add motif_observer::init("service-name"); at the start of the application.

## Behavior in Dev vs Prod Environments

### Development (Dev)
* Services send data to local containers defined in platform/observability/compose.yaml.
* Data is written to local disk (volumes) and is transient.
* Grafana auto-provisions datasources and dashboards, and allows anonymous admin access.
* If the OTEL_EXPORTER_OTLP_ENDPOINT environment variable is unset, motif-observer falls back to printing colored logs to standard output with zero OTLP overhead.

### Production (Prod)
* **Zero code changes:** The application code remains identical. Only the OTel Collector configuration changes.
* The OTel Collector can route telemetry data to cloud providers (Datadog, New Relic, AWS X-Ray, Grafana Cloud, etc.) instead of local Tempo/Loki instances.
* Log retention is managed using persistent cloud storage solutions (like AWS S3 or Google Cloud Storage) backed by Loki.
* Grafana anonymous access is disabled, and enterprise authentication (OAuth/SSO) is enforced.

# Gözlemlenebilirlik Altyapısı (TR)

Bu belge, MotifAI projesine entegre edilen OpenTelemetry (OTel), Prometheus, Loki, Tempo ve Grafana gözlemlenebilirlik yığınının mimarisini ve kullanımını açıklamaktadır.

## Nedir ve Ne İşe Yarar?

Gözlemlenebilirlik (Observability), sistemimizin iç durumunu ürettiği dış çıktılar aracılığıyla anlamamızı sağlar. MotifAI gibi çoklu mikroservis (Gateway, Stream, Inference vb.) içeren mimarilerde sorunların kaynağını bulmak zordur. Bu altyapı şu 3 temel sütunu (Three Pillars of Observability) tek bir merkezde toplar:

1. **Metrikler (Prometheus):** Sisteminizin ne kadar yorulduğunu gösterir. Saniyedeki istek sayısı (RPS), hata oranları ve yanıt süreleri gibi sayısal veriler sunar.
2. **Loglar (Loki):** Sistemde meydana gelen olayların metinsel dökümüdür.
3. **İzlemeler/Traces (Tempo):** Bir kullanıcının eyleminin hangi servislerden geçtiğini, nerede ne kadar beklediğini gösteren kronolojik şelale (waterfall) grafikleridir.

Bu yığın sayesinde, örneğin Gateway'de 500 Internal Server Error döndüren bir hatanın asıl sebebinin Inference servisindeki bir zaman aşımı (timeout) olduğunu saniyeler içinde görebiliriz.

## Mimari ve Bileşenler

* **motif-observer:** packages/observer altında bulunan Rust paketidir. Tüm servislerin ortak kullandığı OpenTelemetry başlatıcısıdır.
* **OTel Collector:** Servislerden (gRPC/HTTP OTLP üzerinden) ham verileri toplar ve ilgili hedeflere (Loki, Tempo, Prometheus) dağıtır.
* **Grafana:** Tüm bu verileri http://localhost:5000 portundan sunan ana arayüzdür.

## Nasıl Kullanılır?

Yerel geliştirme ortamında sistemi ayağa kaldırmak için:

`ash
docker compose -f platform/docker/compose.yaml build
docker compose -f platform/docker/compose.yaml up -d
`

Tarayıcıdan http://localhost:5000 adresine girerek "MotifAI - Genel Bakış" (Overview) dashboard'unu açabilirsiniz.
Mevcut kodunuza metrik eklemek isterseniz, ilgili Rust servisinde xum_prometheus layer'ını kullanmanız ve projenin başlangıcına motif_observer::init("servis-adi"); eklemeniz yeterlidir.

## Geliştirme (Dev) vs Üretim (Prod) Ortamlarında Davranış

### Geliştirme Ortamı (Dev)
* Servisler platform/observability/compose.yaml içerisindeki lokal konteynerlere veri gönderir.
* Veriler lokal diske (volume) yazılır ve geçicidir.
* Grafana otomatik olarak datasourceları ve dashboardları tanımlar (provisioning), şifre sormaz (anonim admin aktiftir).
* OTEL_EXPORTER_OTLP_ENDPOINT ortam değişkeni olmazsa, motif-observer sessizce kapanır ve sadece terminale renkli log basar (overhead yaratmaz).

### Üretim Ortamı (Prod)
* **Koda dokunulmaz:** Uygulama kodu üretimde (Prod) hiçbir değişikliğe uğramaz. Sadece OTel Collector yapılandırması (config) değiştirilir.
* OTel Collector, verileri lokal Tempo/Loki yerine bulut sağlayıcılarına (Datadog, New Relic, AWS X-Ray, Grafana Cloud vb.) yönlendirebilir.
* Üretimde log saklama (retention) süreleri S3 veya GCS gibi kalıcı depolama birimleriyle (cloud storage) yönetilir.
* Grafana'da anonim erişim kapatılır, OAuth/SSO ile giriş zorunlu kılınır.