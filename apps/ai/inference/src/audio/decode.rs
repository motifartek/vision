//! Herhangi bir medya dosyasını 16 kHz mono f32 PCM'e çevirir.
//!
//! İki yol var:
//!
//! 1. **symphonia** (birincil) — süreç içinde çözer. ffmpeg alt süreci bu
//!    makinede ölçülen ~960 ms'lik sabit yükleme maliyeti getiriyordu (Windows'un
//!    süreç açma tabanı 80 ms; fark tamamen ffmpeg ikilisinin boyutundan).
//! 2. **ffmpeg** (yedek) — symphonia'nın çözemediği format/kodekler için.
//!
//! Yeniden örnekleme yüksek kaliteli FFT tabanlı `rubato` ile yapılır; ucuz bir
//! resampler aliasing üretip mel spektrumunu, dolayısıyla model isabetini bozardı.

use std::fs::File;
use std::path::Path;
use std::process::Stdio;

use rubato::{FftFixedIn, Resampler};
use symphonia::core::audio::GenericAudioBufferRef;
use symphonia::core::codecs::CodecParameters;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

use crate::error::InferenceError;

pub const SAMPLE_RATE: usize = 16_000;

/// Üst sınır: 2 saatlik ses (~460 MB f32). Aşan girdiler kırpılır.
pub const DEFAULT_MAX_SAMPLES: usize = SAMPLE_RATE * 60 * 120;

pub struct Decoded {
    pub samples: Vec<f32>,
    pub truncated: bool,
    /// Hangi yolun kullanıldığı; ölçüm ve hata ayıklama için yanıtta raporlanır.
    pub backend: &'static str,
}

impl Decoded {
    pub fn duration_sec(&self) -> f32 {
        self.samples.len() as f32 / SAMPLE_RATE as f32
    }
}

pub async fn decode(path: &Path, max_samples: usize) -> Result<Decoded, InferenceError> {
    if !path.exists() {
        return Err(InferenceError::MediaNotFound(path.display().to_string()));
    }

    // symphonia senkron ve CPU yoğun; çalışan iş parçacığını bloke etmesin.
    let owned = path.to_path_buf();
    let attempt =
        tokio::task::spawn_blocking(move || decode_symphonia(&owned, max_samples)).await;

    match attempt {
        Ok(Ok(decoded)) => return Ok(decoded),
        Ok(Err(err)) => {
            // Desteklenmeyen format/kodek olağan bir durum, hata değil.
            tracing::debug!("symphonia çözemedi, ffmpeg'e düşülüyor: {}", err);
        }
        Err(join_err) => {
            tracing::warn!("symphonia görevi çöktü, ffmpeg'e düşülüyor: {}", join_err);
        }
    }

    decode_ffmpeg(path, max_samples).await
}

// --- symphonia yolu ---------------------------------------------------------

