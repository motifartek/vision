//! `bench` — ölçüm harness'ı.
//!
//! Şartname §4 katılımcıların kendi metriklerini tanımlamasını ve sonuçları
//! demo ile raporlarda açıkça sunmasını zorunlu tutuyor. Bu araç o sayıları
//! üretir.
//!
//! ```text
//! bench generate --out data/fixtures/events      # veri kümesini üret
//! bench run      --dataset data/fixtures/events  # tek ayarla ölç
//! bench sweep    --dataset data/fixtures/events --param alpha
//! ```

mod dataset;
mod metrics;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use motif_optics::{AnalysisConfig, SamplingConfig};

use crate::metrics::{Aggregate, VideoMetrics};

#[derive(Parser)]
#[command(name = "bench", about = "MotifAI stream ölçüm harness'ı", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Örnekleme ayarları (run ve sweep ortak).
#[derive(clap::Args, Clone, Copy)]
struct SamplingArgs {
    #[arg(long, default_value_t = 16)]
    budget: usize,
    #[arg(long, default_value_t = 0.25)]
    alpha: f32,
    #[arg(long, default_value_t = 3)]
    dedup: u32,
    /// Gürültü tabanını düşme.
    #[arg(long)]
    raw_motion: bool,
    /// Sahne kesitlerini seçime zorla dahil etme.
    ///
    /// Ablasyon için: kesitler açıkken adım tipi olaylar zaten yakalandığı
    /// için α'nın katkısı görünmez olur.
    #[arg(long)]
    no_scene_cuts: bool,
    /// Bir olayın kapsanmış sayılması için izin verilen sapma.
    #[arg(long, default_value_t = 1000)]
    tolerance_ms: u64,
    #[arg(long, default_value_t = 15.0)]
    analysis_fps: f64,
}

impl SamplingArgs {
    fn sampling(&self) -> SamplingConfig {
        SamplingConfig {
            budget: self.budget,
            uniform_prior: self.alpha,
            dedup_hamming: self.dedup,
            force_scene_cuts: !self.no_scene_cuts,
            subtract_noise_floor: !self.raw_motion,
        }
    }

    fn analysis(&self) -> AnalysisConfig {
        AnalysisConfig {
            analysis_fps: self.analysis_fps,
            ..Default::default()
        }
    }
}

#[derive(Subcommand)]
enum Command {
    /// Sentetik veri kümesini üretir (ffmpeg gerekir).
    Generate {
        #[arg(long, default_value = "data/fixtures/events")]
        out: PathBuf,
    },

    /// Veri kümesini tek bir ayarla ölçer.
    Run {
        #[arg(long, default_value = "data/fixtures/events")]
        dataset: PathBuf,
        /// Sonuçları JSON olarak buraya yaz.
        #[arg(long)]
        json: Option<PathBuf>,
        #[command(flatten)]
        args: SamplingArgs,
    },

    /// Bir parametreyi süpürüp etkisini ölçer.
    Sweep {
        #[arg(long, default_value = "data/fixtures/events")]
        dataset: PathBuf,
        /// Süpürülecek parametre: alpha, budget, dedup, noise-floor.
        #[arg(long, default_value = "alpha")]
        param: String,
        #[command(flatten)]
        args: SamplingArgs,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Generate { out } => {
            println!("Sentetik veri kümesi üretiliyor: {}\n", out.display());
            let produced = dataset::generate(&out)?;
            let toplam_olay: usize = produced.iter().map(|t| t.events.len()).sum();
            println!(
                "\n{} video, {} olay. Ground truth JSON dosyaları aynı dizinde.",
                produced.len(),
                toplam_olay
            );
        }

        Command::Run { dataset, json, args } => {
            let items = dataset::load_dataset(&dataset)?;
            let results = olc(&items, &dataset, args)?;
            rapor_bas(&results, args);

            if let Some(path) = json {
                let agg = Aggregate::from(&results);
                let cikti = serde_json::json!({
                    "ayarlar": {
                        "budget": args.budget,
                        "alpha": args.alpha,
                        "dedup_hamming": args.dedup,
                        "subtract_noise_floor": !args.raw_motion,
                        "tolerance_ms": args.tolerance_ms,
                        "analysis_fps": args.analysis_fps,
                    },
                    "videolar": results,
                    "toplam": agg,
                });
                std::fs::write(&path, serde_json::to_vec_pretty(&cikti)?)?;
                println!("\nJSON yazıldı: {}", path.display());
            }
        }

        Command::Sweep {
            dataset,
            param,
            args,
        } => {
            let items = dataset::load_dataset(&dataset)?;
            supur(&items, &dataset, &param, args)?;
        }
    }

    Ok(())
}

fn olc(
    items: &[dataset::GroundTruth],
    dir: &std::path::Path,
    args: SamplingArgs,
) -> Result<Vec<VideoMetrics>> {
    items
        .iter()
        .map(|truth| {
            metrics::evaluate(
                truth,
                dir,
                args.analysis(),
                args.sampling(),
                args.tolerance_ms,
            )
        })
        .collect()
}

