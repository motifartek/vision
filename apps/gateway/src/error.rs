use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GatewayError {
    #[error("Kimlik doğrulanamadı (Unauthorized)")]
    Unauthorized,

    #[error("Bu kaynağa erişim yetkiniz yok (Forbidden)")]
    Forbidden,

    #[error("Kratos servisine ulaşılamadı: {0}")]
    KratosUnreachable(#[from] reqwest::Error),

    #[error("Keto servisine ulaşılamadı: {0}")]
    KetoUnreachable(#[from] tonic::Status),

    #[error("Geçersiz video kimliği")]
    InvalidVideoId,

    #[error("Ses çözümleme servisine ulaşılamadı")]
    InferenceUnreachable,

    #[error("{1}")]
    Upstream(StatusCode, String),

    #[error("Bilinmeyen bir iç sistem hatası oluştu")]
    InternalError,
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            Self::Forbidden => (StatusCode::FORBIDDEN, self.to_string()),
            Self::KratosUnreachable(err) => {
                tracing::error!("Kratos iletişim hatası: {}", err);
                (StatusCode::SERVICE_UNAVAILABLE, "Auth servisi şu an geçici olarak kullanılamıyor.".into())
            }
            Self::KetoUnreachable(err) => {
                tracing::error!("Keto iletişim hatası: {}", err);
                (StatusCode::SERVICE_UNAVAILABLE, "Yetkilendirme servisi yanıt vermiyor.".into())
            }
            Self::InvalidVideoId => (StatusCode::BAD_REQUEST, self.to_string()),
            Self::InferenceUnreachable => {
                (StatusCode::SERVICE_UNAVAILABLE, "Ses çözümleme servisi şu an kullanılamıyor.".into())
            }
            // Inference servisinin durum kodu ve mesajı olduğu gibi aktarılır.
            Self::Upstream(status, message) => (status, message),
            Self::InternalError => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error".into()),
        };

        let body = Json(json!({ "error": message }));
        (status, body).into_response()
    }
}