fn decode_symphonia(path: &Path, max_samples: usize) -> Result<Decoded, InferenceError> {
    let file = File::open(path)?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(extension);
    }

    let mut format = symphonia::default::get_probe()
        .probe(&hint, stream, FormatOptions::default(), MetadataOptions::default())
        .map_err(|e| InferenceError::Ffmpeg(format!("symphonia probe: {e}")))?;

    let track = format
        .default_track(TrackType::Audio)
        .ok_or(InferenceError::NoAudioStream)?;
    let track_id = track.id;

    let declared_delay = track.delay.unwrap_or(0) as usize;
    let declared_padding = track.padding.unwrap_or(0) as usize;

    let audio_params = match track.codec_params.as_ref() {
        Some(CodecParameters::Audio(params)) => params.clone(),
        _ => return Err(InferenceError::NoAudioStream),
    };

    // symphonia'nın MP4 okuyucusu kodlayıcı gecikmesini bildirmiyor (`delay` her
    // zaman 0), ffmpeg ise onu kırpıyor. Kırpmazsak sinyal bir AAC çerçevesi
    // kadar geç başlar — ölçüldü: tam 1024 örnek, hizalama sonrası kalan fark
    // 0.00000, yani çözüm bit düzeyinde aynı, yalnız kaymış. Zaman damgalarının
    // oynatıcıyla ve ffmpeg yoluyla tutması için gecikmeyi burada düşüyoruz.
    const AAC_ENCODER_DELAY: usize = 1024;
    let skip_frames = if declared_delay > 0 {
        declared_delay
    } else if audio_params.codec == symphonia::core::codecs::audio::well_known::CODEC_ID_AAC {
        AAC_ENCODER_DELAY
    } else {
        0
    };

    tracing::debug!(
        delay = declared_delay,
        padding = declared_padding,
        codec = ?audio_params.codec,
        atlanacak = skip_frames,
        "symphonia iz bilgisi"
    );
    let source_rate = audio_params
        .sample_rate
        .ok_or_else(|| InferenceError::Ffmpeg("kaynak örnekleme hızı bilinmiyor".into()))?;

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&audio_params, &Default::default())
        .map_err(|e| InferenceError::Ffmpeg(format!("symphonia kodek: {e}")))?;

    // Kırpma sınırı kaynak hızında uygulanır, yeniden örnekleme sonrasında değil.
    let source_cap = (max_samples as u64 * source_rate as u64 / SAMPLE_RATE as u64) as usize;

    let mut mono: Vec<f32> = Vec::new();
    let mut interleaved: Vec<f32> = Vec::new();
    let mut truncated = false;

    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(e) => return Err(InferenceError::Ffmpeg(format!("symphonia okuma: {e}"))),
        };
        if packet.track_id != track_id {
            continue;
        }

        let buffer = match decoder.decode(&packet) {
            Ok(buffer) => buffer,
            // Tek bir bozuk paket tüm dosyayı düşürmemeli.
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(e) => return Err(InferenceError::Ffmpeg(format!("symphonia çözme: {e}"))),
        };

        push_mono(&buffer, &mut interleaved, &mut mono)?;

        if mono.len() >= source_cap {
            mono.truncate(source_cap);
            truncated = true;
            break;
        }
    }

    if mono.is_empty() {
        return Err(InferenceError::NoAudioStream);
    }

    // Kodlayıcı gecikmesi ve varsa kuyruk dolgusu kaynak hızında kırpılır —
    // yeniden örneklemeden önce, ki kesim tam örnek sınırına düşsün.
    if skip_frames > 0 && skip_frames < mono.len() {
        mono.drain(..skip_frames);
    }
    if declared_padding > 0 && declared_padding < mono.len() {
        mono.truncate(mono.len() - declared_padding);
    }

    let samples = if source_rate as usize == SAMPLE_RATE {
        mono
    } else {
        resample(&mono, source_rate as usize, SAMPLE_RATE)?
    };

    Ok(Decoded { samples, truncated, backend: "symphonia" })
}

/// Stereo'yu mono'ya indirger.
///
/// Katsayı `1/√2`, `1/2` **değil**. ffmpeg'in davranışı ölçülerek belirlendi:
/// `-ac 1` çıktısı ile `(L+R)/2` arasındaki korelasyon 1.00000000, oran ise tam
/// olarak √2 çıkıyor. Yani ffmpeg enerji koruyan normalizasyon uyguluyor.
/// Basit ortalama kullanmak sinyali ~%29 sessizleştirir; mel dB ölçeğinde bu
/// kayma model skorlarını değiştirir (ölçüldü: ilk-5 sıralaması bozuluyordu).
///
/// İkiden çok kanalda ffmpeg konuma özgü downmix matrisleri kullanır; onu burada
/// taklit etmek yerine `Unsupported` dönüp ffmpeg yedek yoluna düşüyoruz.
fn push_mono(
    buffer: &GenericAudioBufferRef<'_>,
    scratch: &mut Vec<f32>,
    out: &mut Vec<f32>,
) -> Result<(), InferenceError> {
    let channels = buffer.spec().channels().count().max(1);
    scratch.clear();
    buffer.copy_to_vec_interleaved(scratch);

    match channels {
        1 => out.extend_from_slice(scratch),
        2 => {
            let gain = 1.0 / std::f32::consts::SQRT_2;
            out.extend(scratch.chunks_exact(2).map(|f| (f[0] + f[1]) * gain));
        }
        n => {
            return Err(InferenceError::Ffmpeg(format!(
                "{n} kanallı ses için ffmpeg downmix matrisi gerekiyor"
            )))
        }
    }
    Ok(())
}

