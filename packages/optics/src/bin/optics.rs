//! `optics` — görsel işleme boru hattının komut satırı arayüzü.
//!
//! Servis ayağa kaldırmadan tek tek aşamaları çalıştırmak ve ölçmek için.
//! Faz planındaki her fazın "nihai çıktı" komutu buradan koşar.

use anyhow::Result;
use clap::{Parser, Subcommand};
use motif_optics::{check_dependencies, AnalysisConfig};

#[derive(Parser)]
#[command(
    name = "optics",
    about = "MotifAI görsel işleme araç takımı",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Harici bağımlılıkları (ffmpeg, ffprobe) doğrular.
    Preflight,

    /// Analiz yapılandırmasını ve türetilen değerleri gösterir.
    Config {
        /// Saniyede analiz edilecek kare sayısı.
        #[arg(long, default_value_t = 15.0)]
        analysis_fps: f64,
        /// Analiz karesinin genişliği.
        #[arg(long, default_value_t = 160)]
        width: u32,
        /// Analiz karesinin yüksekliği.
        #[arg(long, default_value_t = 90)]
        height: u32,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Preflight => {
            let tools = check_dependencies()?;
            for (tool, version) in tools {
                println!("  ✓ {:<9} {}", tool.binary(), version);
            }
            println!("\nTüm harici bağımlılıklar hazır.");
        }

        Command::Config {
            analysis_fps,
            width,
            height,
        } => {
            let cfg = AnalysisConfig {
                analysis_fps,
                width,
                height,
            };
            println!("analiz çözünürlüğü : {}x{}", cfg.width, cfg.height);
            println!("analiz kare hızı   : {} fps", cfg.analysis_fps);
            println!("kare başına        : {} bayt", cfg.frame_bytes());
            println!(
                "1 dk video için    : ~{} analiz karesi, ~{:.1} MB akış",
                (cfg.analysis_fps * 60.0) as u64,
                cfg.analysis_fps * 60.0 * cfg.frame_bytes() as f64 / 1_048_576.0,
            );
        }
    }

    Ok(())
}
