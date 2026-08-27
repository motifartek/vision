//! `apps/ai/vision` — video analiz ajanı servisi.
//!
//! Şartnamenin istediği çıktıyı üreten servis budur: bir video kimliği alır,
//! `stream`'den klip ister, çıkarım servisine sorar ve `{summary, events,
//! risk, actions}` raporunu döndürür.
//!
//! `stream` gibi altyapısız ayağa kalkar: NATS, veritabanı ya da nesne deposu
//! gerekmez. Tek zorunlu şey `EVREN_KEY` ortam değişkeni ve ayakta bir
//! `stream` servisi.
//!
//! ```text
//! EVREN_KEY=... cargo run -p vision
//! curl -X POST localhost:8110/v1/analyze -H 'content-type: application/json' \
//!      -d '{"video_id":"..."}'
//! ```

mod agent;
mod api;
mod stream_client;
mod vlm;

use std::sync::Arc;

use anyhow::{Context, Result};

use crate::agent::VisionAgent;
use crate::stream_client::StreamClient;
use crate::vlm::EvrenProvider;

#[tokio::main]
async fn main() -> Result<()> {
    motif_observer::init("vision");

    let bind = std::env::var("VISION_BIND").unwrap_or_else(|_| "0.0.0.0:8110".into());
    let stream_url =
        std::env::var("STREAM_URL").unwrap_or_else(|_| "http://127.0.0.1:8100".into());

    // Anahtar eksikse ilk analiz isteğinde değil, açılışta anlaşılsın.
    let vlm = EvrenProvider::from_env().context("çıkarım servisi istemcisi")?;
    let stream = StreamClient::new(&stream_url).context("stream istemcisi")?;

    tracing::info!(%stream_url, "stream servisi hedefi");

    let agent = Arc::new(VisionAgent::new(Arc::new(stream), Arc::new(vlm)));

    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("{bind} dinlenemedi"))?;

    tracing::info!(addr = %listener.local_addr()?, "vision servisi dinleniyor");

    axum::serve(listener, api::router(agent))
        .await
        .context("sunucu düştü")?;

    Ok(())
}
