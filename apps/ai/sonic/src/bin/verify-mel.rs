//! Mel hattı doğrulama kapısı: sherpa-onnx referans çıktılarıyla karşılaştırır.
//!
//! Rust log-mel + CED ONNX hattını, sherpa-onnx'in yayımladığı referans
//! sonuçlarla aynı kurulumda karşılaştırır: **dosyanın tamamı tek pencere**,
//! `top_db` bütünün tepesine göre. Böylece bir uyumsuzluğun mel hatasından mı
//! pencerelemeden mi geldiği ayrıştırılabilir.
//!
//! Ayrıca `prob` çıktısının gerçekten [0,1] aralığında olduğunu doğrular —
//! sigmoid grafiğe gömülü değilse eşikleme sessizce çökerdi.

use std::error::Error;

use sonic::audio::{decode, mel::MelExtractor};
use sonic::config::Config;
use sonic::model::{self, ced::NUM_CLASSES};

/// sherpa-onnx dokümanlarındaki referans etiketler (aynı test_wavs dosyaları).
const EXPECTED: [(u32, &str); 4] =
    [(1, "Cat"), (2, "Whistling"), (3, "Music"), (4, "Laughter")];

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cfg = Config::from_env();

    let labels =
        model::labels::load(&cfg.models_dir.join(&cfg.model).join("class_labels_indices.csv"))?;
    let mut loaded = model::ced::load(&cfg)?;
    let extractor = MelExtractor::new();

    let wav_dir = cfg.models_dir.join(&cfg.model).join("test_wavs");
    println!("Model : {} ({})", loaded.model_name, loaded.weights_file);
    println!("Sağlayıcı: {:?}\n", loaded.providers);

    let mut failures = 0;

    for (n, expected) in EXPECTED {
        let path = wav_dir.join(format!("{n}.wav"));
        let decoded = decode::decode(&path, decode::DEFAULT_MAX_SAMPLES).await?;
        let log_mel = extractor.compute(&decoded.samples);

        // Tüm dosya tek pencere — sherpa ile birebir aynı kurulum.
        let mut feats = Vec::with_capacity(log_mel.n_frames * sonic::audio::mel::N_MELS);
        log_mel.push_window(0, log_mel.n_frames, &mut feats);

        let probs = model::ced::run_batch(&mut loaded.backend, &feats, 1, log_mel.n_frames)?;

        let min = probs.iter().copied().fold(f32::INFINITY, f32::min);
        let max = probs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let sigmoid_ok = (0.0..=1.0).contains(&min) && (0.0..=1.0).contains(&max);

        let mut ranked: Vec<usize> = (0..NUM_CLASSES).collect();
        ranked.sort_unstable_by(|&a, &b| probs[b].total_cmp(&probs[a]));
        let top: Vec<String> = ranked
            .iter()
            .take(5)
            .map(|&i| format!("{} ({:.3})", labels[i].display_name, probs[i]))
            .collect();

        let hit = ranked
            .iter()
            .take(5)
            .any(|&i| labels[i].display_name.contains(expected));

        println!(
            "{}.wav  {:.1}s  {} kare  →  {}",
            n,
            decoded.duration_sec(),
            log_mel.n_frames,
            top.join(", ")
        );
        println!(
            "        beklenen «{expected}»: {}   sigmoid [{min:.4}, {max:.4}]: {}",
            if hit { "BULUNDU" } else { "YOK" },
            if sigmoid_ok { "tamam" } else { "HATALI" }
        );

        if !hit || !sigmoid_ok {
            failures += 1;
        }
    }

    println!();
    if failures == 0 {
        println!("KAPI GEÇİLDİ — mel hattı referansla uyumlu.");
        Ok(())
    } else {
        Err(format!("KAPI GEÇİLEMEDİ — {failures} dosyada uyumsuzluk").into())
    }
}
