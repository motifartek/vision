//! NATS olay yayını.
//!
//! Yayıncı **isteğe bağlıdır**: `NATS_URL` verilmezse servis broker olmadan
//! tam işlevsel çalışır ve yayınlar sessizce yutulur. Bunun sebebi kolaycılık
//! değil, geliştirme akışı: test arayüzünde bir video üzerinde çalışmak için
//! altyapı ayağa kaldırmak gerekmesin. Kontratlar (`motif-event-sdk`) her iki
//! durumda da aynı.

use motif_event_sdk::{subjects, FrameExtracted, VideoIngested};
use serde::Serialize;

/// Olay yayıncısı.
#[derive(Clone)]
pub struct EventPublisher {
    client: Option<async_nats::Client>,
}

impl EventPublisher {
    /// Bağlantı kurar. URL yoksa ya da bağlantı kurulamazsa devre dışı kalır.
    ///
    /// Bağlantı hatası servisi düşürmez: olay yayını yan etkidir, video
    /// analizinin kendisi buna bağlı değil. Broker sonradan gelirse servis
    /// yeniden başlatılır.
    pub async fn connect(url: Option<&str>) -> Self {
        let Some(url) = url else {
            tracing::info!("NATS_URL verilmedi; olay yayını devre dışı");
            return Self { client: None };
        };

        match async_nats::connect(url).await {
            Ok(client) => {
                tracing::info!(%url, "NATS bağlantısı kuruldu");
                Self {
                    client: Some(client),
                }
            }
            Err(err) => {
                tracing::warn!(%url, %err, "NATS bağlantısı kurulamadı; olay yayını devre dışı");
                Self { client: None }
            }
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.client.is_some()
    }

    pub fn client(&self) -> Option<&async_nats::Client> {
        self.client.as_ref()
    }

    async fn publish<T: Serialize>(&self, subject: &'static str, payload: &T) {
        let Some(client) = &self.client else {
            return;
        };

        let bytes = match serde_json::to_vec(payload) {
            Ok(b) => b,
            Err(err) => {
                tracing::error!(subject, %err, "olay serileştirilemedi");
                return;
            }
        };

        if let Err(err) = client.publish(subject, bytes.into()).await {
            // Yayın başarısızlığı analizi geçersiz kılmaz; yalnız loglanır.
            tracing::warn!(subject, %err, "olay yayımlanamadı");
        }
    }

    pub async fn video_ingested(&self, event: &VideoIngested) {
        self.publish(subjects::VIDEO_INGESTED, event).await;
    }

    pub async fn frames_extracted(&self, event: &FrameExtracted) {
        self.publish(subjects::FRAME_EXTRACTED, event).await;
    }
}
