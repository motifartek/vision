use axum::{
    extract::{Path, Query, State},
    Json,
};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::AuthenticatedUser;
use crate::authz::check_permission;
use crate::error::GatewayError;
use crate::AppState;

#[derive(Clone)]
pub struct InferenceState {
    pub client: Client,
    pub base_url: String,
}

#[derive(Debug, Deserialize)]
pub struct AudioEventsQuery {
    pub profile: Option<String>,
    pub threshold: Option<f32>,
    pub include_frames: Option<bool>,
}

/// Video kimliği doğrudan dosya adına çevrildiği için yalnız güvenli karakterler
/// kabul edilir; aksi halde `../` ile inference'ın medya kökünden çıkılabilirdi.
fn is_safe_video_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Bir videonun ses olaylarını döndürür.
///
/// Kimlik doğrulama ve Keto yetki kontrolü `stream_video` ile aynı deseni izler;
/// asıl çözümlemeyi yerel inference servisi yapar.
pub async fn audio_events(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(video_id): Path<String>,
    Query(query): Query<AudioEventsQuery>,
) -> Result<Json<Value>, GatewayError> {
    if !is_safe_video_id(&video_id) {
        return Err(GatewayError::InvalidVideoId);
    }

    let has_access =
        check_permission(&state.authz, &user.identity_id, "videos", &video_id, "view").await?;
    if !has_access {
        return Err(GatewayError::Forbidden);
    }

    let mut body = json!({ "path": format!("{video_id}.mp4") });
    let map = body.as_object_mut().expect("nesne olarak kuruldu");
    if let Some(profile) = query.profile {
        map.insert("profile".into(), json!(profile));
    }
    if let Some(threshold) = query.threshold {
        map.insert("threshold".into(), json!(threshold));
    }
    if let Some(include_frames) = query.include_frames {
        map.insert("include_frames".into(), json!(include_frames));
    }

    let response = state
        .inference
        .client
        .post(format!("{}/v1/audio/analyze", state.inference.base_url))
        .json(&body)
        .send()
        .await
        .map_err(|err| {
            tracing::error!("inference servisine ulaşılamadı: {}", err);
            GatewayError::InferenceUnreachable
        })?;

    let status = response.status();
    let payload: Value = response.json().await.map_err(|err| {
        tracing::error!("inference yanıtı çözümlenemedi: {}", err);
        GatewayError::InternalError
    })?;

    if !status.is_success() {
        let message = payload
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("ses çözümlemesi başarısız")
            .to_string();
        return Err(GatewayError::Upstream(status, message));
    }

    Ok(Json(payload))
}

#[cfg(test)]
mod tests {
    use super::is_safe_video_id;

    #[test]
    fn rejects_path_traversal_and_separators() {
        assert!(is_safe_video_id("podcast-highlight-03"));
        assert!(is_safe_video_id("video_01"));
        assert!(!is_safe_video_id(""));
        assert!(!is_safe_video_id(".."));
        assert!(!is_safe_video_id("../../etc/passwd"));
        assert!(!is_safe_video_id("a/b"));
        assert!(!is_safe_video_id("a\\b"));
        assert!(!is_safe_video_id("video.mp4"));
        assert!(!is_safe_video_id(&"x".repeat(129)));
    }
}
