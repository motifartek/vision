//! HTTP yüzeyi.
//!
//! İki farklı tüketici var ve ayrımı korumak önemli:
//!
//! - `/v1/videos/...` — **operatör/test arayüzü** uçları. Yükleme, listeleme,
//!   profil görselleştirme.
//! - `/v1/tools/{tool}` — **ajan** uçları. Gövdeleri `motif-event-sdk` içindeki
//!   kontratlar; NATS istek/cevap tüketicisi de aynı fonksiyonları çağırır.
//!
//! Araçların HTTP üzerinden de açılmasının sebebi test arayüzü: ajan olmadan
//! `zoom_range` denenebilsin, davranış gözle görülebilsin.

use std::sync::Arc;

use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use motif_core::VideoId;
use motif_event_sdk::tools::{names, ToolError, ToolErrorCode};
use motif_optics::{motion_chart, ChartOptions};
use serde::Deserialize;
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::state::AppState;
use crate::{catalog, pipeline, tools};

/// API hatası.
struct ApiError {
    status: StatusCode,
    message: String,
    code: Option<ToolErrorCode>,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            code: None,
        }
    }
}

impl From<motif_core::Error> for ApiError {
    fn from(err: motif_core::Error) -> Self {
        let status = match &err {
            motif_core::Error::NotFound(_) => StatusCode::NOT_FOUND,
            motif_core::Error::Config(_) | motif_core::Error::InvalidVideo(_) => {
                StatusCode::BAD_REQUEST
            }
            motif_core::Error::MissingDependency { .. } => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self::new(status, err.to_string())
    }
}

impl From<ToolError> for ApiError {
    fn from(err: ToolError) -> Self {
        let status = match err.code {
            ToolErrorCode::UnknownVideo => StatusCode::NOT_FOUND,
            ToolErrorCode::OutOfRange | ToolErrorCode::InvalidArgument => StatusCode::BAD_REQUEST,
            // Sınır aşımı geçici değil, bilinçli bir politika: ajan bunu
            // "sonra tekrar dene" diye okumamalı.
            ToolErrorCode::ZoomLimitExceeded => StatusCode::TOO_MANY_REQUESTS,
            ToolErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self {
            status,
            message: err.message,
            code: Some(err.code),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut body = json!({ "error": self.message });
        if let Some(code) = self.code {
            body["code"] = json!(code);
        }
        (self.status, Json(body)).into_response()
    }
}

type ApiResult<T> = std::result::Result<T, ApiError>;

pub fn router(state: Arc<AppState>) -> Router {
    let max_upload = state.config.max_upload_bytes;

    Router::new()
        .route("/", get(test_ui))
        .route("/healthz", get(healthz))
        .route("/v1/videos", get(list_videos).post(upload_video))
        .route("/v1/videos/{id}", get(get_video))
        .route("/v1/videos/{id}", delete(delete_video))
        .route("/v1/videos/{id}/profile", get(get_profile))
        .route("/v1/videos/{id}/profile.svg", get(get_profile_svg))
        .route("/v1/videos/{id}/overview", post(post_overview))
        .route("/v1/tools/{tool}", post(call_tool))
        .route("/v1/blobs/{*key}", get(get_blob))
        .layer(DefaultBodyLimit::max(max_upload))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Test arayüzü.
///
/// Derleme zamanında ikiliye gömülüyor: ayrı bir sunucu, npm kurulumu ya da
/// build adımı olmadan `cargo run -p stream` sonrası tarayıcıda açılabilsin.
/// Amacı ajanı beklemeden davranışı gözle görmek — özellikle yakınlaştırmanın
/// gerçekten işe yarayıp yaramadığını.
async fn test_ui() -> Response {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        include_str!("../static/index.html"),
    )
        .into_response()
}

async fn healthz(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "nats": state.events.is_enabled(),
        "tools": names::ALL,
        "config": {
            "overview_budget": state.config.overview_budget,
            "zoom_budget": state.config.zoom_budget,
            "max_zooms_per_video": state.config.max_zooms_per_video,
            "uniform_prior": state.config.sampling.uniform_prior,
            "analysis_fps": state.config.analysis.analysis_fps,
            "timestamp_overlay": state.config.timestamp_overlay,
        }
    }))
}

// --- video uçları ---

async fn list_videos(State(state): State<Arc<AppState>>) -> ApiResult<Json<Value>> {
    let videos = catalog::list(state.store.as_ref())?;
    Ok(Json(json!({ "videos": videos })))
}

async fn upload_video(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> ApiResult<Json<Value>> {
    let mut bytes: Option<Vec<u8>> = None;
    let mut filename = String::from("video.mp4");

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, format!("yükleme okunamadı: {e}")))?
    {
        if field.name() != Some("file") {
            continue;
        }
        if let Some(name) = field.file_name() {
            filename = name.to_string();
        }
        let data = field.bytes().await.map_err(|e| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                format!("dosya gövdesi okunamadı: {e}"),
            )
        })?;
        bytes = Some(data.to_vec());
    }

    let bytes = bytes.ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "`file` alanı bulunamadı (multipart/form-data bekleniyor)",
        )
    })?;

    if bytes.is_empty() {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "dosya boş"));
    }

    let record = pipeline::ingest(&state, bytes, filename).await?;
    Ok(Json(json!(record)))
}

