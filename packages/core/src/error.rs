use thiserror::Error;

/// Tüm MotifAI servislerinin paylaştığı hata yüzeyi.
///
/// Servise özel hatalar kendi crate'lerinde tanımlanır ve gerekirse
/// `#[from]` ile buraya sarılır.
#[derive(Debug, Error)]
pub enum Error {
    #[error("G/Ç hatası: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serileştirme hatası: {0}")]
    Serde(#[from] serde_json::Error),

    /// Harici bir çalıştırılabilir (ffmpeg, ffprobe) PATH üzerinde bulunamadı.
    #[error("Gerekli harici bağımlılık bulunamadı: {name}. {hint}")]
    MissingDependency { name: String, hint: String },

    /// Harici komut çalıştı ama sıfırdan farklı çıkış kodu döndürdü.
    #[error("Harici komut başarısız oldu ({command}): {stderr}")]
    CommandFailed { command: String, stderr: String },

    #[error("Geçersiz veya okunamayan video: {0}")]
    InvalidVideo(String),

    #[error("Bulunamadı: {0}")]
    NotFound(String),

    #[error("Yapılandırma hatası: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, Error>;
