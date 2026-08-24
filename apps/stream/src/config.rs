//! Servis yapılandırması.
//!
//! Tamamı ortam değişkeninden okunur ve makul varsayılanları vardır: servis
//! hiçbir ayar verilmeden `cargo run -p stream` ile ayağa kalkar. Hedef donanım
//! belli olmadığı için kare bütçeleri de dahil her şey çalışma zamanı ayarıdır,
//! koda gömülü değildir.

use std::path::PathBuf;

use motif_optics::{AnalysisConfig, SamplingConfig};

/// Varsayılan yükleme sınırı (2 GB).
///
/// Axum'un varsayılanı 2 MB ve bu, video yüklemesini sessizce ortadan kesip
/// bağlantıyı düşürüyor — hata mesajı da yanıltıcı oluyor. Sınır bilerek
/// cömert tutuluyor.
const DEFAULT_MAX_UPLOAD_BYTES: usize = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: String,
    pub storage_root: PathBuf,
    /// Boşsa NATS bağlantısı kurulmaz ve olaylar yayımlanmaz.
    pub nats_url: Option<String>,
    pub max_upload_bytes: usize,

    pub analysis: AnalysisConfig,
    pub sampling: SamplingConfig,

    /// Genel bakışta modele gidecek kare sayısı.
    pub overview_budget: usize,
    /// Bir yakınlaştırma çağrısında dönecek kare sayısı.
    pub zoom_budget: usize,
    /// Bir video için ajanın yapabileceği azami yakınlaştırma sayısı.
    ///
    /// Ajan kendi kendine yakınlaşmaya karar verdiği için bir üst sınır şart:
    /// yoksa kararsız bir model aynı aralığa tekrar tekrar girip en kötü
    /// durumda gecikmeyi sınırsız büyütebilir.
    pub max_zooms_per_video: usize,
    /// Karelere zaman damgası bindir.
    pub timestamp_overlay: bool,
    /// Modele giden karenin uzun kenar sınırı.
    pub frame_max_dim: u32,
}

fn env_string(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

fn env_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
    env_string(key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_bool(key: &str, default: bool) -> bool {
    match env_string(key).map(|v| v.to_ascii_lowercase()) {
        Some(v) => matches!(v.as_str(), "1" | "true" | "yes" | "evet" | "on"),
        None => default,
    }
}

impl Config {
    pub fn from_env() -> Self {
        let analysis = AnalysisConfig {
            analysis_fps: env_parse("STREAM_ANALYSIS_FPS", 15.0),
            width: env_parse("STREAM_ANALYSIS_WIDTH", 160),
            height: env_parse("STREAM_ANALYSIS_HEIGHT", 90),
        };

        let overview_budget = env_parse("STREAM_OVERVIEW_BUDGET", 16usize);

        let sampling = SamplingConfig {
            budget: overview_budget,
            uniform_prior: env_parse("STREAM_UNIFORM_PRIOR", 0.25),
            dedup_hamming: env_parse("STREAM_DEDUP_HAMMING", 3),
            force_scene_cuts: env_bool("STREAM_FORCE_SCENE_CUTS", true),
            subtract_noise_floor: env_bool("STREAM_SUBTRACT_NOISE_FLOOR", true),
        };

        Self {
            bind: env_string("STREAM_BIND").unwrap_or_else(|| "0.0.0.0:8100".into()),
            storage_root: env_string("STREAM_STORAGE_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("data/stream")),
            nats_url: env_string("NATS_URL"),
            max_upload_bytes: env_parse("STREAM_MAX_UPLOAD_BYTES", DEFAULT_MAX_UPLOAD_BYTES),
            analysis,
            sampling,
            overview_budget,
            zoom_budget: env_parse("STREAM_ZOOM_BUDGET", 12),
            max_zooms_per_video: env_parse("STREAM_MAX_ZOOMS", 8),
            timestamp_overlay: env_bool("STREAM_TIMESTAMP_OVERLAY", true),
            frame_max_dim: env_parse("STREAM_FRAME_MAX_DIM", 768),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::from_env()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ayarsiz_calisabilir_varsayilanlar() {
        // Ortam değişkeni yokken bile geçerli bir yapılandırma çıkmalı:
        // `cargo run -p stream` hiçbir kurulum gerektirmeden ayağa kalkıyor.
        let cfg = Config {
            bind: "0.0.0.0:8100".into(),
            storage_root: PathBuf::from("data/stream"),
            nats_url: None,
            max_upload_bytes: DEFAULT_MAX_UPLOAD_BYTES,
            analysis: AnalysisConfig::default(),
            sampling: SamplingConfig::default(),
            overview_budget: 16,
            zoom_budget: 12,
            max_zooms_per_video: 8,
            timestamp_overlay: true,
            frame_max_dim: 768,
        };

        assert_eq!(cfg.analysis.width, 160);
        assert_eq!(cfg.sampling.budget, 16);
        assert!(cfg.nats_url.is_none(), "NATS varsayılan olarak kapalı");
    }

    #[test]
    fn bool_ayrıştırma_turkce_degerleri_de_kabul_eder() {
        assert!(!env_bool("MOTIF_TEST_YOK_BOYLE_BIR_DEGISKEN", false));
        assert!(env_bool("MOTIF_TEST_YOK_BOYLE_BIR_DEGISKEN", true));
    }
}
