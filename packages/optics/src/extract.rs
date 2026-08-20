//! Seçilen karelerin tam kalitede çıkarılması.
//!
//! Analiz 160x90 gri karelerle yapılır; modele giden kareler ise tam
//! çözünürlükte olmalıdır. Bu modül seçilen zaman damgalarındaki kareleri
//! kaynak videodan JPEG olarak çıkarır.
//!
//! # Zaman damgası bindirmesi
//!
//! VLM'ler kare **sırasını** bilir, saati bilmez: 16 kare verildiğinde
//! hangisinin 00:15 olduğunu çıkaramaz. Oysa şartname puanı tam da zaman
//! damgasından veriyor ("kritik anları zaman bilgisi ile belirlemelidir").
//!
//! Ucuz çözüm: zaman damgasını karenin köşesine yazmak. Model okuyabiliyor.
//! Seçenek olarak bırakıldı, çünkü etkisi ölçülüp A/B karşılaştırılacak —
//! bindirme görüntünün bir kısmını da kapatıyor.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use motif_core::{Error, Result};

use crate::preflight::ExternalTool;
use crate::types::AnalysisConfig;

/// Çıkarma ayarları.
#[derive(Debug, Clone)]
pub struct ExtractOptions {
    /// Uzun kenarın piksel sınırı. Kare bu kutuya sığacak şekilde küçültülür.
    pub max_dim: Option<u32>,
    /// JPEG kalitesi (ffmpeg `-q:v`): 2 en iyi, 31 en kötü.
    pub quality: u8,
    /// Zaman damgasını karenin köşesine yaz.
    pub timestamp_overlay: bool,
    /// Bindirme için kullanılacak font. Verilmezse sistemde aranır.
    pub font_path: Option<PathBuf>,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            max_dim: None,
            quality: 2,
            timestamp_overlay: false,
            font_path: None,
        }
    }
}

/// Çıkarılmış tek bir kare.
#[derive(Debug, Clone)]
pub struct ExtractedFrame {
    pub t_ms: u64,
    pub path: PathBuf,
}

/// Çıkarma işleminin sonucu ve süresi.
#[derive(Debug, Clone)]
pub struct ExtractionResult {
    pub frames: Vec<ExtractedFrame>,
    pub elapsed: Duration,
}

/// Yaygın tek aralıklı font konumları.
///
/// Tek aralıklı font tercih ediliyor: rakamlar sabit genişlikte olduğu için
/// bindirme kutusu kare kare zıplamıyor.
const FONT_CANDIDATES: &[&str] = &[
    "C:/Windows/Fonts/consola.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
    "/System/Library/Fonts/Menlo.ttc",
];

fn find_font(explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = explicit {
        return path.exists().then(|| path.to_path_buf());
    }
    FONT_CANDIDATES
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
}

/// ffmpeg filtre argümanı içindeki özel karakterleri kaçırır.
///
/// Filtre söziziminde `:` seçenekleri ayırır, dolayısıyla değerin içinde
/// geçtiğinde kaçırılmalıdır. Windows yollarında hem `:` hem `\` bulunduğu
/// için ters bölü önce eğik çizgiye çevriliyor.
fn escape_filter_value(value: &str) -> String {
    value
        .replace('\\', "/")
        .replace(':', "\\:")
        .replace('\'', "\\'")
}

/// Zaman damgasını `MM:SS.m` biçiminde yazar.
fn overlay_text(t_ms: u64) -> String {
    let total_secs = t_ms / 1000;
    let tenths = (t_ms % 1000) / 100;
    format!("{:02}:{:02}.{}", total_secs / 60, total_secs % 60, tenths)
}

/// Verilen ayarlar için `-vf` filtre zincirini kurar.
fn build_filters(t_ms: u64, opts: &ExtractOptions, font: Option<&Path>) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();

    if let Some(max_dim) = opts.max_dim {
        // Kutuya sığdır, en-boy oranını koru, büyütme yapma.
        parts.push(format!(
            "scale=w={max_dim}:h={max_dim}:force_original_aspect_ratio=decrease"
        ));
    }

    if opts.timestamp_overlay {
        if let Some(font) = font {
            parts.push(format!(
                "drawtext=fontfile='{}':text='{}':x=12:y=12:fontsize=28:fontcolor=white:box=1:boxcolor=black@0.6:boxborderw=8",
                escape_filter_value(&font.to_string_lossy()),
                escape_filter_value(&overlay_text(t_ms))
            ));
        }
    }

    (!parts.is_empty()).then(|| parts.join(","))
}

