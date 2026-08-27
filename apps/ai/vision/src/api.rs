//! HTTP yüzeyi.
//!
//! İki biçim sunuluyor: zenginleştirilmiş rapor (dahili alanlarla) ve
//! şartnamenin §5'te verdiği dar teslim biçimi. Jüriye giden şeyin ne olduğu
//! konusunda şüphe kalmasın diye ikincisi ayrı bir uçta duruyor.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::extract::Path;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use motif_event_sdk::ClipRef;
use motif_prompt::{PromptContext, PromptKind, UntrustedText};

use crate::agent::{AgentError, VisionAgent};

#[derive(Debug, Deserialize)]
pub struct AnalyzeBody {
    pub video_id: String,
    /// `sonic`'in ses analizinden çıkan özet.
    ///
    /// İsteğe bağlı: verilmezse prompt bayt bayt eskisiyle aynı kalıyor, yani
    /// ses hattı bağlanmamış bir kurulumda davranış değişmiyor. Orchestrator
    /// bu alanı doldurduğunda metin prompt'un ayraçlı güvenilmez bölgesine
    /// giriyor — modelin kendi çıktısı bir sonraki prompt'un talimatı
    /// olmasın diye (tasarım §K7).
    #[serde(default)]
    pub isitsel_baglam: Option<String>,
}

impl AnalyzeBody {
    /// Ses bağlamını güvenilmez metne çevirir.
    ///
    /// Boş ya da yalnızca boşluktan oluşan bir değer `None` sayılıyor: aksi
    /// hâlde içi boş bir "güvenilmez bağlam" bölümü açılır, bağlamı şişirir
    /// ve modele söyleyecek şeyi olmayan bir bölüm gösterirdi.
    fn isitsel(&self) -> Option<UntrustedText> {
        self.isitsel_baglam
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(UntrustedText::new)
    }
}

pub fn router(agent: Arc<VisionAgent>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/analyze", post(analyze))
        .route("/v1/analyze/sartname", post(analyze_sartname))
        .route("/v1/prompts/preview", post(preview_prompt))
        .route("/v1/prompts", get(list_prompts))
        .route("/v1/prompts/{agent}/{fragment}", put(put_override))
        .route("/v1/prompts/{agent}/{fragment}", delete(delete_override))
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
    let outcome = agent.analyze(&body.video_id, body.isitsel()).await?;
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
    let outcome = agent.analyze(&body.video_id, body.isitsel()).await?;
    Ok(Json(outcome.report.to_sartname_json()))
}

/// Prompt önizleme isteği.
///
/// Klip bilgisi **dışarıdan** veriliyor, servis onu üretmiyor: önizleme yan
/// etkisiz ve ucuz olmalı. Klibi zaten `stream` üretiyor; panel onun döndürdüğü
/// değerleri buraya taşıyor.
#[derive(Debug, Deserialize)]
pub struct PreviewBody {
    pub duration_ms: u64,
    /// Verilirse yakınlaştırma istemi, verilmezse genel bakış istemi.
    #[serde(default)]
    pub clip: Option<ClipRef>,
    /// Örnek ses bağlamı.
    ///
    /// Önizlemede de kabul ediliyor ki yönetici güvenilmez bölgenin nasıl
    /// render edildiğini — ve ayraç kaçırmanın çalıştığını — göndermeden
    /// görebilsin.
    #[serde(default)]
    pub isitsel_baglam: Option<String>,
}

/// Modele gidecek metni gönderilmeden üretir.
///
/// Panelin gösterdiği metin ile modele gidenin ayrışmaması için tek yol bu:
/// ajanın kendi render'ı çağrılıyor. Önceden `stream` kendi prompt'unu
/// üretiyordu ve ikisi ayrışmıştı — panel gönderilmeyen bir metni
/// "tam olarak bu gidiyor" diye gösteriyordu.
async fn preview_prompt(
    State(agent): State<Arc<VisionAgent>>,
    Json(body): Json<PreviewBody>,
) -> Json<serde_json::Value> {
    let kind = if body.clip.is_some() {
        PromptKind::VisionYakinlastirma
    } else {
        PromptKind::VisionIlkBakis
    };

    let mut ctx = PromptContext::new(body.duration_ms);
    if let Some(clip) = body.clip {
        ctx = ctx.with_clip(clip);
    }
    if let Some(ses) = body
        .isitsel_baglam
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        ctx = ctx.with_audio(UntrustedText::new(ses));
    }

    let p = agent.preview(kind, &ctx);
    let metin = p.joined();

    Json(json!({
        "kind": match kind {
            PromptKind::VisionIlkBakis => "ilk_bakis",
            PromptKind::VisionYakinlastirma => "yakinlastirma",
        },
        "prefix": p.prefix,
        "suffix": p.suffix,
        "joined": metin,
        "version": p.version,
        // Türkçe metinde kabaca bir token'a dört karakter düşüyor.
        "text_tokens": metin.chars().count() / 4,
    }))
}

