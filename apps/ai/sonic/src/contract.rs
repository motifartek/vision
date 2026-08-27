//! `/v1/audio/analyze` yanıtının şekli — istemcinin gördüğü sözleşme.
//!
//! Boru hattının nasıl çalıştığından ayrı tutuluyor: bu dosyadaki bir alan adı
//! değişirse dashboard kırılır, `analysis.rs` içindeki bir hesap değişirse
//! kırılmaz. İkisinin aynı dosyada olması bu ayrımı görünmez kılıyordu.

use serde::Serialize;

use crate::events::{AudioEvent, ClassSummary};
use crate::safety::SafetyReport;

/// Pencere başına ilk-K sınıf, **sıkışık biçimde**: `[sınıf indeksi, skor]`.
///
/// Etiket adları burada taşınmaz; istemci `/v1/labels` çağrısıyla 527 etiketi
/// bir kez alıp önbelleğe koyar. 9 dakikalık videoda fark büyük: adlarla
/// ~540 KB, indeksle ~126 KB. Canlı okuma paneli her analizde bu veriyi
/// istediği için boyut doğrudan arayüzün açılma hızına yansıyor.
#[derive(Debug, Clone, Serialize)]
pub struct FrameTop {
    pub t: f32,
    pub top: Vec<(usize, f32)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaInfo {
    pub duration_sec: f32,
    pub sample_rate: usize,
    pub truncated: bool,
    /// Kullanılan çözücü: "symphonia" (süreç içi) veya "ffmpeg" (yedek).
    pub decoder: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub name: String,
    pub weights: String,
    pub providers: Vec<String>,
    pub classes: usize,
    pub profile: &'static str,
    pub window_sec: f32,
    pub hop_sec: f32,
    pub windows: usize,
    pub batch_size: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Timing {
    pub decode_ms: u128,
    pub mel_ms: u128,
    pub inference_ms: u128,
    pub segment_ms: u128,
    pub total_ms: u128,
    /// Medya süresi ÷ harcanan süre. 1'in üzerindeki her değer gerçek zamandan hızlı.
    pub realtime_factor: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Analysis {
    pub media: MediaInfo,
    pub model: ModelInfo,
    pub events: Vec<AudioEvent>,
    /// `max_events` sınırına takılıp olay listesi kısaldıysa `true`. Sessiz
    /// kırpma, "bu kayıtta başka bir şey yok" gibi okunuyordu; istemci farkı
    /// söyleyebilsin diye açıkça bildiriliyor. `summary` kırpmadan etkilenmez.
    pub events_truncated: bool,
    pub summary: Vec<ClassSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frames: Option<Vec<FrameTop>>,
    /// İş güvenliği katmanı: güvenlik sınıfına düşen olaylar ve kural bulguları.
    pub safety: SafetyReport,
    pub timing: Timing,
}
