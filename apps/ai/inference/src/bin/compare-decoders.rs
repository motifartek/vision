//! Kalite kapısı: symphonia (süreç içi) ile ffmpeg çözücülerinin **aynı** sonucu
//! ürettiğini doğrular.
//!
//! Hız kazancı yalnızca kalite bozulmuyorsa değerlidir. Bu araç iki yolu aynı
//! dosyada koşturur ve karşılaştırır:
//!
//! - örnek sayısı / süre
//! - RMS (genel seviye)
//! - **modelin ilk-5 etiketi ve skorları** — asıl ölçüt bu; PCM birebir aynı
//!   olmasa da (farklı resampler fazı) modelin kararı değişmemeli.
//!
//! Kullanım: `cargo run -p inference --bin compare-decoders -- <dosya...>`

use std::error::Error;
use std::path::PathBuf;
use std::time::Instant;

use inference::audio::decode::{self, DEFAULT_MAX_SAMPLES};
use inference::audio::mel::{MelExtractor, N_MELS};
use inference::config::Config;
use inference::model::{self, ced::NUM_CLASSES};

/// İlk-5 skorlarında kabul edilen en büyük fark.
const SCORE_TOLERANCE: f32 = 0.05;

struct Run {
    label: &'static str,
    pcm: Vec<f32>,
    rms: f32,
    decode_ms: u128,
    top: Vec<(usize, f32)>,
}