async fn get_video(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let id = VideoId::from(id);
    let record = catalog::VideoRecord::load(state.store.as_ref(), &id)?;
    let used = state.zoom_count(&id).await;

    Ok(Json(json!({
        "video": record,
        // Test arayüzü ajanın yakınlaştırma bütçesini tüketişini görebilsin.
        "zooms_used": used,
        "zooms_remaining": state.config.max_zooms_per_video.saturating_sub(used),
    })))
}

async fn delete_video(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let id = VideoId::from(id);

    // Olmayan videoyu silmek sessizce başarılı dönmemeli: test arayüzünde
    // yanlış kimlik yazıldığında "silindi" demek yanıltıcı olurdu.
    if !catalog::VideoRecord::exists(state.store.as_ref(), &id) {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            format!("bilinmeyen video: {id}"),
        ));
    }

    catalog::delete(state.store.as_ref(), &id)?;
    state.forget(&id).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct ProfileQuery {
    /// Örnekleri bu genişlikte kovalara indirger.
    bucket_ms: Option<u64>,
}

async fn get_profile(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(q): Query<ProfileQuery>,
) -> ApiResult<Json<Value>> {
    let id = VideoId::from(id);
    let profile = pipeline::profile(&state, &id).await?;

    match q.bucket_ms {
        Some(bucket) if bucket > 0 => {
            let buckets: Vec<Value> = profile
                .bucketed(bucket)
                .into_iter()
                .map(|(t_ms, score, is_scene_cut)| json!({
                    "t_ms": t_ms,
                    "score": score,
                    "is_scene_cut": is_scene_cut,
                }))
                .collect();
            Ok(Json(json!({
                "duration_ms": profile.duration_ms,
                "analysis_fps": profile.analysis_fps,
                "buckets": buckets,
            })))
        }
        _ => Ok(Json(json!(*profile))),
    }
}

async fn get_profile_svg(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Response> {
    let id = VideoId::from(id);
    let profile = pipeline::profile(&state, &id).await?;
    let svg = motion_chart(&profile, ChartOptions::default());

    Ok((
        [(header::CONTENT_TYPE, "image/svg+xml; charset=utf-8")],
        svg,
    )
        .into_response())
}

#[derive(Deserialize)]
struct OverviewBody {
    budget: Option<usize>,
}

async fn post_overview(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    body: Option<Json<OverviewBody>>,
) -> ApiResult<Json<Value>> {
    let id = VideoId::from(id);
    let budget = body.and_then(|b| b.budget);

    state.reset_zooms(&id).await;
    let frames = pipeline::overview(&state, &id, budget).await?;
    Ok(Json(json!({ "frames": frames })))
}

// --- ajan araç ucu ---

/// Tek bir uçtan tüm araçları dağıtır.
///
/// Ajan çerçeveleri araçları isimle çağırır; her araca ayrı yol açmak yerine
/// tek bir dağıtıcı, NATS tarafındaki `stream.tool.<ad>` desenine de birebir
/// karşılık geliyor.
async fn call_tool(
    State(state): State<Arc<AppState>>,
    Path(tool): Path<String>,
    Json(payload): Json<Value>,
) -> ApiResult<Json<Value>> {
    let value = dispatch(&state, &tool, payload).await?;
    Ok(Json(value))
}

/// Araç adına göre çağrıyı yönlendirir. HTTP ve NATS ortak kullanır.
pub async fn dispatch(
    state: &Arc<AppState>,
    tool: &str,
    payload: Value,
) -> std::result::Result<Value, ToolError> {
    fn decode<T: serde::de::DeserializeOwned>(
        payload: Value,
    ) -> std::result::Result<T, ToolError> {
        serde_json::from_value(payload).map_err(|e| ToolError {
            code: ToolErrorCode::InvalidArgument,
            message: format!("istek gövdesi çözümlenemedi: {e}"),
        })
    }

    fn encode<T: serde::Serialize>(value: T) -> std::result::Result<Value, ToolError> {
        serde_json::to_value(value).map_err(|e| ToolError {
            code: ToolErrorCode::Internal,
            message: e.to_string(),
        })
    }

    match tool {
        names::VIDEO_INFO => encode(tools::video_info(state, decode(payload)?).await?),
        names::MOTION_PROFILE => encode(tools::motion_profile(state, decode(payload)?).await?),
        names::SAMPLE_OVERVIEW => encode(tools::sample_overview(state, decode(payload)?).await?),
        names::ZOOM_RANGE => encode(tools::zoom_range(state, decode(payload)?).await?),
        names::GET_FRAME => encode(tools::get_frame(state, decode(payload)?).await?),
        names::CROP_REGION => encode(tools::crop_region(state, decode(payload)?).await?),
        other => Err(ToolError {
            code: ToolErrorCode::InvalidArgument,
            message: format!(
                "bilinmeyen araç: {other}. Kullanılabilir: {}",
                names::ALL.join(", ")
            ),
        }),
    }
}

// --- nesne sunumu ---

async fn get_blob(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> ApiResult<Response> {
    let bytes = state.store.get(&key)?;
    let mime = mime_guess::from_path(&key)
        .first_or_octet_stream()
        .to_string();

    Ok(([(header::CONTENT_TYPE, mime)], bytes).into_response())
}
