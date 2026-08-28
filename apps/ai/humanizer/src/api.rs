use std::sync::Arc;
use axum::{
    extract::{Path, State},
    routing::{delete, get, post, put},
    Json, Router,
    response::{IntoResponse, Response},
    http::StatusCode,
};
use serde::{Deserialize};
use serde_json::json;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use crate::agent::HumanizerAgent;
use motif_prompt::{PromptContext, PromptKind};

#[derive(Debug, Deserialize)]
pub struct EnhanceBody {
    pub report_json: String,
    pub tools: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChatBody {
    pub session_id: String,
    pub video_id: String,
    pub message: String,
    pub tools: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PreviewBody {
    pub kind: String, // "enhance" or "chat"
    pub isitsel: Option<String>,
    pub tools: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DocumentBody {
    pub kind: String, // "dilekce" or "tutanak"
    pub report_json: String,
}

pub fn router(agent: Arc<HumanizerAgent>) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/v1/humanize", post(enhance))
        .route("/v1/chat", post(chat))
        .route("/v1/document", post(generate_document))
        .route("/v1/prompts/preview", post(preview_prompt))
        .route("/v1/prompts", get(list_prompts))
        .route("/v1/prompts/{agent}/{fragment}", put(put_override))
        .route("/v1/prompts/{agent}/{fragment}", delete(delete_override))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(agent)
}

async fn enhance(
    State(agent): State<Arc<HumanizerAgent>>,
    Json(body): Json<EnhanceBody>,
) -> Result<Json<serde_json::Value>, String> {
    let (text, tool_calls, prompt) = agent.enhance_report(&body.report_json, body.tools).await.map_err(|e| e.to_string())?;
    Ok(Json(json!({ "result": text, "tool_calls": tool_calls, "prompt": prompt })))
}

async fn chat(
    State(agent): State<Arc<HumanizerAgent>>,
    Json(body): Json<ChatBody>,
) -> Result<Json<serde_json::Value>, String> {
    let text = agent.chat(&body.session_id, &body.video_id, &body.message, body.tools).await.map_err(|e| e.to_string())?;
    Ok(Json(json!({ "result": text })))
}

async fn generate_document(
    State(agent): State<Arc<HumanizerAgent>>,
    Json(body): Json<DocumentBody>,
) -> Result<Json<serde_json::Value>, String> {
    let prompt_kind = match body.kind.as_str() {
        "dilekce" => PromptKind::HumanizerDilekce,
        "tutanak" => PromptKind::HumanizerTutanak,
        _ => return Err("Geçersiz belge türü".to_string()),
    };
    let (text, _) = agent.generate_document(&body.report_json, prompt_kind).await.map_err(|e| e.to_string())?;
    Ok(Json(json!({ "result": text })))
}

async fn preview_prompt(
    State(agent): State<Arc<HumanizerAgent>>,
    Json(body): Json<PreviewBody>,
) -> Json<serde_json::Value> {
    let kind = if body.kind == "chat" {
        PromptKind::HumanizerChat
    } else {
        PromptKind::HumanizerEnhance
    };

    let mut ctx = PromptContext::new(0);
    if let Some(audio) = body.isitsel {
        ctx = ctx.with_audio(motif_prompt::UntrustedText::new(&audio));
    }
    if let Some(tools) = body.tools {
        ctx = ctx.with_tools(Some(tools));
    }

    let p = agent.preview(kind, &ctx);
    Json(json!({
        "kind": body.kind,
        "text": p.joined()
    }))
}

async fn list_prompts(State(agent): State<Arc<HumanizerAgent>>) -> Json<serde_json::Value> {
    let r = agent.prompts();
    let overrides = r.overrides();
    let parcalar: Vec<serde_json::Value> = r
        .fragments("humanizer")
        .map(|f| {
            f.iter()
                .map(|(ad, parca)| {
                    let ov = overrides
                        .iter()
                        .find(|o| o.agent == "humanizer" && &o.fragment == ad);
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

    Json(json!({ "agent": "humanizer", "fragments": parcalar }))
}

#[derive(Deserialize)]
struct OverrideBody {
    text: String,
    author: String,
}

async fn put_override(
    State(agent): State<Arc<HumanizerAgent>>,
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
            let msg = e.to_string();
            let kod = match e {
                motif_prompt::OverrideError::Store(_) | motif_prompt::OverrideError::NoStore => {
                    StatusCode::SERVICE_UNAVAILABLE
                }
                _ => StatusCode::BAD_REQUEST,
            };
            (kod, msg).into_response()
        }
    }
}

async fn delete_override(
    State(agent): State<Arc<HumanizerAgent>>,
    Path((ajan, parca)): Path<(String, String)>,
) -> Response {
    match agent.prompts().override_sil(&ajan, &parca).await {
        Ok(()) => (StatusCode::NO_CONTENT).into_response(),
        Err(e) => {
            let msg = e.to_string();
            let kod = match e {
                motif_prompt::OverrideError::Store(_) | motif_prompt::OverrideError::NoStore => {
                    StatusCode::SERVICE_UNAVAILABLE
                }
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (kod, msg).into_response()
        }
    }
}