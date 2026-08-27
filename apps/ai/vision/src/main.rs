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

use std::sync::Arc;

use anyhow::{Context, Result};

use motif_prompt::PromptRegistry;
use vision::agent::VisionAgent;
use vision::api;
use vision::stream_client::StreamClient;
use vision::vlm::EvrenProvider;

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

    // Katalog açılışta yükleniyor: bozuk bir prompt açılışta patlamalı,
    // analiz sırasında değil.
    let katalog = PromptRegistry::from_env_or_embedded().context("prompt kataloğu")?;

    // Override deposu **isteğe bağlı**. DATABASE_URL yoksa ya da veritabanına
    // ulaşılamıyorsa uyarı loglanıp gömülü katalogla devam ediliyor: prompt'un
    // çalışma anı bağımlılığı olması yeni bir düşme yolu demek olurdu.
    let katalog = match std::env::var("DATABASE_URL") {
        Ok(url) => match motif_database::connect(&url).await {
            Ok(pool) => {
                let store = Arc::new(motif_database::PostgresPromptStore::new(pool));
                tracing::info!("prompt override deposu bağlandı");
                katalog.with_store(store).await
            }
            Err(e) => {
                tracing::warn!(hata = %e, "veritabanına bağlanılamadı; override'lar devre dışı");
                katalog
            }
        },
        Err(_) => {
            tracing::info!("DATABASE_URL yok; yalnızca gömülü katalog");
            katalog
        }
    };

    let prompts = Arc::new(katalog);
    tracing::info!("prompt kataloğu yüklendi");

    let agent = Arc::new(VisionAgent::new(
        Arc::new(stream),
        Arc::new(vlm),
        prompts,
    ));

    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("{bind} dinlenemedi"))?;

    tracing::info!(addr = %listener.local_addr()?, "vision servisi dinleniyor");

    axum::serve(listener, api::router(agent))
        .await
        .context("sunucu düştü")?;

    Ok(())
}
