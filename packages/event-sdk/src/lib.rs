//! MotifAI servisleri arasındaki paylaşılan kontratlar.
//!
//! Bu crate bilerek bağımlılık açısından hafif tutulur: ağ istemcisi,
//! veritabanı ya da medya kodu içermez. Amacı `apps/stream`, `apps/ai` ve
//! `apps/gateway`'in aynı tiplere karşı derlenmesidir.
//!
//! # Sürümleme
//!
//! Her mesaj `schema_version` taşır. Uyumsuz bir değişiklik yapılırken
//! [`SCHEMA_VERSION`] artırılmalı ve #6 altında ekibe duyurulmalıdır.

pub mod messages;
pub mod report;
pub mod subjects;
pub mod tools;

pub use messages::{FrameExtracted, FrameRef, SamplingPass, VideoIngested};
pub use report::{AnalysisReport, DetectedEvent, RiskLevel};
pub use tools::{
    ClipRangeRequest, ClipRef, ClipResponse, VideoInfoResponse, ZoomRangeRequest,
};

/// Kontrat sürümü. Kırıcı değişikliklerde artırılır.
pub const SCHEMA_VERSION: u32 = 1;

/// Milisaniyeyi şartnamenin beklediği `"MM:SS"` biçimine çevirir.
///
/// Bir saati aşan videolarda `"HH:MM:SS"` üretir.
pub fn format_timestamp(t_ms: u64) -> String {
    let total_secs = t_ms / 1000;
    let (h, m, s) = (total_secs / 3600, (total_secs % 3600) / 60, total_secs % 60);
    if h > 0 {
        format!("{h:02}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zaman_damgasi_sartname_bicimine_uyar() {
        assert_eq!(format_timestamp(15_200), "00:15");
        assert_eq!(format_timestamp(95_000), "01:35");
        assert_eq!(format_timestamp(3_725_000), "01:02:05");
    }
}
