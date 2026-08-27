//! HTTP yüzeyi.
//!
//! İki biçim sunuluyor: zenginleştirilmiş rapor (dahili alanlarla) ve
//! şartnamenin §5'te verdiği dar teslim biçimi. Jüriye giden şeyin ne olduğu
//! konusunda şüphe kalmasın diye ikincisi ayrı bir uçta duruyor.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::agent::{AgentError, VisionAgent};

#[derive(Debug, Deserialize)]
pub struct AnalyzeBody {
    pub video_id: String,
}

pub fn router(agent: Arc<VisionAgent>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/analyze", post(analyze))
        .route("/v1/analyze/sartname", post(analyze_sartname))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(agent)
}

async fn healthz() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "max_zoom": crate::agent::MAX_ZOOM,
        "zoom_budget": crate::agent::ZOOM_BUDGET,
    }))
}

/// Tam rapor: olay başına `t_ms` ve `severity`, ajanın attığı adımlar.
async fn analyze(
    State(agent): State<Arc<VisionAgent>>,
    Json(body): Json<AnalyzeBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let outcome = agent.analyze(&body.video_id).await?;
    Ok(Json(json!({
        "report": outcome.report,
        "steps": outcome.steps,
    })))
}

/// Şartname §5 teslim biçimi. Dahili alanlar bu uçtan çıkmaz.
async fn analyze_sartname(
    State(agent): State<Arc<VisionAgent>>,
    Json(body): Json<AnalyzeBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let outcome = agent.analyze(&body.video_id).await?;
    Ok(Json(outcome.report.to_sartname_json()))
}

pub struct ApiError(AgentError);

impl From<AgentError> for ApiError {
    fn from(e: AgentError) -> Self {
        Self(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        use crate::stream_client::StreamError;

        // İstemci hatası ile sunucu hatası ayrılıyor: olmayan bir video kimliği
        // 404, servis erişilemezliği 502.
        let (kod, tur) = match &self.0 {
            AgentError::Stream(StreamError::Status { status: 404, .. }) => {
                (StatusCode::NOT_FOUND, "not_found")
            }
            AgentError::Stream(StreamError::Status { status: 400, .. }) => {
                (StatusCode::BAD_REQUEST, "invalid_argument")
            }
            AgentError::Stream(_) => (StatusCode::BAD_GATEWAY, "stream_unavailable"),
            AgentError::Vlm(_) => (StatusCode::BAD_GATEWAY, "vlm_unavailable"),
            AgentError::NoReport => (StatusCode::UNPROCESSABLE_ENTITY, "no_report"),
        };

        tracing::warn!(hata = %self.0, "analiz başarısız");
        (kod, Json(json!({"code": tur, "error": self.0.to_string()}))).into_response()
    }
}
