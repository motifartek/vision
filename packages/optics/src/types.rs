use serde::{Deserialize, Serialize};

/// Bir video dosyasının temel özellikleri (ffprobe çıktısı).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoInfo {
    pub duration_ms: u64,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
    pub codec: String,
}

/// Analiz geçişinde çözülmüş tek bir gri kare.
///
/// Bunlar **analiz** kareleridir; küçük ve gri. Modele gönderilecek
/// kareler ayrıca tam kalitede çıkarılır.
#[derive(Debug, Clone)]
pub struct AnalysisFrame {
    /// Analiz akışındaki sıra numarası (0'dan başlar).
    pub index: u32,
    /// Karenin videodaki gerçek zamanı.
    pub t_ms: u64,
    /// `width * height` boyutunda gri piksel tamponu.
    pub data: Vec<u8>,
}

/// Pass 1 (hareket profili) çözümleme ayarları.
///
/// Bu değerler bilerek yapılandırılabilir: hedef donanım henüz belli
/// değil ve kare bütçesi ona göre değişecek.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AnalysisConfig {
    /// Saniyede kaç kare analiz edilecek.
    ///
    /// 30 fps çözmenin anlamı yok: "bir şey oluyor mu" sorusu için 15 fps
    /// fazlasıyla yeterli ve iş yükünü yarıya indiriyor. Ayrıca ffmpeg'in
    /// `fps=` filtresi çıktıyı sabit kare hızına zorladığı için zaman
    /// hesabı (`t_ms = index * 1000 / analysis_fps`) değişken kare hızlı
    /// videolarda bile kesin kalır.
    pub analysis_fps: f64,
    /// Analiz karesinin genişliği.
    pub width: u32,
    /// Analiz karesinin yüksekliği.
    pub height: u32,
}

impl AnalysisConfig {
    /// Tek bir analiz karesinin bayt cinsinden boyutu.
    pub const fn frame_bytes(&self) -> usize {
        (self.width * self.height) as usize
    }

    /// Verilen sıra numarasının zaman damgası.
    pub fn timestamp_ms(&self, index: u32) -> u64 {
        (index as f64 * 1000.0 / self.analysis_fps).round() as u64
    }
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            analysis_fps: 15.0,
            width: 160,
            height: 90,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varsayilan_kare_boyutu() {
        assert_eq!(AnalysisConfig::default().frame_bytes(), 14_400);
    }

    #[test]
    fn zaman_damgasi_sabit_kare_hizindan_turer() {
        let cfg = AnalysisConfig::default();
        assert_eq!(cfg.timestamp_ms(0), 0);
        assert_eq!(cfg.timestamp_ms(15), 1000);
        assert_eq!(cfg.timestamp_ms(213), 14_200);
    }
}
