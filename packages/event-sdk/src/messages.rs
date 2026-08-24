//! Olay güdümlü (pub/sub) mesajlar.

use chrono::{DateTime, Utc};
use motif_core::VideoId;
use serde::{Deserialize, Serialize};

use crate::SCHEMA_VERSION;

/// `stream.video.ingested` — ham video alındı, metadata çıkarıldı.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoIngested {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub video_id: VideoId,
    /// Ham videonun nesne deposundaki anahtarı.
    pub object_key: String,
    pub duration_ms: u64,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
    pub codec: String,
    pub ingested_at: DateTime<Utc>,
}

/// Karelerin hangi geçişte üretildiği.
///
/// Tüketici tarafın ayırt etmesi gerekir: `Overview` videonun tamamının
/// kaba taraması, `Zoom` ise ajanın talebi üzerine dar bir aralıktan
/// çıkarılmış yoğun karelerdir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SamplingPass {
    Overview,
    Zoom,
}

/// Tek bir seçilmiş kare.
///
/// Kare verisi mesaja **gömülmez**; sadece nesne deposu anahtarı taşınır.
/// Böylece NATS mesajları küçük kalır ve tüketiciler sadece ihtiyaç
/// duyduklarını indirir.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameRef {
    /// Karenin videodaki gerçek zamanı. Ajan zamanı sıradan çıkarmak
    /// zorunda kalmaz.
    pub t_ms: u64,
    pub object_key: String,
    /// 0..1 aralığında normalize edilmiş hareket skoru.
    pub motion_score: f32,
    pub is_scene_cut: bool,
}

/// `stream.frame.extracted` — bir grup kare hazır.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameExtracted {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub video_id: VideoId,
    /// Aynı geçişe ait kareleri gruplamak için.
    pub batch_id: String,
    pub pass: SamplingPass,
    pub frames: Vec<FrameRef>,
}

fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}
