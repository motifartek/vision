//! Harici bağımlılıkların varlığını doğrular.
//!
//! Tasarım gereği OpenCV Rust binding'lerine girmiyoruz; medya işleri
//! `ffmpeg`/`ffprobe` **alt süreç** olarak çalıştırılarak yapılıyor.
//! Bunun bedeli tek bir kurulum adımı, kazancı ise Windows'ta binding
//! derleme derdinin tamamen ortadan kalkması.
//!
//! Servis açılışında [`check_dependencies`] çağrılır ki eksik bağımlılık
//! ilk video yüklendiğinde değil, **hemen** anlaşılsın.

use std::process::Command;

use motif_core::{Error, Result};

/// İhtiyaç duyduğumuz harici çalıştırılabilirler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalTool {
    Ffmpeg,
    Ffprobe,
}

impl ExternalTool {
    pub const fn binary(self) -> &'static str {
        match self {
            ExternalTool::Ffmpeg => "ffmpeg",
            ExternalTool::Ffprobe => "ffprobe",
        }
    }

    fn hint(self) -> &'static str {
        "ffmpeg'i kurup PATH'e ekleyin. \
         Windows: https://www.gyan.dev/ffmpeg/builds/ · \
         Linux: apt install ffmpeg · macOS: brew install ffmpeg"
    }

    /// Sürüm satırını döndürür, bulunamazsa hata verir.
    pub fn version(self) -> Result<String> {
        let output = Command::new(self.binary())
            .arg("-version")
            .output()
            .map_err(|_| Error::MissingDependency {
                name: self.binary().to_string(),
                hint: self.hint().to_string(),
            })?;

        if !output.status.success() {
            return Err(Error::CommandFailed {
                command: format!("{} -version", self.binary()),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().next().unwrap_or_default().trim().to_string())
    }
}

/// Tüm harici bağımlılıkları kontrol eder ve sürümlerini döndürür.
///
/// İlk eksik bağımlılıkta durur; hata mesajı kullanıcıya ne yapması
/// gerektiğini söyler.
pub fn check_dependencies() -> Result<Vec<(ExternalTool, String)>> {
    [ExternalTool::Ffmpeg, ExternalTool::Ffprobe]
        .into_iter()
        .map(|tool| tool.version().map(|v| (tool, v)))
        .collect()
}
