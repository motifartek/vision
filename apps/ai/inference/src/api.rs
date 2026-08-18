use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::{extract::State, routing::{get, post}, Json, Router};
use ort::session::Session;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::analysis::{self, AnalyzeParams};
use crate::contract::Analysis;
use crate::audio::mel::MelExtractor;
use crate::config::{profile, Config, DEFAULT_PROFILE, PROFILES};
use crate::error::InferenceError;
use crate::model::labels::ClassLabel;

pub struct AppState {
    pub labels: Arc<Vec<ClassLabel>>,
    pub model_name: String,
    pub weights_file: String,
    pub providers: Vec<&'static str>,
    /// `Session::run` `&mut self` istiyor; ağır iş `spawn_blocking` içinde
    /// koştuğu için std Mutex (tokio Mutex'i değil) doğru araç.
    pub session: Arc<Mutex<Session>>,
    pub mel: Arc<MelExtractor>,
    pub media_root: Option<PathBuf>,
    pub default_batch: usize,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/labels", get(labels))
        .route("/v1/audio/analyze", post(analyze))
        .route("/v1/videos", get(crate::upload::list_videos))
        .route("/v1/upload", post(crate::upload::upload_video))
        .with_state(state)
}

async fn healthz(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "model": {
            "name": state.model_name,
            "weights": state.weights_file,
            "providers": state.providers,
            "classes": state.labels.len(),
        },
        "profiles": PROFILES,
        "default_profile": DEFAULT_PROFILE,
        "batch_size": state.default_batch,
    }))
}

async fn labels(State(state): State<Arc<AppState>>) -> Json<Vec<ClassLabel>> {
    Json((*state.labels).clone())
}

#[derive(Debug, Deserialize)]
pub struct AnalyzeRequest {
    pub path: String,
    pub profile: Option<String>,
    pub threshold: Option<f32>,
    pub top_k: Option<usize>,
    pub min_duration_sec: Option<f32>,
    pub gap_sec: Option<f32>,
    pub max_events: Option<usize>,
    pub include_frames: Option<bool>,
    pub batch_size: Option<usize>,
}

impl AnalyzeRequest {
    fn into_params(self, state: &AppState) -> Result<(String, AnalyzeParams), InferenceError> {
        let defaults = AnalyzeParams::default();
        let name = self.profile.as_deref().unwrap_or(DEFAULT_PROFILE);
        let profile = profile(name).ok_or_else(|| {
            InferenceError::Config(format!(
                "bilinmeyen profil «{name}»; geçerli olanlar: {}",
                PROFILES.iter().map(|p| p.name).collect::<Vec<_>>().join(", ")
            ))
        })?;

        Ok((
            self.path,
            AnalyzeParams {
                profile,
                threshold: self.threshold.unwrap_or(defaults.threshold).clamp(0.01, 0.99),
                top_k: self.top_k.unwrap_or(defaults.top_k).clamp(1, 20),
                min_duration_sec: self.min_duration_sec.unwrap_or(defaults.min_duration_sec).max(0.0),
                gap_sec: self.gap_sec.unwrap_or(defaults.gap_sec).max(0.0),
                max_events: self.max_events.unwrap_or(defaults.max_events).clamp(1, 5000),
                include_frames: self.include_frames.unwrap_or(defaults.include_frames),
                batch_size: self.batch_size.unwrap_or(state.default_batch).clamp(1, 1024),
            },
        ))
    }
}

async fn analyze(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AnalyzeRequest>,
) -> Result<Json<Analysis>, InferenceError> {
    let (raw_path, params) = request.into_params(&state)?;
    let path = resolve_media_path(&state, &raw_path)?;

    let decode_started = Instant::now();
    let decoded = analysis::decode_media(&path).await?;
    let decode_ms = decode_started.elapsed().as_millis();

    let session = state.session.clone();
    let mel = state.mel.clone();
    let labels = state.labels.clone();
    let model_name = state.model_name.clone();
    let weights = state.weights_file.clone();
    let providers = state.providers.clone();

    let result = tokio::task::spawn_blocking(move || {
        let mut session = session.lock().map_err(|_| {
            InferenceError::Model("model oturumu önceki bir çökme sonrası kullanılamaz".into())
        })?;
        analysis::analyze_decoded(
            &decoded,
            &mel,
            &mut session,
            &labels,
            &params,
            &model_name,
            &weights,
            &providers,
            decode_ms,
        )
    })
    .await
    .map_err(|e| InferenceError::Config(format!("çözümleme görevi tamamlanamadı: {e}")))??;

    tracing::info!(
        dosya = %path.display(),
        sure_sn = result.media.duration_sec,
        pencere = result.model.windows,
        olay = result.events.len(),
        toplam_ms = result.timing.total_ms,
        gercek_zaman_kat = result.timing.realtime_factor,
        "çözümleme tamamlandı"
    );

    Ok(Json(result))
}

/// `INFERENCE_MEDIA_ROOT` ayarlıysa istenen yol bu kökün dışına çıkamaz.
/// Mutlak yollar da `join` tarafından kökü ezdiği için aynı denetime takılır.
fn resolve_media_path(state: &AppState, raw: &str) -> Result<PathBuf, InferenceError> {
    let requested = Path::new(raw);
    let joined = match &state.media_root {
        Some(root) => root.join(requested),
        None => requested.to_path_buf(),
    };

    let canonical = std::fs::canonicalize(&joined)
        .map_err(|_| InferenceError::MediaNotFound(raw.to_string()))?;

    if let Some(root) = &state.media_root {
        let root = std::fs::canonicalize(root).map_err(|e| {
            InferenceError::Config(format!("INFERENCE_MEDIA_ROOT çözümlenemedi: {e}"))
        })?;
        if !canonical.starts_with(&root) {
            return Err(InferenceError::PathNotAllowed);
        }
    }

    Ok(canonical)
}

impl AppState {
    pub fn new(
        cfg: &Config,
        labels: Vec<ClassLabel>,
        session: Session,
        model_name: String,
        weights_file: String,
        providers: Vec<&'static str>,
    ) -> Self {
        Self {
            labels: Arc::new(labels),
            model_name,
            weights_file,
            providers,
            session: Arc::new(Mutex::new(session)),
            mel: Arc::new(MelExtractor::new()),
            media_root: cfg.media_root.clone(),
            default_batch: cfg.batch_size,
        }
    }
}
