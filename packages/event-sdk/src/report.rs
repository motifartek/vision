//! Sistemin nihai çıktısı — şartnamenin zorunlu tuttuğu rapor biçimi.
//!
//! Şartname (3. Senaryo §5) çıktı şeklini birebir vermiştir:
//!
//! ```json
//! {
//!   "summary": "Videoda forklift kazası ve yaralanma riski gözlenmiştir.",
//!   "events": [
//!     {"time": "00:15", "event": "Forklift devrildi"},
//!     {"time": "00:20", "event": "Yerde hareketsiz kişi"}
//!   ],
//!   "risk": "Yüksek",
//!   "actions": ["Sağlık ekibini çağır", "Alanı güvenlik altına al"]
//! }
//! ```
//!
//! [`AnalysisReport`] bu şeklin üstüne dahili alanlar ekler.
//! Jüriye/teslime giden dar biçim için [`AnalysisReport::to_sartname_json`]
//! kullanılır — böylece zenginleştirme yaparken teslim formatından sapılmaz.

use motif_core::VideoId;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{format_timestamp, SCHEMA_VERSION};

/// Şartnamenin risk seviyeleri. Türkçe değerlerle serileşir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    #[serde(rename = "Düşük")]
    Dusuk,
    #[serde(rename = "Orta")]
    Orta,
    #[serde(rename = "Yüksek")]
    Yuksek,
}

impl RiskLevel {
    /// Panelde kullanılacak renk kodu (#4, risk renkli uyarı kartları).
    pub fn severity_rank(self) -> u8 {
        match self {
            RiskLevel::Dusuk => 0,
            RiskLevel::Orta => 1,
            RiskLevel::Yuksek => 2,
        }
    }
}

/// Zaman damgalı tek bir tespit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedEvent {
    /// Kanonik zaman. Tüm dahili hesaplar bunu kullanır.
    pub t_ms: u64,
    /// İnsan tarafından okunan biçim (`"00:15"`). `t_ms`'den türetilir.
    pub time: String,
    /// Türkçe olay açıklaması.
    pub event: String,
    pub severity: RiskLevel,
    /// Modelin kendi güveni, varsa. Açıklanabilirlik için taşınır.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
}

impl DetectedEvent {
    pub fn new(t_ms: u64, event: impl Into<String>, severity: RiskLevel) -> Self {
        Self {
            t_ms,
            time: format_timestamp(t_ms),
            event: event.into(),
            severity,
            confidence: None,
        }
    }
}

/// `event.risk.detected` yükü ve sistemin nihai analiz çıktısı.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisReport {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub video_id: VideoId,
    /// Kısa, operatörün hızlı karar almasını destekleyen Türkçe özet.
    pub summary: String,
    pub events: Vec<DetectedEvent>,
    /// Videonun tamamı için genel risk değerlendirmesi.
    pub risk: RiskLevel,
    /// Operatöre sunulan uygulanabilir aksiyon önerileri.
    pub actions: Vec<String>,
    /// Analizin toplam süresi. Şartname performansı puanlıyor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processing_ms: Option<u64>,
}

impl AnalysisReport {
    /// Şartnamenin §5'te verdiği dar teslim biçimini üretir.
    ///
    /// Dahili alanlar (t_ms, severity, confidence, video_id) atılır;
    /// sadece `summary`, `events[{time,event}]`, `risk`, `actions` kalır.
    pub fn to_sartname_json(&self) -> serde_json::Value {
        json!({
            "summary": self.summary,
            "events": self.events.iter().map(|e| json!({
                "time": e.time,
                "event": e.event,
            })).collect::<Vec<_>>(),
            "risk": self.risk,
            "actions": self.actions,
        })
    }
}

fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teslim_bicimi_sartnameyle_ayni_sekle_sahip() {
        let report = AnalysisReport {
            schema_version: SCHEMA_VERSION,
            video_id: VideoId::from("v1".to_string()),
            summary: "Videoda forklift kazası ve yaralanma riski gözlenmiştir.".into(),
            events: vec![
                DetectedEvent::new(15_000, "Forklift devrildi", RiskLevel::Yuksek),
                DetectedEvent::new(20_000, "Yerde hareketsiz kişi", RiskLevel::Yuksek),
            ],
            risk: RiskLevel::Yuksek,
            actions: vec!["Sağlık ekibini çağır".into(), "Alanı güvenlik altına al".into()],
            processing_ms: Some(4200),
        };

        let out = report.to_sartname_json();

        // Şartnamedeki dört anahtar, fazlası değil.
        let obj = out.as_object().unwrap();
        assert_eq!(obj.len(), 4);
        assert!(obj.contains_key("summary"));
        assert!(obj.contains_key("events"));
        assert!(obj.contains_key("risk"));
        assert!(obj.contains_key("actions"));

        assert_eq!(out["risk"], "Yüksek");
        assert_eq!(out["events"][0]["time"], "00:15");
        assert_eq!(out["events"][0]["event"], "Forklift devrildi");
        // Dahili alanlar sızmamalı.
        assert!(out["events"][0].get("t_ms").is_none());
        assert!(out["events"][0].get("severity").is_none());
    }
}