fn resample(input: &[f32], from: usize, to: usize) -> Result<Vec<f32>, InferenceError> {
    const CHUNK: usize = 4096;
    const SUB_CHUNKS: usize = 2;

    let mut resampler = FftFixedIn::<f32>::new(from, to, CHUNK, SUB_CHUNKS, 1)
        .map_err(|e| InferenceError::Ffmpeg(format!("resampler kurulamadı: {e}")))?;

    let expected = (input.len() as u64 * to as u64 / from as u64) as usize;
    let mut out: Vec<f32> = Vec::with_capacity(expected + CHUNK);
    let mut position = 0;

    while position < input.len() {
        let needed = resampler.input_frames_next();
        let end = (position + needed).min(input.len());

        let mut chunk = input[position..end].to_vec();
        // Son parça eksikse sıfırla tamamlanır; fazlalık aşağıda kırpılıyor.
        chunk.resize(needed, 0.0);

        let processed = resampler
            .process(&[chunk], None)
            .map_err(|e| InferenceError::Ffmpeg(format!("yeniden örnekleme: {e}")))?;
        out.extend_from_slice(&processed[0]);

        position = end;
    }

    out.truncate(expected);
    Ok(out)
}

// --- ffmpeg yedek yolu ------------------------------------------------------

pub async fn decode_ffmpeg(path: &Path, max_samples: usize) -> Result<Decoded, InferenceError> {
    let mut child = Command::new("ffmpeg")
        .args(["-hide_banner", "-nostdin", "-loglevel", "error"])
        .arg("-i")
        .arg(path)
        // Yalnız ilk ses akışı; ses yoksa ffmpeg "matches no streams" ile döner.
        .args(["-vn", "-map", "0:a:0", "-ac", "1", "-ar", "16000"])
        .args(["-f", "f32le", "-acodec", "pcm_f32le", "pipe:1"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => InferenceError::FfmpegMissing,
            _ => InferenceError::Io(e),
        })?;

    let mut stdout = child.stdout.take().expect("stdout piped");
    let mut stderr = child.stderr.take().expect("stderr piped");
    let max_bytes = max_samples.saturating_mul(4);

    // stdout ve stderr eşzamanlı okunmalı; yalnız birini okumak boruyu tıkar.
    let pump = async {
        let mut buf: Vec<u8> = Vec::new();
        let mut chunk = vec![0u8; 64 * 1024];
        let mut truncated = false;
        loop {
            let n = stdout.read(&mut chunk).await?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
            if buf.len() >= max_bytes {
                buf.truncate(max_bytes);
                truncated = true;
                break;
            }
        }
        Ok::<_, std::io::Error>((buf, truncated))
    };
    let drain = async {
        let mut log = String::new();
        stderr.read_to_string(&mut log).await.map(|_| log)
    };

    let (pump, drain) = tokio::join!(pump, drain);
    let (bytes, truncated) = pump?;
    let log = drain.unwrap_or_default();

    if truncated {
        // Okumayı bıraktığımız için ffmpeg yazmada bloke olur; süreci sonlandır.
        let _ = child.kill().await;
    } else {
        let status = child.wait().await?;
        if !status.success() {
            return Err(classify(&log, path));
        }
    }

    let samples: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();

    if samples.is_empty() {
        return Err(if log.is_empty() {
            InferenceError::NoAudioStream
        } else {
            classify(&log, path)
        });
    }

    Ok(Decoded { samples, truncated, backend: "ffmpeg" })
}

/// ffmpeg'in stderr çıktısını istemciye gösterilebilir bir hataya çevirir.
///
/// Ham log **yanıta konmaz**. İçinde sunucunun mutlak yolu geçiyor
/// (`\\?\C:\Users\...\media\x.mp4`) ve arayüz bu metni başlıkta birebir
/// basıyordu: hem bilgi sızıntısı hem de kullanıcının hiçbir şey yapamayacağı
/// çok satırlı bir döküm. Ayrıntı log'a gider, istemciye sebebin insan
/// dilindeki karşılığı döner.
fn classify(log: &str, path: &Path) -> InferenceError {
    let lower = log.to_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| lower.contains(n));

    if has(&["matches no streams", "does not contain any stream"]) {
        return InferenceError::NoAudioStream;
    }
    if has(&["no such file"]) {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("dosya");
        return InferenceError::MediaNotFound(name.to_string());
    }

    tracing::warn!(
        dosya = %path.display(),
        ffmpeg = log.trim(),
        "medya çözümlenemedi"
    );

    if has(&["moov atom not found", "invalid data found", "header parsing failed", "truncat"]) {
        InferenceError::Ffmpeg("dosya bozuk ya da eksik".into())
    } else if has(&["decoder not found", "unknown codec", "unsupported", "no decoder"]) {
        InferenceError::Ffmpeg("bu biçim veya kodek desteklenmiyor".into())
    } else {
        InferenceError::Ffmpeg("ses akışı çözülemedi".into())
    }
}

