//! `optics` — görsel işleme boru hattının komut satırı arayüzü.
//!
//! Servis ayağa kaldırmadan tek tek aşamaları çalıştırmak ve ölçmek için.
//! Faz planındaki her fazın "nihai çıktı" komutu buradan koşar.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use clap::{Parser, Subcommand};
use motif_optics::{check_dependencies, decode_gray, measure_spawn_overhead, probe, AnalysisConfig};

#[derive(Parser)]
#[command(name = "optics", about = "MotifAI görsel işleme araç takımı", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Analiz geçişinin ortak ayarları.
#[derive(clap::Args, Clone, Copy)]
struct ConfigArgs {
    /// Saniyede analiz edilecek kare sayısı.
    #[arg(long, default_value_t = 15.0)]
    analysis_fps: f64,
    /// Analiz karesinin genişliği.
    #[arg(long, default_value_t = 160)]
    width: u32,
    /// Analiz karesinin yüksekliği.
    #[arg(long, default_value_t = 90)]
    height: u32,
}

impl From<ConfigArgs> for AnalysisConfig {
    fn from(a: ConfigArgs) -> Self {
        AnalysisConfig {
            analysis_fps: a.analysis_fps,
            width: a.width,
            height: a.height,
        }
    }
}

#[derive(Subcommand)]
enum Command {
    /// Harici bağımlılıkları (ffmpeg, ffprobe) doğrular.
    Preflight,

    /// Analiz yapılandırmasını ve türetilen değerleri gösterir.
    Config {
        #[command(flatten)]
        cfg: ConfigArgs,
    },

    /// Video metadata bilgisini okur (ffprobe).
    Info {
        /// Video dosyasının yolu.
        path: PathBuf,
    },

    /// Videoyu gri karelere çözer ve throughput ölçer.
    Decode {
        /// Video dosyasının yolu.
        path: PathBuf,
        /// Sadece ilk N kareyi çöz. Verilmezse video sonuna kadar gider.
        #[arg(long)]
        limit: Option<u32>,
        #[command(flatten)]
        cfg: ConfigArgs,
    },

    /// ffmpeg süreç açma maliyetini ölçer.
    ///
    /// Pass 3 yakınlaştırmasının gecikme bütçesi doğrudan bu sayıya bağlı.
    SpawnCost {
        /// Kaç kez ölçülüp ortalanacağı.
        #[arg(long, default_value_t = 10)]
        samples: u32,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Preflight => {
            for (tool, version) in check_dependencies()? {
                println!("  ✓ {:<9} {}", tool.binary(), version);
            }
            println!("\nTüm harici bağımlılıklar hazır.");
        }

        Command::Config { cfg } => {
            let cfg: AnalysisConfig = cfg.into();
            println!("analiz çözünürlüğü : {}x{}", cfg.width, cfg.height);
            println!("analiz kare hızı   : {} fps", cfg.analysis_fps);
            println!("kare başına        : {} bayt", cfg.frame_bytes());
            println!(
                "1 dk video için    : ~{} analiz karesi, ~{:.1} MB akış",
                (cfg.analysis_fps * 60.0) as u64,
                cfg.analysis_fps * 60.0 * cfg.frame_bytes() as f64 / 1_048_576.0,
            );
        }

        Command::Info { path } => {
            let started = Instant::now();
            let info = probe(&path)?;
            let elapsed = started.elapsed();

            println!("dosya       : {}", path.display());
            println!(
                "süre        : {:.3} sn ({} ms)",
                info.duration_ms as f64 / 1000.0,
                info.duration_ms
            );
            println!("kare hızı   : {:.3} fps", info.fps);
            println!("çözünürlük  : {}x{}", info.width, info.height);
            println!("codec       : {}", info.codec);
            println!(
                "boyut       : {:.2} MB",
                info.size_bytes as f64 / 1_048_576.0
            );
            println!(
                "toplam kare : ~{}",
                (info.duration_ms as f64 / 1000.0 * info.fps).round() as u64
            );
            println!("ffprobe     : {:.0} ms", elapsed.as_secs_f64() * 1000.0);
        }

        Command::Decode { path, limit, cfg } => {
            let cfg: AnalysisConfig = cfg.into();
            let info = probe(&path)?;

            let started = Instant::now();
            let frames = decode_gray(&path, cfg)?;

            let mut count: u64 = 0;
            let mut first_frame_at = None;
            // Checksum, karelerin gerçekten okunmasını garanti eder; aksi halde
            // derleyici döngüyü optimize edip ölçümü anlamsız kılabilir.
            let mut checksum: u64 = 0;
            let mut last_t_ms = 0;

            for frame in frames {
                let frame = frame?;
                if first_frame_at.is_none() {
                    first_frame_at = Some(started.elapsed());
                }
                checksum = checksum.wrapping_add(frame.data[0] as u64);
                last_t_ms = frame.t_ms;
                count += 1;
                if limit.is_some_and(|l| count >= l as u64) {
                    break;
                }
            }

            let elapsed = started.elapsed();
            let secs = elapsed.as_secs_f64();
            let decoded_video_ms = last_t_ms as f64 + 1000.0 / cfg.analysis_fps;

            println!("çözülen kare      : {count}");
            println!("son kare zamanı   : {:.3} sn", last_t_ms as f64 / 1000.0);
            println!(
                "ilk kareye kadar  : {:.0} ms   <- ffmpeg açılışı buraya dahil",
                first_frame_at.unwrap_or_default().as_secs_f64() * 1000.0
            );
            println!("toplam süre       : {:.0} ms", secs * 1000.0);
            if secs > 0.0 {
                println!("throughput        : {:.0} kare/sn", count as f64 / secs);
                println!(
                    "gerçek zaman katı : {:.1}x   <- işlenen video süresi / harcanan süre",
                    decoded_video_ms / 1000.0 / secs
                );
            }
            println!(
                "kaynak            : {:.1} sn, {:.3} fps, {}x{}",
                info.duration_ms as f64 / 1000.0,
                info.fps,
                info.width,
                info.height
            );
            println!("checksum          : {checksum}");
        }

        Command::SpawnCost { samples } => {
            let avg = measure_spawn_overhead(samples)?;
            let ms = avg.as_secs_f64() * 1000.0;
            println!("ffmpeg açılış maliyeti : {ms:.0} ms  ({samples} ölçümün ortalaması)");
            println!();
            println!("Bu, ffmpeg iş yapmadan önceki taban maliyet. Pass 1 videoyu bir kez");
            println!("taradığı için bunu bir kez öder. Kare başına ödenirse birikir:");
            println!("30 karelik bir çıkarma tek tek yapılırsa {:.1} sn sadece süreç", ms * 30.0 / 1000.0);
            println!("açmaya gider. Bu yüzden kareler tek çağrıda toplu çıkarılır.");
        }
    }

    Ok(())
}