/// İki sinyal arasındaki en iyi hizalama kaymasını ve hizalamadan sonra kalan
/// farkı bulur. Saf zaman kayması (ör. AAC kodlayıcı gecikmesi) ile gerçek
/// içerik farkını (farklı kodek çıktısı) ayırt etmek için.
fn alignment(a: &[f32], b: &[f32]) -> (i32, f32, f32) {
    const SEARCH: i32 = 4096;
    const SPAN: usize = 160_000; // 10 s @ 16 kHz

    let base = 16_000usize.min(a.len() / 4); // baştaki sessizliği atla
    let span = SPAN.min(a.len().saturating_sub(base + SEARCH as usize)).max(1);

    let mut best = (0i32, f32::NEG_INFINITY);
    for shift in -SEARCH..=SEARCH {
        let mut dot = 0.0f64;
        for i in 0..span {
            let ai = base + i;
            let bi = ai as i32 + shift;
            if bi < 0 || bi as usize >= b.len() {
                continue;
            }
            dot += (a[ai] as f64) * (b[bi as usize] as f64);
        }
        if dot as f32 > best.1 {
            best = (shift, dot as f32);
        }
    }

    // Hizalamadan sonra kalan fark
    let shift = best.0;
    let (mut num, mut den) = (0.0f64, 0.0f64);
    for i in 0..span {
        let ai = base + i;
        let bi = ai as i32 + shift;
        if bi < 0 || bi as usize >= b.len() {
            continue;
        }
        let d = (a[ai] - b[bi as usize]) as f64;
        num += d * d;
        den += (a[ai] as f64) * (a[ai] as f64);
    }
    let residual_rms = (num / span as f64).sqrt() as f32;
    let relative = if den > 0.0 { (num / den).sqrt() as f32 } else { 0.0 };
    (shift, residual_rms, relative)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let files: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if files.is_empty() {
        return Err("kullanım: compare-decoders <dosya...>".into());
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "inference=info".into()),
        )
        .init();

    let cfg = Config::from_env();
    let labels =
        model::labels::load(&cfg.models_dir.join(&cfg.model).join("class_labels_indices.csv"))?;
    let mut loaded = model::ced::load(&cfg)?;
    let extractor = MelExtractor::new();

    let mut failures = 0;

    for file in &files {
        println!("\n=== {} ===", file.display());

        let mut runs = Vec::new();
        for label in ["symphonia", "ffmpeg"] {
            let started = Instant::now();
            let decoded = if label == "ffmpeg" {
                decode::decode_ffmpeg(file, DEFAULT_MAX_SAMPLES).await?
            } else {
                // decode() symphonia'yı önce dener; kullanılan yolu teyit ediyoruz.
                let d = decode::decode(file, DEFAULT_MAX_SAMPLES).await?;
                if d.backend != "symphonia" {
                    println!("  ! symphonia bu dosyayı çözemedi, ffmpeg'e düştü");
                }
                d
            };
            let decode_ms = started.elapsed().as_millis();

            // Karşılaştırma tek pencerede yapılır; model eğitim uzunluğunun
            // ötesini kabul etmediğinden ilk MAX_WINDOW_FRAMES kare kullanılır.
            let log_mel = extractor.compute(&decoded.samples);
            let frames = log_mel.n_frames.min(model::ced::MAX_WINDOW_FRAMES);
            let mut feats = Vec::with_capacity(frames * N_MELS);
            log_mel.push_window(0, frames, &mut feats);
            let probs = model::ced::run_batch(&mut loaded.session, &feats, 1, frames)?;

            let mut ranked: Vec<usize> = (0..NUM_CLASSES).collect();
            ranked.sort_unstable_by(|&a, &b| probs[b].total_cmp(&probs[a]));

            let rms = (decoded.samples.iter().map(|s| s * s).sum::<f32>()
                / decoded.samples.len() as f32)
                .sqrt();

            runs.push(Run {
                label,
                rms,
                decode_ms,
                top: ranked.into_iter().take(5).map(|i| (i, probs[i])).collect(),
                pcm: decoded.samples,
            });
        }

        let (a, b) = (&runs[0], &runs[1]);

        for run in &runs {
            println!(
                "  {:<10} {:>8} örnek  {:>7.2}s  RMS {:.4}  çözme {:>5} ms",
                run.label,
                run.pcm.len(),
                run.pcm.len() as f32 / decode::SAMPLE_RATE as f32,
                run.rms,
                run.decode_ms
            );
        }

        let (shift, residual, relative) = alignment(&a.pcm, &b.pcm);
        println!(
            "  hizalama: {shift} örnek ({:.1} ms) · hizalama sonrası kalan fark {residual:.5} \
             (sinyalin %{:.2}'si)",
            shift as f32 * 1000.0 / decode::SAMPLE_RATE as f32,
            relative * 100.0
        );

        // Süre farkı: resampler kuyruk davranışı birkaç örnek oynatabilir.
        let sample_delta = (a.pcm.len() as i64 - b.pcm.len() as i64).abs();
        let sample_ok = sample_delta <= 1024;
        let rms_delta = (a.rms - b.rms).abs();
        let rms_ok = rms_delta <= 0.01;

        let labels_ok = a.top.iter().zip(&b.top).all(|((ia, _), (ib, _))| ia == ib);
        let worst_score = a
            .top
            .iter()
            .zip(&b.top)
            .map(|((_, sa), (_, sb))| (sa - sb).abs())
            .fold(0.0f32, f32::max);
        let scores_ok = worst_score <= SCORE_TOLERANCE;

        println!("  ilk-5 (symphonia): {}", format_top(&a.top, &labels));
        println!("  ilk-5 (ffmpeg)   : {}", format_top(&b.top, &labels));
        println!(
            "  örnek farkı {sample_delta} ({}) · RMS farkı {rms_delta:.4} ({}) · \
             etiket sırası {} · en büyük skor farkı {worst_score:.4} ({})",
            yes_no(sample_ok),
            yes_no(rms_ok),
            if labels_ok { "aynı" } else { "FARKLI" },
            yes_no(scores_ok)
        );

        if !(sample_ok && rms_ok && labels_ok && scores_ok) {
            failures += 1;
        }

        if b.decode_ms > 0 {
            println!("  hızlanma: {:.1}×", b.decode_ms as f32 / a.decode_ms.max(1) as f32);
        }
    }

    println!();
    if failures == 0 {
        println!("KAPI GEÇİLDİ — süreç içi çözme, ffmpeg ile aynı model kararını veriyor.");
        Ok(())
    } else {
        Err(format!("KAPI GEÇİLEMEDİ — {failures} dosyada sapma").into())
    }
}

fn format_top(top: &[(usize, f32)], labels: &[model::labels::ClassLabel]) -> String {
    top.iter()
        .map(|(i, s)| format!("{} {:.3}", labels[*i].display_name, s))
        .collect::<Vec<_>>()
        .join(", ")
}

fn yes_no(ok: bool) -> &'static str {
    if ok {
        "tamam"
    } else {
        "SAPMA"
    }
}
