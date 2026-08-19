//! Çözme → log-mel → pencereleme → batch çıkarım → segmentasyon boru hattı.

use std::path::Path;
use std::time::Instant;


use crate::contract::{Analysis, FrameTop, MediaInfo, ModelInfo, Timing};
use crate::audio::decode::{self, DEFAULT_MAX_SAMPLES};
use crate::audio::mel::{LogMel, MelExtractor, HOP_LENGTH, N_MELS, SAMPLE_RATE};
use crate::config::WindowProfile;
use crate::error::InferenceError;
use crate::events::{self, SegmentParams};
use crate::model::ced::{self, NUM_CLASSES};
use crate::model::labels::ClassLabel;
use crate::safety::{self, SafetyParams};

/// log-mel kare hızı: 16000 / 160 = saniyede 100 kare.
const FRAMES_PER_SEC: f32 = SAMPLE_RATE as f32 / HOP_LENGTH as f32;

#[derive(Debug, Clone)]
pub struct AnalyzeParams {
    pub profile: WindowProfile,
    pub threshold: f32,
    pub top_k: usize,
    pub min_duration_sec: f32,
    pub gap_sec: f32,
    pub max_events: usize,
    pub include_frames: bool,
    pub batch_size: usize,
}

impl Default for AnalyzeParams {
    fn default() -> Self {
        Self {
            profile: crate::config::PROFILES[1], // dengeli
            threshold: 0.35,
            top_k: 5,
            min_duration_sec: 0.0,
            gap_sec: 0.5,
            max_events: 500,
            include_frames: false,
            batch_size: 32,
        }
    }
}

/// ffmpeg alt süreci beklerken tokio çalışanını bloke etmemek için çözme async.
pub async fn decode_media(path: &Path) -> Result<decode::Decoded, InferenceError> {
    decode::decode(path, DEFAULT_MAX_SAMPLES).await
}

