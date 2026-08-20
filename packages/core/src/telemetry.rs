use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Yapılandırılmış loglamayı kurar.
///
/// `RUST_LOG` ortam değişkeni varsa o kullanılır, yoksa `default_filter`.
/// Birden fazla kez çağrılırsa sessizce yok sayılır (testler için).
pub fn init(default_filter: &str) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_filter));

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .try_init();
}
