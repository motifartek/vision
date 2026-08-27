use axum::{
    extract::{Path, Query, State},
    Json,
};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::AuthenticatedUser;
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
    /// Pencere başına kaç sınıf dönsün. Panelin "şu an duyulan" bölümü bunu
    /// 6 olarak veriyor; burada taşınmazsa sessizce sonic'in varsayılanına
    /// düşer ve panel istediğinden farklı bir liste alır.
    pub top_k: Option<usize>,
}

/// Video kimliği doğrudan dosya adına çevrildiği için yalnız güvenli karakterler
/// kabul edilir; aksi halde `../` ile sonic'ın medya kökünden çıkılabilirdi.
fn is_safe_video_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Bir videonun ses olaylarını döndürür.
///
/// Oturum aranıyor; asıl çözümlemeyi yerel sonic servisi yapıyor.
///
/// # Video başına yetki neden yok
///
/// Burada `check_permission(…, "Video", &video_id, "view")` çağrısı vardı ve
/// **her istek 403 dönüyordu**: Keto'nun `Video` namespace'i boş, kimseye
/// `viewers` ilişkisi yazan bir kod yolu yok — yükleme vekili bilinçli olarak
/// kimlik doğrulamasız bırakıldığı için gateway videoyu kimin yüklediğini zaten
/// bilmiyor. Üstelik namespace adı da yanlıştı (`"videos"`, oysa yapılandırmada
/// `Video`), yani çağrı var olmayan bir namespace'i sorguluyordu.
///
/// Aynı videonun analiz raporunu yayınlayan kardeş uç
/// (`GET /api/videos/:id/events`, SSE) hiçbir yetki kontrolü yapmıyor. Yani bu
/// kapı kapalı tutulduğunda korunan bir şey yoktu — yalnız ses paneli boş
/// kalıyordu.
///
/// Sahiplik bağlandığında (yüklemede `Video:<id>#viewers@<identity>` kaydı)
/// kontrol tek satırla geri gelir.
pub async fn audio_events(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(video_id): Path<String>,
    Query(query): Query<AudioEventsQuery>,
) -> Result<Json<Value>, GatewayError> {
    if !is_safe_video_id(&video_id) {
        return Err(GatewayError::InvalidVideoId);
    }

    tracing::debug!(
        kullanici = %user.identity_id,
        video = %video_id,
        "ses olayları istendi"
    );

    // Uzantı **eklenmiyor**: sonic servisi uzantısız kimliği medya kökünde
    // kendisi çözüyor (`upload::find_by_id`). `.mp4` varsaymak, mkv/webm/mov
    // olarak yüklenen her videoyu "dosya bulunamadı" ile kırıyordu.
    let mut body = json!({ "path": video_id });
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
    if let Some(top_k) = query.top_k {
        map.insert("top_k".into(), json!(top_k));
    }

    let response = state
        .sonic
        .client
        .post(format!("{}/v1/audio/analyze", state.sonic.base_url))
        .json(&body)
        .send()
        .await
        .map_err(|err| {
            tracing::error!("sonic servisine ulaşılamadı: {}", err);
            GatewayError::InferenceUnreachable
        })?;

    let status = response.status();
    let payload: Value = response.json().await.map_err(|err| {
        tracing::error!("sonic yanıtı çözümlenemedi: {}", err);
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

/// AudioSet sınıf tablosunu (527 satır) döndürür.
///
/// Panel bu tabloyu olayların önem derecesini boyamak için istiyordu ama
/// doğrudan sonic'e gidiyordu; mimaride dışarıya açılan tek kapı gateway
/// olduğu için (bkz. `documents/architecture/agentic-macro-loop.md`) uç
/// buraya taşındı.
///
/// Kaynağa özgü bir yetki kontrolü yok: tablo statik bir referans, videoya
/// ya da kullanıcıya bağlı hiçbir veri taşımıyor. Yine de oturum aranıyor,
/// çünkü kapının arkasındaki hiçbir uç anonim olmamalı.
pub async fn audio_labels(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> Result<Json<Value>, GatewayError> {
    let response = state
        .sonic
        .client
        .get(format!("{}/v1/labels", state.sonic.base_url))
        .send()
        .await
        .map_err(|err| {
            tracing::error!("sonic servisine ulaşılamadı: {}", err);
            GatewayError::InferenceUnreachable
        })?;

    let status = response.status();
    let payload: Value = response.json().await.map_err(|err| {
        tracing::error!("sonic etiket yanıtı çözümlenemedi: {}", err);
        GatewayError::InternalError
    })?;

    if !status.is_success() {
        return Err(GatewayError::Upstream(
            status,
            "etiket tablosu alınamadı".to_string(),
        ));
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
