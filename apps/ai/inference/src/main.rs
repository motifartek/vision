use std::sync::Arc;

use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use inference::api::{self, AppState};
use inference::config::{Config, PROFILES};
use inference::model;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "inference=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg = Config::from_env();
    tracing::info!(?cfg, "yapÄ±landÄ±rma yÃ¼klendi");

    let labels_path = cfg.models_dir.join(&cfg.model).join("class_labels_indices.csv");
    let labels = model::labels::load(&labels_path)?;

    let mut loaded = model::ced::load(&cfg)?;

    // Profillerin pencere boyutlarÄ± iÃ§in oturumu Ä±sÄ±t (GPU'da ÅŸekil baÅŸÄ±na
    // Ã§ekirdek derlemesi ilk isteÄŸi yavaÅŸlatÄ±yordu).
    let window_frames: Vec<usize> = PROFILES
        .iter()
        .map(|p| (p.window_sec * 100.0).round() as usize)
        .collect();
    let warmup_started = std::time::Instant::now();
    model::ced::warmup(&mut loaded.session, cfg.batch_size, &window_frames);
    tracing::info!(
        ms = warmup_started.elapsed().as_millis(),
        sekil = window_frames.len(),
        "model Ä±sÄ±tÄ±ldÄ±"
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
            "INFERENCE_MEDIA_ROOT ayarlÄ± deÄŸil; analyze uÃ§ noktasÄ± yerel dosya \
             sistemindeki herhangi bir yolu okuyabilir"
        );
    }

    // KÃ¶ken yerel arayÃ¼zle sÄ±nÄ±rlÄ±: servis zaten yalnÄ±z 127.0.0.1 dinliyor, yani
    // `permissive` yerel aÄŸa eriÅŸim kazandÄ±rmÄ±yordu â€” yalnÄ±zca herhangi bir web
    // sayfasÄ±nÄ±n tarayÄ±cÄ± Ã¼zerinden buraya istek atmasÄ±na (silme dahil) izin
    // veriyordu. Port serbest, Ã§Ã¼nkÃ¼ dashboard 3000 dÄ±ÅŸÄ±nda da Ã§alÄ±ÅŸabiliyor.
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin, _| {
            origin.to_str().map(api::is_local_origin).unwrap_or(false)
        }))
        .allow_methods(Any)
        .allow_headers(Any);

    // Video yÃ¼klemeleri iÃ§in boyut limitini tamamen kaldÄ±rÄ±yoruz (3GB, 10GB vs. sÄ±nÄ±rsÄ±z)
    let app = api::router(state)
        .layer(axum::extract::DefaultBodyLimit::disable())
        .layer(cors);

    let listener = tokio::net::TcpListener::bind((cfg.host, cfg.port)).await?;
    tracing::info!("inference servisi {} adresinde dinliyor", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}
