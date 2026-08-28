//! `apps/stream` servisinin HTTP istemcisi.
//!
//! Ajanın gördüğü her kare buradan geliyor. Sahte veri yok: klip gerçekten
//! üretiliyor, indiriliyor ve modele o baytlar gidiyor.

use motif_event_sdk::{ClipRef, ClipResponse, VideoInfoResponse, ZoomRangeRequest};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    #[error("stream servisine ulaşılamadı ({url}): {source}")]
    Transport {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("stream {status} döndü: {body}")]
    Status { status: u16, body: String },
    #[error("stream cevabı çözümlenemedi: {0}")]
    Decode(String),
}

/// Ajanın klip kaynağı.
///
/// Trait olmasının sebebi test edilebilirlik: ajanın yakınlaştırma döngüsü,
/// ne ağ ne ffmpeg gerektirmeden sınanabiliyor. Modelin canlı koşuda
/// yakınlaştırma istemediği ölçüldü (10 videonun onunda da ilk turda rapor
/// verdi), yani bu yol yalnızca testle kanıtlanabiliyor.
#[async_trait::async_trait]
pub trait ClipSource: Send + Sync {
    async fn video_info(&self, video_id: &str) -> Result<VideoInfoResponse, StreamError>;
    /// Kaynağın bir penceresini gerçek zamanlı klip olarak ister.
    ///
    /// Uzun kayıtlar servise tek istekte sığmadığı için pencere alıyor:
    /// tüm video için `(0, duration_ms)` verilir.
    async fn window_clip(
        &self,
        video_id: &str,
        t0_ms: u64,
        t1_ms: u64,
        max_dim: Option<u32>,
    ) -> Result<ClipRef, StreamError>;
    async fn zoom_clip(
        &self,
        video_id: &str,
        t0_ms: u64,
        t1_ms: u64,
        budget: usize,
    ) -> Result<ClipRef, StreamError>;
    async fn fetch_blob(&self, object_key: &str) -> Result<Vec<u8>, StreamError>;
}

pub struct StreamClient {
    client: reqwest::Client,
    base_url: String,
}

impl StreamClient {
    pub fn new(base_url: impl Into<String>) -> anyhow::Result<Self> {
        Ok(Self {
            // Klip üretimi ffmpeg çalıştırıyor; uzun videoda dakikaları bulabilir.
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(1800))
                .build()?,
            base_url: base_url.into().trim_end_matches('/').to_string(),
        })
    }

    /// Geçici arızada bir kez yeniden denenen POST.
    ///
    /// # Neden
    ///
    /// Klip üretimi ffmpeg alt süreci ve dosya sistemi kullanıyor; ikisi de
    /// geçici olarak düşebiliyor. Ölçümde görüldü: aynı çağrı yoğun dosya
    /// trafiği altında `500 G/Ç hatası` verdi, tek başına tekrarlandığında
    /// 5/5 başarılı oldu.
    ///
    /// Bu yol tekrarlanabilir: `clip_range` ve `video_info` yan etkisiz,
    /// `zoom_range` ise yakınlaştırma bütçesinden hak harcıyor — o yüzden
    /// **yalnız istek servise ulaşamadığında ya da 5xx döndüğünde**
    /// tekrarlanıyor. 4xx (bütçe bitti, geçersiz aralık) tekrarlanmıyor:
    /// aynı cevabı getirir ve `429` durumunda hakkı boşa harcardı.
    async fn post<T: Serialize>(&self, yol: &str, govde: &T) -> Result<Value, StreamError> {
        let url = format!("{}{yol}", self.base_url);
        let mut son_hata: Option<StreamError> = None;

        for deneme in 0..2 {
            if deneme > 0 {
                tracing::warn!(%url, hata = ?son_hata, "stream çağrısı geçici olarak düştü, tekrar deneniyor");
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }

            let res = match self.client.post(&url).json(govde).send().await {
                Ok(r) => r,
                Err(source) => {
                    son_hata = Some(StreamError::Transport {
                        url: url.clone(),
                        source,
                    });
                    continue;
                }
            };

            let status = res.status();
            if !status.is_success() {
                let body = res.text().await.unwrap_or_default();
                let hata = StreamError::Status {
                    status: status.as_u16(),
                    body: body.chars().take(400).collect(),
                };
                if status.is_server_error() {
                    son_hata = Some(hata);
                    continue;
                }
                return Err(hata);
            }

            return res
                .json()
                .await
                .map_err(|e| StreamError::Decode(e.to_string()));
        }

        Err(son_hata.unwrap_or_else(|| StreamError::Decode("bilinmeyen".into())))
    }

}

#[async_trait::async_trait]
impl ClipSource for StreamClient {
    /// Videonun süresi, çözünürlüğü, kodeği.
    async fn video_info(&self, video_id: &str) -> Result<VideoInfoResponse, StreamError> {
        let v = self
            .post("/v1/tools/video_info", &serde_json::json!({"video_id": video_id}))
            .await?;
        serde_json::from_value(v).map_err(|e| StreamError::Decode(e.to_string()))
    }

    /// Bir pencereyi gerçek zamanlı klip olarak ister.
    async fn window_clip(
        &self,
        video_id: &str,
        t0_ms: u64,
        t1_ms: u64,
        max_dim: Option<u32>,
    ) -> Result<ClipRef, StreamError> {
        let v = self
            .post(
                "/v1/tools/clip_range",
                &serde_json::json!({
                    "video_id": video_id,
                    "t0_ms": t0_ms,
                    "t1_ms": t1_ms,
                    "max_dim": max_dim,
                }),
            )
            .await?;
        let r: ClipResponse =
            serde_json::from_value(v).map_err(|e| StreamError::Decode(e.to_string()))?;
        Ok(r.clip)
    }

    /// Bir aralığın yakınlaştırılmış klibini ister.
    ///
    /// `budget` istenen kare sayısı; servis 2 fps örneklediği için gerekiyorsa
    /// klibi ağır çekime alır ve bunu `ClipRef.time_scale` ile bildirir.
    async fn zoom_clip(
        &self,
        video_id: &str,
        t0_ms: u64,
        t1_ms: u64,
        budget: usize,
    ) -> Result<ClipRef, StreamError> {
        let req = ZoomRangeRequest {
            video_id: video_id.to_string().into(),
            t0_ms,
            t1_ms,
            budget,
        };
        let v = self.post("/v1/tools/zoom_range", &req).await?;
        let r: ClipResponse =
            serde_json::from_value(v).map_err(|e| StreamError::Decode(e.to_string()))?;
        Ok(r.clip)
    }

    /// Klip baytlarını indirir.
    async fn fetch_blob(&self, object_key: &str) -> Result<Vec<u8>, StreamError> {
        let url = format!("{}/v1/blobs/{object_key}", self.base_url);
        let res = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|source| StreamError::Transport {
                url: url.clone(),
                source,
            })?;

        let status = res.status();
        if !status.is_success() {
            return Err(StreamError::Status {
                status: status.as_u16(),
                body: res.text().await.unwrap_or_default().chars().take(400).collect(),
            });
        }

        Ok(res
            .bytes()
            .await
            .map_err(|e| StreamError::Decode(e.to_string()))?
            .to_vec())
    }
}
