use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

// ffmpeg/medya varyantları faz 1-2'de (çözme + analyze API) devreye girecek.
#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum InferenceError {
    #[error("ffmpeg bulunamadı; PATH üzerinde kurulu olmalı")]
    FfmpegMissing,

    #[error("Medya çözümlenemedi: {0}")]
    Ffmpeg(String),

    #[error("Dosyada ses akışı yok")]
    NoAudioStream,

    #[error("Medya dosyası bulunamadı: {0}")]
    MediaNotFound(String),

    #[error("Bu yol SONIC_MEDIA_ROOT dışında, erişim reddedildi")]
    PathNotAllowed,

    #[error("Yalnız video dosyaları yüklenebilir; «{0}» kabul edilmiyor")]
    UnsupportedUpload(String),

    #[error("«{0}» kimliği «{1}» dosyasında kullanılıyor; farklı bir ad verin")]
    IdConflict(String, String),

    #[error("Dosya boyutu sınırı aşıldı ({0} bayt)")]
    UploadTooLarge(u64),

    #[error("Model hatası: {0}")]
    Model(String),

    #[error("Yapılandırma hatası: {0}")]
    Config(String),

    #[error("Giriş/çıkış hatası: {0}")]
    Io(#[from] std::io::Error),
}

impl IntoResponse for InferenceError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::MediaNotFound(_) => StatusCode::NOT_FOUND,
            Self::PathNotAllowed => StatusCode::FORBIDDEN,
            // Çözülemeyen ya da sessiz medya istemci tarafı bir sorundur.
            Self::NoAudioStream | Self::Ffmpeg(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::UnsupportedUpload(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::IdConflict(_, _) => StatusCode::CONFLICT,
            Self::UploadTooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
            Self::FfmpegMissing | Self::Model(_) | Self::Config(_) | Self::Io(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };

        if status == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!("sonic hatası: {}", self);
        }

        (status, Json(json!({ "error": self.to_string() }))).into_response()
    }
}
