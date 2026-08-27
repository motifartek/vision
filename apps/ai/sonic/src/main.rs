use std::sync::Arc;

use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use sonic::api::{self, AppState};
use sonic::config::{Config, PROFILES};
use sonic::model;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            // Hedef adı crate adıdır. Burada `inference` yazıyordu — crate
            // `sonic` olarak yeniden adlandırıldığında bu dize güncellenmedi ve
            // direktif hiçbir şeyle eşleşmez oldu. EnvFilter eşleşmeyen hedefleri
            // ERROR'a düşürdüğü için servis konteynerde tamamen sustu: görülen
            // tek satır, `main` Err döndüğünde Rust'ın kendisinin bastığı ölümcül
            // hataydı. Düzgün çalışırken hiçbir iz bırakmıyordu.
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sonic=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg = Config::from_env();
    tracing::info!(?cfg, "yapılandırma yüklendi");

    let labels_path = cfg.models_dir.join(&cfg.model).join("class_labels_indices.csv");
    let labels = model::labels::load(&labels_path)?;

    let mut loaded = model::ced::load(&cfg)?;

    // Profillerin pencere boyutları için oturumu ısıt (GPU'da şekil başına
    // çekirdek derlemesi ilk isteği yavaşlatıyordu).
    let window_frames: Vec<usize> = PROFILES
        .iter()
        .map(|p| (p.window_sec * 100.0).round() as usize)
        .collect();
    let warmup_started = std::time::Instant::now();
    model::ced::warmup(&mut loaded.backend, cfg.batch_size, &window_frames);
    tracing::info!(
        ms = warmup_started.elapsed().as_millis(),
        sekil = window_frames.len(),
        "model ısıtıldı"
    );

    let state = Arc::new(AppState::new(
        &cfg,
        labels,
        loaded.backend,
        loaded.model_name,
        loaded.weights_file,
        loaded.providers,
    ));

    if cfg.media_root.is_none() {
        tracing::warn!(
            "SONIC_MEDIA_ROOT ayarlı değil; analyze uç noktası yerel dosya \
             sistemindeki herhangi bir yolu okuyabilir"
        );
    }

    // Köken yerel arayüzle sınırlı: servis zaten yalnız 127.0.0.1 dinliyor, yani
    // `permissive` yerel ağa erişim kazandırmıyordu — yalnızca herhangi bir web
    // sayfasının tarayıcı üzerinden buraya istek atmasına (silme dahil) izin
    // veriyordu. Port serbest, çünkü dashboard 3000 dışında da çalışabiliyor.
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin, _| {
            origin.to_str().map(api::is_local_origin).unwrap_or(false)
        }))
        .allow_methods(Any)
        .allow_headers(Any);

    // Prometheus katmanı ve /metrics ucu. Ölçüm katmanı en dışta duruyor ki
    // /metrics dahil her isteği saysın.
    //
    // `platform/observability/prometheus.yaml` bu servisi baştan beri kazıyordu
    // ama rota hiç eklenmemişti: her kazıma 404 dönüyor, sonic'in tek bir
    // metriği bile Prometheus'a ulaşmıyordu. Sessizce başarısızdı — istek
    // logları olmadığı için kimse görmedi.
    let (prometheus_layer, metric_handle) = axum_prometheus::PrometheusMetricLayer::pair();

    // Video yüklemeleri için boyut limitini tamamen kaldırıyoruz (3GB, 10GB vs. sınırsız)
    let app = api::router(state)
        .route(
            "/metrics",
            axum::routing::get(|| async move { metric_handle.render() }),
        )
        .layer(axum::extract::DefaultBodyLimit::disable())
        .layer(cors)
        .layer(prometheus_layer);

    if !cfg.bind.ip().is_loopback() {
        tracing::warn!(
            adres = %cfg.bind,
            "servis loopback dışında dinliyor; kendi kimlik doğrulaması yok, \
             bu adrese erişebilen herkes analiz ve silme çağırabilir"
        );
    }

    let listener = tokio::net::TcpListener::bind(cfg.bind).await?;
    tracing::info!("sonic servisi {} adresinde dinliyor", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}