/// Dosyayı çözmeden süresini okur — yalnız kapsayıcı başlığı.
///
/// Listeleme için gerekli: 24 dosyalık bir klasörde tam çözme dakikalar sürerdi,
/// başlık okuması ise dosya başına birkaç milisaniye. Başlıkta süre yoksa
/// (canlı akış, bozuk dosya) `None` döner ve arayüz "—" gösterir.
pub fn probe_duration(path: &Path) -> Option<f32> {
    let file = File::open(path).ok()?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(extension);
    }

    let format = symphonia::default::get_probe()
        .probe(&hint, stream, FormatOptions::default(), MetadataOptions::default())
        .ok()?;

    // İzlerin en uzunu alınır: kimi dosyada ses izi süreyi bildirmiyor ama video
    // izi bildiriyor. Ölçüldü — yalnız ses izine bakınca 21 dosyanın 4'ü "0 sn"
    // diyordu ve arayüzde "00:00" görünüyordu; bu, süreyi hiç bilmemekten kötü.
    let mut longest = 0.0f32;

    for track in format.tracks() {
        if let Some(time_base) = track.time_base {
            if let Some(time) = track.duration.and_then(|d| time_base.calc_duration(d)) {
                longest = longest.max(time.as_secs_f64() as f32);
            }
        }

        // Kapsayıcı süreyi yazmıyorsa çerçeve sayısından türet.
        if let (Some(frames), Some(CodecParameters::Audio(params))) =
            (track.num_frames, track.codec_params.as_ref())
        {
            if let Some(rate) = params.sample_rate {
                longest = longest.max(frames as f32 / rate as f32);
            }
        }
    }

    // Sıfır "bilinmiyor" demektir; arayüz bunu "—" olarak gösteriyor.
    (longest > 0.0).then_some(longest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampling_preserves_duration_and_amplitude() {
        // 1 saniyelik 440 Hz sinüs, 44.1 kHz
        let from = 44_100;
        let input: Vec<f32> = (0..from)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / from as f32).sin())
            .collect();

        let out = resample(&input, from, SAMPLE_RATE).expect("yeniden örnekleme");

        assert_eq!(out.len(), SAMPLE_RATE, "1 saniye 16 kHz'de 16000 örnek olmalı");

        // 440 Hz, 16 kHz Nyquist'inin (8 kHz) çok altında; genlik korunmalı.
        // Kenar geçişlerini dışlamak için ortadaki bölge ölçülüyor.
        let mid = &out[2000..14000];
        let peak = mid.iter().fold(0.0f32, |acc, v| acc.max(v.abs()));
        assert!((peak - 1.0).abs() < 0.05, "tepe genlik {peak}, ~1.0 bekleniyordu");

        let rms = (mid.iter().map(|v| v * v).sum::<f32>() / mid.len() as f32).sqrt();
        // Sinüsün RMS'i 1/√2 ≈ 0.707
        assert!((rms - 0.707).abs() < 0.02, "RMS {rms}");
    }

    #[test]
    fn resampling_attenuates_above_nyquist() {
        // 12 kHz ton 16 kHz'in Nyquist'inin (8 kHz) üstünde; iyi bir alçak geçiren
        // süzgeç bunu bastırmalı. Bastırmazsa aliasing olarak geri katlanır ve
        // mel spektrumunu bozar.
        let from = 44_100;
        let input: Vec<f32> = (0..from)
            .map(|i| (2.0 * std::f32::consts::PI * 12_000.0 * i as f32 / from as f32).sin())
            .collect();

        let out = resample(&input, from, SAMPLE_RATE).expect("yeniden örnekleme");
        let mid = &out[2000..14000];
        let rms = (mid.iter().map(|v| v * v).sum::<f32>() / mid.len() as f32).sqrt();

        assert!(rms < 0.02, "Nyquist üstü ton bastırılmadı, RMS {rms}");
    }
}
