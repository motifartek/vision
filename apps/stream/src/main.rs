//! `apps/stream` — video alma ve dinamik kare örnekleme servisi.
//!
//! Görevi: yüklenen bir videoyu **güvenlikle ilgili tüm olayları koruyacak
//! şekilde mümkün olan en az kareye** indirmek ve bu yeteneği ajana
//! çağrılabilir araçlar olarak sunmak.
//!
//! Servis hiçbir altyapı olmadan ayağa kalkar: nesne deposu varsayılan olarak
//! yerel dosya sistemi, NATS isteğe bağlı. `cargo run -p stream` yeterli.
//!
//! Yol haritası: `documents/architecture/stream-phase-plan.md`

mod api;
mod catalog;
mod config;
mod events;
mod nats;
mod pipeline;
mod state;
mod storage;
mod tools;

use std::sync::Arc;

use anyhow::{Context, Result};
use motif_optics::check_dependencies;

use crate::config::Config;
use crate::events::EventPublisher;
use crate::state::AppState;
use crate::storage::LocalStore;

#[tokio::main]
async fn main() -> Result<()> {
    motif_core::telemetry::init("stream=debug,motif_optics=info,tower_http=info");

    // Eksik bir bağımlılık ilk video yüklendiğinde değil, açılışta anlaşılsın.
    for (tool, version) in check_dependencies().context("harici bağımlılık kontrolü")? {
        tracing::info!(tool = tool.binary(), %version, "harici bağımlılık hazır");
    }

    let config = Config::from_env();
    tracing::info!(
        storage = %config.storage_root.display(),
        overview_budget = config.overview_budget,
        zoom_budget = config.zoom_budget,
        "yapılandırma yüklendi"
    );

    let store = Arc::new(
        LocalStore::new(&config.storage_root).context("nesne deposu açılamadı")?,
    );
    let publisher = EventPublisher::connect(config.nats_url.as_deref()).await;

    let bind = config.bind.clone();
    let state = Arc::new(AppState::new(config, store, publisher));

    // Araçlar NATS üzerinden de çağrılabilir; HTTP ile aynı gövdeyi kullanır.
    nats::serve_tools(state.clone());

    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("{bind} dinlenemedi"))?;

    tracing::info!(addr = %listener.local_addr()?, "stream servisi dinleniyor");

    axum::serve(listener, api::router(state))
        .await
        .context("sunucu düştü")?;

    Ok(())
}