/// Katalog ve etkin override'lar.
///
/// Arayüz her parçanın gömülü metnini, varsa üstüne binen düzenlemeyi ve
/// düzenlenebilir olup olmadığını birlikte gösteriyor — fark görünümü buna
/// dayanıyor.
async fn list_prompts(State(agent): State<Arc<VisionAgent>>) -> Json<serde_json::Value> {
    let r = agent.prompts();
    let overrides = r.overrides();
    let parcalar: Vec<serde_json::Value> = r
        .fragments("vision")
        .map(|f| {
            f.iter()
                .map(|(ad, parca)| {
                    let ov = overrides
                        .iter()
                        .find(|o| o.agent == "vision" && &o.fragment == ad);
                    json!({
                        "fragment": ad,
                        "editable": parca.editable,
                        "embedded": parca.text,
                        "override": ov.map(|o| json!({
                            "text": o.text,
                            "author": o.author,
                            "updated_at": o.updated_at,
                        })),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Json(json!({ "agent": "vision", "fragments": parcalar }))
}

#[derive(Debug, Deserialize)]
pub struct OverrideBody {
    pub text: String,
    #[serde(default = "bilinmeyen_yazar")]
    pub author: String,
}

fn bilinmeyen_yazar() -> String {
    "bilinmiyor".to_string()
}

/// Bir parçayı override eder.
///
/// Doğrulamadan geçmezse **400** döner ve kayıt yapılmaz: bozuk bir prompt'un
/// depoya girmesine izin verilmiyor.
async fn put_override(
    State(agent): State<Arc<VisionAgent>>,
    Path((ajan, parca)): Path<(String, String)>,
    Json(body): Json<OverrideBody>,
) -> Response {
    let o = motif_prompt::PromptOverride {
        id: format!("{ajan}/{parca}"),
        agent: ajan,
        fragment: parca,
        text: body.text,
        author: body.author,
        updated_at: String::new(),
    };

    match agent.prompts().override_kaydet(o).await {
        Ok(()) => (StatusCode::NO_CONTENT).into_response(),
        Err(e) => {
            let kod = match e {
                motif_prompt::OverrideError::Store(_) | motif_prompt::OverrideError::NoStore => {
                    StatusCode::SERVICE_UNAVAILABLE
                }
                _ => StatusCode::BAD_REQUEST,
            };
            tracing::warn!(hata = %e, "override reddedildi");
            (kod, Json(json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Override'ı siler; parça gömülü hâline döner.
async fn delete_override(
    State(agent): State<Arc<VisionAgent>>,
    Path((ajan, parca)): Path<(String, String)>,
) -> Response {
    match agent.prompts().override_sil(&ajan, &parca).await {
        Ok(()) => (StatusCode::NO_CONTENT).into_response(),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn govde(ses: Option<&str>) -> AnalyzeBody {
        AnalyzeBody {
            video_id: "v1".into(),
            isitsel_baglam: ses.map(str::to_string),
        }
    }

    /// Alan hiç verilmezse ses bağlamı yok sayılmalı.
    ///
    /// `#[serde(default)]` olmadan eski istemciler 422 alırdı; orchestrator
    /// ve panel bu alanı bilmeden de çağırabilmeli.
    #[test]
    fn alan_verilmezse_ses_yok() {
        let g: AnalyzeBody = serde_json::from_str(r#"{"video_id":"v1"}"#).unwrap();
        assert!(g.isitsel().is_none());
    }

    /// Boş ya da yalnız boşluktan oluşan değer bölge açmamalı.
    ///
    /// Aksi hâlde `sonic` hiçbir şey duymadığında prompt'a içi boş bir
    /// "güvenilmez bağlam" bölümü girer: bağlamı şişirir ve modele söyleyecek
    /// şeyi olmayan bir bölüm gösterir.
    #[test]
    fn bos_deger_bolge_acmaz() {
        assert!(govde(Some("")).isitsel().is_none());
        assert!(govde(Some("   \n\t ")).isitsel().is_none());
    }

    #[test]
    fn dolu_deger_guvenilmez_metne_cevriliyor() {
        assert!(govde(Some("cam kırıldı")).isitsel().is_some());
    }
}
