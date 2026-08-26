//! Merkezi telemetri başlatıcı.
//!
//! Tüm servisler motif_observer::init("servis-adı") ile başlatır;
//! bu fonksiyon tek çağrıda:
//!   - Yapılandırılmış stdout loglarını (fmt layer)
//!   - OTLP üzerinden trace ihracını (OTel Collector -> Tempo)
//!
//! OTEL_EXPORTER_OTLP_ENDPOINT ortam değişkeni yoksa hiçbir şey
//! ihraç edilmez — servis lokal çalışmaya devam eder.

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    propagation::TraceContextPropagator,
    runtime,
    trace::{RandomIdGenerator, Sampler},
    Resource,
};
use opentelemetry::KeyValue;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Telemetriyi başlatır.
///
/// service_name: "gateway", "stream", "inference" gibi.
/// OTEL_EXPORTER_OTLP_ENDPOINT ayarlıysa OTLP ihracatı etkin olur;
/// ayarlı değilse sadece stdout loglama yapılır.
pub fn init(service_name: &str) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("{service_name}=debug,tower_http=debug,info")));

    let otlp_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok();

    if let Some(endpoint) = otlp_endpoint {
        opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());

        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()
            .expect("OTel span exporter kurulamadı");

        let resource = Resource::new(vec![
            KeyValue::new("service.name", service_name.to_string()),
        ]);

        let provider = opentelemetry_sdk::trace::TracerProvider::builder()
            .with_sampler(Sampler::AlwaysOn)
            .with_id_generator(RandomIdGenerator::default())
            .with_resource(resource)
            .with_batch_exporter(exporter, runtime::Tokio)
            .build();

        let tracer = provider.tracer(service_name.to_string());
        opentelemetry::global::set_tracer_provider(provider);

        let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

        let _ = tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer())
            .with(otel_layer)
            .try_init();

        tracing::info!(
            service = service_name,
            "OTel tracing etkin: OTLP aktarımı başlatıldı"
        );
    } else {
        let _ = tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer())
            .try_init();
    }
}

pub fn shutdown() {
    opentelemetry::global::shutdown_tracer_provider();
}