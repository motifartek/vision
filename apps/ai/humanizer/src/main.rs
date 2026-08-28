use std::sync::Arc;

use anyhow::{Context, Result};

use motif_prompt::PromptRegistry;
use humanizer::agent::HumanizerAgent;
use humanizer::api;
use async_nats;

use humanizer::llm::EvrenProvider;
use humanizer::db::ChatStore;

#[tokio::main]
async fn main() -> Result<()> {
    motif_observer::init("humanizer");

    let bind = std::env::var("HUMANIZER_BIND").unwrap_or_else(|_| "0.0.0.0:8115".into());
    let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".into());
    let db_url = std::env::var("DATABASE_URL").context("DATABASE_URL gerekli")?;

    let vlm = EvrenProvider::from_env().context("çkarım servisi istemcisi")?;
    let nats = async_nats::connect(&nats_url).await.context("NATS bağlantısı")?;
    
    let pool = motif_database::connect(&db_url).await.context("Veritabanı bağlantısı")?;
    let chat_store = Arc::new(ChatStore::new(pool.clone()));

    let katalog = PromptRegistry::from_env_or_embedded().context("prompt kataloğu")?;
    let store = Arc::new(motif_database::PostgresPromptStore::new(pool));
    tracing::info!("prompt override deposu bağlandı");
    let katalog = katalog.with_store(store).await;
    
    let prompts = Arc::new(katalog);
    tracing::info!("prompt kataloğu yüklendi");

    let agent = Arc::new(HumanizerAgent::new(
        Arc::new(vlm),
        prompts,
        nats,
        chat_store,
    ));

    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("{bind} dinlenemedi"))?;

    tracing::info!(addr = %listener.local_addr()?, "humanizer servisi dinleniyor");

    axum::serve(listener, api::router(agent))
        .await
        .context("sunucu düştü")?;

    Ok(())
}