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
    model::ced::warmup(&mut loaded.session, cfg.batch_size, &window_frames);
    tracing::info!(
        ms = warmup_started.elapsed().as_millis(),
        sekil = window_frames.len(),
        "model ısıtıldı"
    );

    let state = Arc::new(AppState::new(
        &cfg,
        labels,
        loaded.session,
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

    // Video yüklemeleri için boyut limitini tamamen kaldırıyoruz (3GB, 10GB vs. sınırsız)
    let app = api::router(state)
        .layer(axum::extract::DefaultBodyLimit::disable())
        .layer(cors);

    let listener = tokio::net::TcpListener::bind((cfg.host, cfg.port)).await?;
    tracing::info!("sonic servisi {} adresinde dinliyor", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}
