//! Paylaşılan servis durumu.

use std::collections::HashMap;
use std::sync::Arc;

use motif_core::VideoId;
use motif_optics::MotionProfile;
use tokio::sync::RwLock;

use crate::config::Config;
use crate::events::EventPublisher;
use crate::storage::BlobStore;

pub struct AppState {
    pub config: Config,
    pub store: Arc<dyn BlobStore>,
    pub events: EventPublisher,

    /// Hesaplanmış hareket profilleri.
    ///
    /// Yakınlaştırmanın ucuz olmasının sebebi bu önbellek: profil video başına
    /// bir kez çıkarılır, sonraki her `zoom_range` çağrısı videoyu tekrar
    /// çözmek yerine profilin kesitini alır.
    profiles: RwLock<HashMap<VideoId, Arc<MotionProfile>>>,

    /// Video başına yapılmış yakınlaştırma sayısı.
    zooms: RwLock<HashMap<VideoId, usize>>,
}

impl AppState {
    pub fn new(config: Config, store: Arc<dyn BlobStore>, events: EventPublisher) -> Self {
        Self {
            config,
            store,
            events,
            profiles: RwLock::new(HashMap::new()),
            zooms: RwLock::new(HashMap::new()),
        }
    }

    pub async fn cached_profile(&self, id: &VideoId) -> Option<Arc<MotionProfile>> {
        self.profiles.read().await.get(id).cloned()
    }

    pub async fn cache_profile(&self, id: VideoId, profile: Arc<MotionProfile>) {
        self.profiles.write().await.insert(id, profile);
    }

    /// Yakınlaştırma sayacını artırır; sınır aşıldıysa `false` döner.
    ///
    /// Ajan yakınlaşmaya kendi karar verdiği için üst sınır şart: kararsız bir
    /// model aynı aralığa tekrar tekrar girip gecikmeyi sınırsız büyütebilir.
    pub async fn try_consume_zoom(&self, id: &VideoId) -> bool {
        let mut zooms = self.zooms.write().await;
        let count = zooms.entry(id.clone()).or_insert(0);
        if *count >= self.config.max_zooms_per_video {
            return false;
        }
        *count += 1;
        true
    }

    pub async fn zoom_count(&self, id: &VideoId) -> usize {
        self.zooms.read().await.get(id).copied().unwrap_or(0)
    }

    /// Yakınlaştırma bütçesini sıfırlar (yeni bir analiz turu başlarken).
    pub async fn reset_zooms(&self, id: &VideoId) {
        self.zooms.write().await.remove(id);
    }

    pub async fn forget(&self, id: &VideoId) {
        self.profiles.write().await.remove(id);
        self.zooms.write().await.remove(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::LocalStore;

    async fn state(max_zooms: usize) -> AppState {
        let dir = std::env::temp_dir().join(format!("motif-state-{}", uuid::Uuid::new_v4()));
        let config = Config {
            max_zooms_per_video: max_zooms,
            ..Config::from_env()
        };
        AppState::new(
            config,
            Arc::new(LocalStore::new(dir).unwrap()),
            EventPublisher::connect(None).await,
        )
    }

    #[tokio::test]
    async fn yakinlastirma_siniri_uygulanir() {
        let state = state(3).await;
        let id = VideoId::new();

        for _ in 0..3 {
            assert!(state.try_consume_zoom(&id).await);
        }
        assert!(
            !state.try_consume_zoom(&id).await,
            "sınır aşıldıktan sonra reddedilmeliydi"
        );
        assert_eq!(state.zoom_count(&id).await, 3);
    }

    #[tokio::test]
    async fn sinir_video_basina_ayri_tutulur() {
        let state = state(1).await;
        let a = VideoId::new();
        let b = VideoId::new();

        assert!(state.try_consume_zoom(&a).await);
        assert!(!state.try_consume_zoom(&a).await);
        // Başka bir videonun bütçesi etkilenmemeli.
        assert!(state.try_consume_zoom(&b).await);
    }

    #[tokio::test]
    async fn sifirlama_butceyi_geri_verir() {
        let state = state(1).await;
        let id = VideoId::new();

        assert!(state.try_consume_zoom(&id).await);
        assert!(!state.try_consume_zoom(&id).await);

        state.reset_zooms(&id).await;
        assert!(state.try_consume_zoom(&id).await);
    }
}
