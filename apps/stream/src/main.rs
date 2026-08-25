//! `apps/stream` — video alma ve dinamik kare ornekleme servisi.
//!
//! Gorevi: yuklenen bir videoyu **guvenlikle ilgili tum olaylari koruyacak
//! sekilde mumkun olan en az kareye** indirmek ve bu yetenegi ajana
//! cagrilabilir araclar olarak sunmak.
//!
//! Servis hicbir altyapi olmadan ayaga kalkar: nesne deposu varsayilan olarak
//! yerel dosya sistemi, NATS istege bagli. `cargo run -p stream` yeterli.
//!
//! Yol haritasi: `documents/architecture/stream-phase-plan.md`

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

    // Eksik bir bagimlilik ilk video yuklendiginde degil, acilista anlasilsin.
    for (tool, version) in check_dependencies().context("harici bagimlilik kontrolu")? {
        tracing::info!(tool = tool.binary(), %version, "harici bagimlilik hazir");
    }

    let config = Config::from_env();
    tracing::info!(
        storage = %config.storage_root.display(),
        overview_budget = config.overview_budget,
        zoom_budget = config.zoom_budget,
        "yapilandirma yuklendi"
    );

    let store = Arc::new(
        LocalStore::new(&config.storage_root).context("nesne deposu acilamadi")?,
    );
    let publisher = EventPublisher::connect(config.nats_url.as_deref()).await;

    let bind = config.bind.clone();
    let state = Arc::new(AppState::new(config, store, publisher));

    // Araclar NATS uzerinden de cagrilabilir; HTTP ile ayni govdeyi kullanir.
    nats::serve_tools(state.clone());

    let (prometheus_layer, metric_handle) = axum_prometheus::PrometheusMetricLayer::pair();

    // api::router ile prometheus_layer'i birlestiriyoruz.
    // Onceki hatada api::router(state) hem burada hem axum::serve icinde
    // cagriliyordu — ikinci cagri prometheus katmanindan gecmiyordu.
    let app = api::router(state)
        .route("/metrics", axum::routing::get(|| async move { metric_handle.render() }))
        .layer(prometheus_layer);

    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("{bind} dinlenemedi"))?;

    tracing::info!(addr = %listener.local_addr()?, "stream servisi dinleniyor");

    let axum_server = axum::serve(listener, app);
    tokio::select! {
        res = axum_server => {
            res.context("sunucu dustu")?;
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Kapatiliyor...");
        }
    }

    motif_observer::shutdown();
    Ok(())
}