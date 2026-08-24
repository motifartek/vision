//! `apps/stream` â€” video alma ve dinamik kare Ã¶rnekleme servisi.
//!
//! GÃ¶revi: yÃ¼klenen bir videoyu **gÃ¼venlikle ilgili tÃ¼m olaylarÄ± koruyacak
//! ÅŸekilde mÃ¼mkÃ¼n olan en az kareye** indirmek ve bu yeteneÄŸi ajana
//! Ã§aÄŸrÄ±labilir araÃ§lar olarak sunmak.
//!
//! Servis hiÃ§bir altyapÄ± olmadan ayaÄŸa kalkar: nesne deposu varsayÄ±lan olarak
//! yerel dosya sistemi, NATS isteÄŸe baÄŸlÄ±. `cargo run -p stream` yeterli.
//!
//! Yol haritasÄ±: `documents/architecture/stream-phase-plan.md`

mod api;
mod catalog;
mod config;
mod events;
mod nats;
mod payload;
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
    motif_observer::init("stream");

    // Eksik bir baÄŸÄ±mlÄ±lÄ±k ilk video yÃ¼klendiÄŸinde deÄŸil, aÃ§Ä±lÄ±ÅŸta anlaÅŸÄ±lsÄ±n.
    for (tool, version) in check_dependencies().context("harici baÄŸÄ±mlÄ±lÄ±k kontrolÃ¼")? {
        tracing::info!(tool = tool.binary(), %version, "harici baÄŸÄ±mlÄ±lÄ±k hazÄ±r");
    }

    let config = Config::from_env();
    tracing::info!(
        storage = %config.storage_root.display(),
        overview_budget = config.overview_budget,
        zoom_budget = config.zoom_budget,
        "yapÄ±landÄ±rma yÃ¼klendi"
    );

    let store = Arc::new(
        LocalStore::new(&config.storage_root).context("nesne deposu aÃ§Ä±lamadÄ±")?,
    );
    let publisher = EventPublisher::connect(config.nats_url.as_deref()).await;

    let bind = config.bind.clone();
    let state = Arc::new(AppState::new(config, store, publisher));

    // AraÃ§lar NATS Ã¼zerinden de Ã§aÄŸrÄ±labilir; HTTP ile aynÄ± gÃ¶vdeyi kullanÄ±r.
    nats::serve_tools(state.clone());

    let (prometheus_layer, metric_handle) = axum_prometheus::PrometheusMetricLayer::pair();
    
    let app = api::router(state)
        .route("/metrics", axum::routing::get(|| async move { metric_handle.render() }))
        .layer(prometheus_layer);

    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("{bind} dinlenemedi"))?;

    tracing::info!(addr = %listener.local_addr()?, "stream servisi dinleniyor");

    axum::serve(listener, api::router(state))
        .await
        .context("sunucu dÃ¼ÅŸtÃ¼")?;

    Ok(())
}