/// Seçilen zaman damgalarındaki kareleri JPEG olarak çıkarır.
///
/// # Neden kare başına ayrı ffmpeg çağrısı
///
/// Alternatif, tek çağrıda `select` filtresiyle hepsini almaktı. Ama o yol
/// videonun **tamamını** tam çözünürlükte çözmeyi gerektiriyor; 16 kare için
/// 2 dakikalık videoyu baştan sona çözmek, 16 kez anahtar kareye atlayıp
/// ileri çözmekten pahalı. Süreç açma maliyeti ölçüldü (~20 ms), toplamda
/// ihmal edilebilir kalıyor.
pub fn extract_jpegs(
    video: &Path,
    timestamps: &[u64],
    out_dir: &Path,
    opts: &ExtractOptions,
) -> Result<ExtractionResult> {
    if !video.exists() {
        return Err(Error::NotFound(format!(
            "video dosyası yok: {}",
            video.display()
        )));
    }

    std::fs::create_dir_all(out_dir)?;

    let font = opts
        .timestamp_overlay
        .then(|| find_font(opts.font_path.as_deref()))
        .flatten();

    if opts.timestamp_overlay && font.is_none() {
        return Err(Error::Config(
            "zaman damgası bindirmesi istendi ama kullanılabilir font bulunamadı; \
             --font ile bir .ttf yolu verin"
                .into(),
        ));
    }

    let started = Instant::now();
    let mut frames = Vec::with_capacity(timestamps.len());

    for &t_ms in timestamps {
        // Sıfır dolgulu ad: dosyalar sözlük sırasında kronolojik gelir.
        let out_path = out_dir.join(format!("{t_ms:09}.jpg"));

        let mut cmd = Command::new(ExternalTool::Ffmpeg.binary());
        cmd.args(["-nostdin", "-v", "error", "-y"]);
        // -ss girdiden önce: anahtar kareye hızlı atlar, oradan ileri çözer.
        cmd.args(["-ss", &format!("{:.3}", t_ms as f64 / 1000.0)]);
        cmd.arg("-i").arg(video);
        cmd.args(["-frames:v", "1"]);

        if let Some(filters) = build_filters(t_ms, opts, font.as_deref()) {
            cmd.args(["-vf", &filters]);
        }

        cmd.args(["-q:v", &opts.quality.to_string()]);
        cmd.arg(&out_path);

        let output = cmd
            .stdin(Stdio::null())
            .output()
            .map_err(|_| Error::MissingDependency {
                name: ExternalTool::Ffmpeg.binary().to_string(),
                hint: "ffmpeg'i kurup PATH'e ekleyin.".to_string(),
            })?;

        if !output.status.success() {
            return Err(Error::CommandFailed {
                command: format!("ffmpeg kare çıkarma (t={t_ms} ms)"),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        if !out_path.exists() {
            return Err(Error::InvalidVideo(format!(
                "{t_ms} ms için kare üretilemedi; zaman video sınırları dışında olabilir"
            )));
        }

        frames.push(ExtractedFrame {
            t_ms,
            path: out_path,
        });
    }

    Ok(ExtractionResult {
        frames,
        elapsed: started.elapsed(),
    })
}

/// Tek bir zaman damgasındaki kareyi analiz çözünürlüğünde gri olarak çıkarır.
///
/// Atlama (seek) doğruluğunu sınamak için var: aynı karenin sıralı çözmeden
/// gelen hâliyle karşılaştırılabilmesi gerekiyor. `-ss` girdiden önce
/// kullanıldığında ffmpeg anahtar kareye atlayıp ileri çözer; bu hızlıdır ama
/// bazı kodeklerde kayabilir. Kaymanın olup olmadığı ölçülmeli, varsayılmamalı.
pub fn extract_gray_at(video: &Path, t_ms: u64, cfg: AnalysisConfig) -> Result<Vec<u8>> {
    let filter = format!("scale={}:{},format=gray", cfg.width, cfg.height);

    let output = Command::new(ExternalTool::Ffmpeg.binary())
        .args(["-nostdin", "-v", "error"])
        .args(["-ss", &format!("{:.3}", t_ms as f64 / 1000.0)])
        .arg("-i")
        .arg(video)
        .args(["-frames:v", "1", "-vf", &filter])
        .args(["-f", "rawvideo", "-pix_fmt", "gray", "-"])
        .stdin(Stdio::null())
        .output()
        .map_err(|_| Error::MissingDependency {
            name: ExternalTool::Ffmpeg.binary().to_string(),
            hint: "ffmpeg'i kurup PATH'e ekleyin.".to_string(),
        })?;

    if !output.status.success() {
        return Err(Error::CommandFailed {
            command: format!("ffmpeg gri kare çıkarma (t={t_ms} ms)"),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    if output.stdout.len() != cfg.frame_bytes() {
        return Err(Error::InvalidVideo(format!(
            "{} bayt bekleniyordu, {} bayt geldi",
            cfg.frame_bytes(),
            output.stdout.len()
        )));
    }

    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zaman_damgasi_metni_dogru_bicimlenir() {
        assert_eq!(overlay_text(0), "00:00.0");
        assert_eq!(overlay_text(15_200), "00:15.2");
        assert_eq!(overlay_text(95_000), "01:35.0");
        assert_eq!(overlay_text(3_725_400), "62:05.4");
    }

    #[test]
    fn filtre_degerindeki_ozel_karakterler_kacirilir() {
        assert_eq!(
            escape_filter_value("C:\\Windows\\Fonts\\consola.ttf"),
            "C\\:/Windows/Fonts/consola.ttf"
        );
        assert_eq!(escape_filter_value("00:15.2"), "00\\:15.2");
    }

    #[test]
    fn filtre_zinciri_yalnizca_istenenleri_icerir() {
        let bos = ExtractOptions::default();
        assert!(build_filters(0, &bos, None).is_none());

        let olcekli = ExtractOptions {
            max_dim: Some(768),
            ..Default::default()
        };
        let f = build_filters(0, &olcekli, None).unwrap();
        assert!(f.contains("scale=w=768:h=768"));
        assert!(f.contains("force_original_aspect_ratio=decrease"));
        assert!(!f.contains("drawtext"));

        // Font yoksa bindirme sessizce atlanır; çağıran taraf zaten
        // extract_jpegs içinde hata alır.
        let bindirmeli = ExtractOptions {
            timestamp_overlay: true,
            ..Default::default()
        };
        assert!(build_filters(0, &bindirmeli, None).is_none());

        let font = PathBuf::from("C:/Windows/Fonts/consola.ttf");
        let f = build_filters(15_200, &bindirmeli, Some(&font)).unwrap();
        assert!(f.contains("drawtext"));
        assert!(f.contains("00\\:15.2"));
    }
}
