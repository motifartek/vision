//! ffprobe ile video metadata çıkarma.
//!
//! Tek bir `ffprobe` çağrısıyla hem akış hem konteyner bilgisini JSON olarak
//! alır. Boru hattının ilk adımı: kaç kare çözeceğimizi, hangi çözünürlükte
//! çalışacağımızı ve zaman hesabının tabanını buradan öğreniyoruz.

use std::path::Path;
use std::process::Command;

use motif_core::{Error, Result};
use serde::Deserialize;

use crate::preflight::ExternalTool;
use crate::types::VideoInfo;

#[derive(Deserialize)]
struct FfprobeOutput {
    #[serde(default)]
    streams: Vec<FfprobeStream>,
    #[serde(default)]
    format: FfprobeFormat,
}

#[derive(Deserialize)]
struct FfprobeStream {
    width: Option<u32>,
    height: Option<u32>,
    /// Konteynerin bildirdiği taban kare hızı, `"30000/1001"` gibi bir kesir.
    r_frame_rate: Option<String>,
    /// Gerçekleşen ortalama kare hızı. Değişken kare hızlı (VFR) videolarda
    /// `r_frame_rate`'ten daha temsili olduğu için önce buna bakılır.
    avg_frame_rate: Option<String>,
    codec_name: Option<String>,
    duration: Option<String>,
}

#[derive(Deserialize, Default)]
struct FfprobeFormat {
    duration: Option<String>,
    size: Option<String>,
}

/// `"30000/1001"` biçimindeki kesri saniyedeki kareye çevirir.
///
/// ffprobe bölünemeyen akışlarda `"0/0"` döndürür; bu durumda `None` verilir
/// ki çağıran diğer alana düşebilsin.
fn parse_rational(value: &str) -> Option<f64> {
    let (num, den) = value.split_once('/')?;
    let num: f64 = num.trim().parse().ok()?;
    let den: f64 = den.trim().parse().ok()?;
    if den == 0.0 || num == 0.0 {
        return None;
    }
    Some(num / den)
}

/// Bir video dosyasının temel özelliklerini okur.
pub fn probe(path: &Path) -> Result<VideoInfo> {
    if !path.exists() {
        return Err(Error::NotFound(format!("video dosyası yok: {}", path.display())));
    }

    let output = Command::new(ExternalTool::Ffprobe.binary())
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,r_frame_rate,avg_frame_rate,codec_name,duration",
            "-show_entries",
            "format=duration,size",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .map_err(|_| Error::MissingDependency {
            name: ExternalTool::Ffprobe.binary().to_string(),
            hint: "ffmpeg'i kurup PATH'e ekleyin.".to_string(),
        })?;

    if !output.status.success() {
        return Err(Error::CommandFailed {
            command: format!("ffprobe {}", path.display()),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    let parsed: FfprobeOutput = serde_json::from_slice(&output.stdout)?;

    let stream = parsed
        .streams
        .into_iter()
        .next()
        .ok_or_else(|| Error::InvalidVideo(format!("dosyada video akışı yok: {}", path.display())))?;

    let width = stream
        .width
        .ok_or_else(|| Error::InvalidVideo("video akışında genişlik bilgisi yok".into()))?;
    let height = stream
        .height
        .ok_or_else(|| Error::InvalidVideo("video akışında yükseklik bilgisi yok".into()))?;

    // Önce gerçekleşen ortalama, sonra konteynerin taban kare hızı.
    let fps = stream
        .avg_frame_rate
        .as_deref()
        .and_then(parse_rational)
        .or_else(|| stream.r_frame_rate.as_deref().and_then(parse_rational))
        .ok_or_else(|| Error::InvalidVideo("kare hızı belirlenemedi".into()))?;

    // Süre konteynerde daha güvenilir; yoksa akıştan alınır.
    let duration_secs: f64 = parsed
        .format
        .duration
        .as_deref()
        .or(stream.duration.as_deref())
        .and_then(|d| d.trim().parse().ok())
        .ok_or_else(|| Error::InvalidVideo("video süresi belirlenemedi".into()))?;

    if duration_secs <= 0.0 {
        return Err(Error::InvalidVideo("video süresi sıfır veya negatif".into()));
    }

    let size_bytes = parsed
        .format
        .size
        .as_deref()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);

    Ok(VideoInfo {
        duration_ms: (duration_secs * 1000.0).round() as u64,
        fps,
        width,
        height,
        size_bytes,
        codec: stream.codec_name.unwrap_or_else(|| "bilinmiyor".into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kesir_kare_hizina_cevrilir() {
        assert_eq!(parse_rational("30/1"), Some(30.0));
        assert!((parse_rational("30000/1001").unwrap() - 29.97).abs() < 0.01);
    }

    #[test]
    fn bolunemeyen_kesir_none_doner() {
        assert_eq!(parse_rational("0/0"), None);
        assert_eq!(parse_rational("25"), None);
        assert_eq!(parse_rational("0/1"), None);
    }
}
