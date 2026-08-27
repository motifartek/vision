use futures::StreamExt;
use motif_event_sdk::{subjects, VideoIngested};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    motif_observer::init("orchestrator");
    tracing::info!("Orchestrator servisi başlatılıyor...");

    let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    
    let mut retries = 5;
    let client = loop {
        match async_nats::connect(&nats_url).await {
            Ok(c) => break c,
            Err(e) => {
                retries -= 1;
                if retries == 0 {
                    tracing::error!("NATS bağlantısı kurulamadı: {}", e);
                    return Err(e.into());
                }
                tracing::warn!("NATS'a bağlanılamadı, 2 saniye sonra tekrar deneniyor...");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    };

    tracing::info!("NATS'a bağlanıldı!");

    let mut subscriber = client.subscribe(subjects::VIDEO_INGESTED).await?;
    tracing::info!("{} dinleniyor...", subjects::VIDEO_INGESTED);

    while let Some(message) = subscriber.next().await {
        match serde_json::from_slice::<VideoIngested>(&message.payload) {
            Ok(event) => {
                tracing::info!(
                    video_id = %event.video_id,
                    object_key = %event.object_key,
                    "Yeni video yüklendi. (İleride Sonic/VLM servisleri buradan tetiklenecek!)"
                );
            }
            Err(e) => {
                tracing::error!("Geçersiz VideoIngested mesajı: {}", e);
            }
        }
    }

    motif_observer::shutdown();
    Ok(())
}