fn rapor_bas(results: &[VideoMetrics], args: SamplingArgs) {
    println!(
        "bütçe {} · α {} · dedup {} · gürültü tabanı {} · sahne kesiti {} · tolerans {} ms\n",
        args.budget,
        args.alpha,
        args.dedup,
        if args.raw_motion { "kapalı" } else { "açık" },
        if args.no_scene_cuts { "kapalı" } else { "açık" },
        args.tolerance_ms
    );

    println!(
        "{:<18} {:>7} {:>8} {:>7} {:>8} {:>8} {:>7} {:>8}",
        "video", "olay", "kapsama", "kare", "azaltma", "boşluk", "yanlış", "hız"
    );
    println!("{}", "-".repeat(78));

    for r in results {
        let kapsama = match r.recall() {
            Some(v) => format!("{:.0}%", v * 100.0),
            None => "—".to_string(),
        };
        let bosluk = if r.gap_violated() {
            format!("{:.1}s!", r.max_gap_ms as f64 / 1000.0)
        } else {
            format!("{:.1}s", r.max_gap_ms as f64 / 1000.0)
        };

        println!(
            "{:<18} {:>7} {:>8} {:>7} {:>7.0}x {:>8} {:>7} {:>7.0}x",
            r.video.trim_end_matches(".mp4"),
            format!("{}/{}", r.events_covered, r.events_total),
            kapsama,
            r.selected_frames,
            r.reduction_ratio,
            bosluk,
            r.false_scene_cuts,
            r.realtime_factor
        );
    }

    let agg = Aggregate::from(results);
    println!("{}", "-".repeat(78));
    println!(
        "{:<18} {:>7} {:>8} {:>7.1} {:>7.0}x {:>8} {:>7} {:>7.0}x",
        "TOPLAM",
        format!("{}/{}", agg.events_covered, agg.events_total),
        format!("{:.0}%", agg.recall * 100.0),
        agg.mean_frames,
        agg.mean_reduction,
        "",
        agg.false_scene_cuts,
        agg.mean_realtime
    );

    println!("\nkapsanan olaylarda ortalama sapma : {} ms", agg.mean_offset_ms);
    println!("boşluk garantisi ihlali            : {}", agg.gap_violations);
    println!("toplam işlem süresi                : {} ms", agg.total_ms);

    // Kaçırılan olaylar tek tek gösterilir: hangi senaryonun zorlandığı
    // toplam yüzdeden çok daha bilgilendirici.
    let kacan: Vec<&VideoMetrics> = results
        .iter()
        .filter(|r| r.events_covered < r.events_total)
        .collect();

    if !kacan.is_empty() {
        println!("\nkaçırılan olaylar:");
        for r in kacan {
            println!(
                "  {:<18} {}/{} kapsandı, en yakın kare {} ms uzakta",
                r.video.trim_end_matches(".mp4"),
                r.events_covered,
                r.events_total,
                r.worst_miss_ms.unwrap_or(0)
            );
        }
    }
}

fn supur(
    items: &[dataset::GroundTruth],
    dir: &std::path::Path,
    param: &str,
    base: SamplingArgs,
) -> Result<()> {
    let degerler: Vec<(String, SamplingArgs)> = match param {
        "alpha" => [0.0f32, 0.1, 0.25, 0.5, 0.75, 1.0]
            .iter()
            .map(|&a| {
                let mut args = base;
                args.alpha = a;
                (format!("α={a}"), args)
            })
            .collect(),
        "budget" => [4usize, 8, 12, 16, 24, 32]
            .iter()
            .map(|&b| {
                let mut args = base;
                args.budget = b;
                (format!("bütçe={b}"), args)
            })
            .collect(),
        "dedup" => [0u32, 1, 3, 6, 12]
            .iter()
            .map(|&d| {
                let mut args = base;
                args.dedup = d;
                (format!("dedup={d}"), args)
            })
            .collect(),
        "scene-cuts" => [false, true]
            .iter()
            .map(|&kapali| {
                let mut args = base;
                args.no_scene_cuts = kapali;
                (
                    format!("kesit={}", if kapali { "kapalı" } else { "açık" }),
                    args,
                )
            })
            .collect(),
        "noise-floor" => [false, true]
            .iter()
            .map(|&raw| {
                let mut args = base;
                args.raw_motion = raw;
                (
                    format!("taban={}", if raw { "kapalı" } else { "açık" }),
                    args,
                )
            })
            .collect(),
        other => anyhow::bail!(
            "bilinmeyen parametre: {other}. Seçenekler: alpha, budget, dedup, noise-floor, scene-cuts"
        ),
    };

    println!("Süpürme: {param}\n");
    println!(
        "{:<14} {:>9} {:>8} {:>9} {:>9} {:>8} {:>7}",
        "ayar", "kapsama", "kare", "azaltma", "sapma", "yanlış", "ihlal"
    );
    println!("{}", "-".repeat(70));

    for (etiket, args) in degerler {
        let results = olc(items, dir, args)?;
        let agg = Aggregate::from(&results);
        println!(
            "{:<14} {:>8.0}% {:>8.1} {:>8.0}x {:>7} ms {:>8} {:>7}",
            etiket,
            agg.recall * 100.0,
            agg.mean_frames,
            agg.mean_reduction,
            agg.mean_offset_ms,
            agg.false_scene_cuts,
            agg.gap_violations
        );
    }

    println!("\nkapsama : olayların yüzde kaçına tolerans içinde kare düştü");
    println!("azaltma : kaynak kare / seçilen kare");
    println!("sapma   : kapsanan olaylarda kare ile olay arası ortalama uzaklık");
    println!("yanlış  : hiçbir olaya denk gelmeyen sahne kesiti sayısı");
    println!("ihlal   : α'dan türeyen boşluk garantisinin aşıldığı video sayısı");

    Ok(())
}