/// CPU yoğun kısım; çağıran `spawn_blocking` içinde çalıştırmalı.
#[allow(clippy::too_many_arguments)]
pub fn analyze_decoded(
    decoded: &decode::Decoded,
    extractor: &MelExtractor,
    session: &mut ort::session::Session,
    labels: &[ClassLabel],
    params: &AnalyzeParams,
    model_name: &str,
    weights: &str,
    providers: &[&'static str],
    decode_ms: u128,
) -> Result<Analysis, InferenceError> {
    let started = Instant::now();
    let duration_sec = decoded.duration_sec();

    let mel_start = Instant::now();
    let log_mel = extractor.compute(&decoded.samples);
    let mel_ms = mel_start.elapsed().as_millis();

    let window_frames = (params.profile.window_sec * FRAMES_PER_SEC).round() as usize;
    let hop_frames = (params.profile.hop_sec * FRAMES_PER_SEC).round().max(1.0) as usize;
    let n_windows = window_count(log_mel.n_frames, window_frames, hop_frames);

    let infer_start = Instant::now();
    let scores = run_windows(session, &log_mel, window_frames, hop_frames, n_windows, params)?;
    let inference_ms = infer_start.elapsed().as_millis();

    let segment_start = Instant::now();
    let (mut events, summary) = events::segment(
        &scores,
        n_windows,
        labels,
        &SegmentParams {
            threshold: params.threshold,
            // Histerezis çıkış eşiği girişin %60'ı: eşik civarında salınan
            // skorların olayı parçalamasını engeller.
            release: params.threshold * 0.6,
            min_duration_sec: params.min_duration_sec,
            gap_sec: params.gap_sec,
            window_sec: params.profile.window_sec,
            hop_sec: params.profile.hop_sec,
            duration_sec,
        },
    );
    let segment_ms = segment_start.elapsed().as_millis();

    // Güvenlik kuralları **kırpma öncesi** tam liste üzerinde koşar: kırpma
    // güven sırasına baktığı için düşük güvenli ama gerçek bir alarm, gürültülü
    // bir kayıtta elenip bulgusuyla birlikte yok olabiliyordu. Kırpma yalnız
    // istemciye giden listeyi küçültür ve güvenlik sınıflarını muaf tutar.
    let safety = safety::analyze(&events, &SafetyParams::default());
    let events_truncated = events::cap_events(&mut events, params.max_events);
    if events_truncated {
        tracing::debug!(sinir = params.max_events, "olay listesi kırpıldı");
    }

    let frames = params
        .include_frames
        .then(|| frame_tops(&scores, n_windows, params));

    let total_ms = decode_ms + started.elapsed().as_millis();

    Ok(Analysis {
        media: MediaInfo {
            duration_sec,
            sample_rate: SAMPLE_RATE,
            truncated: decoded.truncated,
            decoder: decoded.backend,
        },
        model: ModelInfo {
            name: model_name.to_string(),
            weights: weights.to_string(),
            providers: providers.to_vec(),
            classes: labels.len(),
            profile: params.profile.name,
            window_sec: params.profile.window_sec,
            hop_sec: params.profile.hop_sec,
            windows: n_windows,
            batch_size: params.batch_size,
        },
        events,
        events_truncated,
        summary,
        safety,
        frames,
        timing: Timing {
            decode_ms,
            mel_ms,
            inference_ms,
            segment_ms,
            total_ms,
            realtime_factor: if total_ms == 0 {
                f32::INFINITY
            } else {
                duration_sec / (total_ms as f32 / 1000.0)
            },
        },
    })
}

/// Sinyalin sonunu da kapsayacak kadar pencere; son pencere gerekirse taşar ve
/// `push_window` tarafından taban değerle doldurulur.
fn window_count(n_frames: usize, window_frames: usize, hop_frames: usize) -> usize {
    if n_frames <= window_frames {
        1
    } else {
        // Son pencerenin başlangıcı sinyalin içinde kalmalı.
        (n_frames - window_frames).div_ceil(hop_frames) + 1
    }
}

/// GPU belleğinin yetmediğini gösteren hata imzaları.
///
/// Farklı sağlayıcılar farklı mesaj veriyor: DirectML `887A0006` (cihaz askıda)
/// ya da `887A0005`, CUDA `out of memory` / `cudaMalloc`. Hepsi aynı çareyi
/// gerektiriyor: daha küçük batch.
fn is_memory_pressure(err: &InferenceError) -> bool {
    let msg = err.to_string().to_lowercase();
    ["887a0006", "887a0005", "out of memory", "cudamalloc", "oom", "device hung"]
        .iter()
        .any(|marker| msg.contains(marker))
}

fn run_windows(
    session: &mut ort::session::Session,
    log_mel: &LogMel,
    window_frames: usize,
    hop_frames: usize,
    n_windows: usize,
    params: &AnalyzeParams,
) -> Result<Vec<f32>, InferenceError> {
    // Batch boyutu makinenin VRAM'ine bağlı ve bunu önceden bilemeyiz: aynı
    // model 24 GB'lık kartta 256'yı kaldırırken 6 GB'lık kartta 256'da çöküyor
    // (ölçüldü). Bellek hatası gelirse batch'i yarıya indirip yeniden dene —
    // böylece kullanıcı hiçbir ayar bilmeden kendi donanımına oturur.
    let mut batch_size = params.batch_size.max(1);

    loop {
        match run_all_batches(session, log_mel, window_frames, hop_frames, n_windows, batch_size) {
            Ok(scores) => {
                if batch_size != params.batch_size {
                    tracing::info!(
                        istenen = params.batch_size,
                        kullanilan = batch_size,
                        "batch boyutu donanıma göre düşürüldü"
                    );
                }
                return Ok(scores);
            }
            Err(err) if is_memory_pressure(&err) && batch_size > 1 => {
                let reduced = (batch_size / 2).max(1);
                tracing::warn!(
                    onceki = batch_size,
                    yeni = reduced,
                    "GPU belleği yetmedi, batch küçültülüyor"
                );
                batch_size = reduced;
            }
            Err(err) => return Err(err),
        }
    }
}

fn run_all_batches(
    session: &mut ort::session::Session,
    log_mel: &LogMel,
    window_frames: usize,
    hop_frames: usize,
    n_windows: usize,
    batch_size: usize,
) -> Result<Vec<f32>, InferenceError> {
    let mut scores = Vec::with_capacity(n_windows * NUM_CLASSES);
    let mut feats = Vec::with_capacity(batch_size * N_MELS * window_frames);

    for chunk_start in (0..n_windows).step_by(batch_size) {
        let batch = batch_size.min(n_windows - chunk_start);

        feats.clear();
        for w in chunk_start..chunk_start + batch {
            log_mel.push_window(w * hop_frames, window_frames, &mut feats);
        }

        let probs = ced::run_batch(session, &feats, batch, window_frames)?;
        scores.extend_from_slice(&probs);
    }

    Ok(scores)
}

fn frame_tops(scores: &[f32], n_windows: usize, params: &AnalyzeParams) -> Vec<FrameTop> {
    (0..n_windows)
        .map(|w| {
            let row = &scores[w * NUM_CLASSES..(w + 1) * NUM_CLASSES];
            // Tüm sınıfları sıralamak yerine yalnız ilk-K'yı seç: 527 yerine
            // K elemanlık kısmi sıralama, 2000+ pencerede hissedilir fark.
            let mut ranked: Vec<usize> = (0..NUM_CLASSES).collect();
            let k = params.top_k.min(NUM_CLASSES);
            ranked.select_nth_unstable_by(k - 1, |&a, &b| row[b].total_cmp(&row[a]));
            ranked.truncate(k);
            ranked.sort_unstable_by(|&a, &b| row[b].total_cmp(&row[a]));

            FrameTop {
                t: w as f32 * params.profile.hop_sec,
                // Skoru 3 haneye yuvarla: JSON'da gereksiz ondalık taşımayalım.
                top: ranked.into_iter().map(|i| (i, (row[i] * 1000.0).round() / 1000.0)).collect(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_cover_the_whole_signal() {
        // 10 s sinyal (1001 kare), 2 s pencere (200), 0.5 s adım (50)
        let n = window_count(1001, 200, 50);
        // Son pencerenin başlangıcı 1001-200=801 kareyi geçmeli
        assert!((n - 1) * 50 >= 801 - 50, "n={n}");
        assert_eq!(n, 18);
    }

    #[test]
    fn short_signal_yields_single_window() {
        assert_eq!(window_count(50, 200, 50), 1);
        assert_eq!(window_count(200, 200, 50), 1);
    }
}
