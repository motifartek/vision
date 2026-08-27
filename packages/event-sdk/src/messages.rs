//! Olay gÃ¼dÃ¼mlÃ¼ (pub/sub) mesajlar.

use chrono::{DateTime, Utc};
use motif_core::VideoId;
use serde::{Deserialize, Serialize};

use crate::SCHEMA_VERSION;

/// `stream.video.ingested` â€” ham video alÄ±ndÄ±, metadata Ã§Ä±karÄ±ldÄ±.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoIngested {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub video_id: VideoId,
    /// Ham videonun nesne deposundaki anahtarÄ±.
    pub object_key: String,
    pub duration_ms: u64,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
    pub codec: String,
    pub ingested_at: DateTime<Utc>,
}

/// Karelerin hangi geÃ§iÅŸte Ã¼retildiÄŸi.
///
/// TÃ¼ketici tarafÄ±n ayÄ±rt etmesi gerekir: `Overview` videonun tamamÄ±nÄ±n
/// kaba taramasÄ±, `Zoom` ise ajanÄ±n talebi Ã¼zerine dar bir aralÄ±ktan
/// Ã§Ä±karÄ±lmÄ±ÅŸ yoÄŸun karelerdir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SamplingPass {
    Overview,
    Zoom,
}

/// Tek bir seÃ§ilmiÅŸ kare.
///
/// Kare verisi mesaja **gÃ¶mÃ¼lmez**; sadece nesne deposu anahtarÄ± taÅŸÄ±nÄ±r.
/// BÃ¶ylece NATS mesajlarÄ± kÃ¼Ã§Ã¼k kalÄ±r ve tÃ¼keticiler sadece ihtiyaÃ§
/// duyduklarÄ±nÄ± indirir.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameRef {
    /// Karenin videodaki gerÃ§ek zamanÄ±. Ajan zamanÄ± sÄ±radan Ã§Ä±karmak
    /// zorunda kalmaz.
    pub t_ms: u64,
    pub object_key: String,
    /// 0..1 aralÄ±ÄŸÄ±nda normalize edilmiÅŸ hareket skoru.
    pub motion_score: f32,
    pub is_scene_cut: bool,
}

/// `stream.frame.extracted` â€” bir grup kare hazÄ±r.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameExtracted {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub video_id: VideoId,
    /// AynÄ± geÃ§iÅŸe ait kareleri gruplamak iÃ§in.
    pub batch_id: String,
    pub pass: SamplingPass,
    pub frames: Vec<FrameRef>,
}

fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}

/// 	oolbox.execute - Dış bir aracı (mock) çalıştırma isteği
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecuteRequest {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub video_id: VideoId,
    pub tool_name: String,
    pub payload: serde_json::Value,
}
