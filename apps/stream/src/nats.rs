//! Araçların NATS istek/cevap üzerinden sunulması.
//!
//! `#6`'da kararlaştırıldığı gibi ajan araçları NATS istek/cevap ile
//! çağrılıyor: broker zaten çalışıyor, ayrı bir servis keşif katmanı
//! gerekmiyor. HTTP yüzeyi de duruyor ama o test arayüzü içindir.
//!
//! Dinleyici `stream.tool.*` desenine abone olur ve gövdeyi HTTP ile **aynı**
//! dağıtıcıya verir; iş mantığı tek yerde kalır.

use std::sync::Arc;

use futures::StreamExt;
use motif_event_sdk::subjects;
use motif_event_sdk::tools::{ToolError, ToolErrorCode};
use serde_json::Value;

use crate::api::dispatch;
use crate::state::AppState;

/// Araç dinleyicisini arka planda başlatır.
///
/// NATS yapılandırılmadıysa sessizce hiçbir şey yapmaz: servis broker olmadan
/// da tam işlevsel çalışır.
pub fn serve_tools(state: Arc<AppState>) {
    let Some(client) = state.events.client().cloned() else {
        tracing::info!("NATS kapalı; araçlar yalnızca HTTP üzerinden sunuluyor");
        return;
    };

    tokio::spawn(async move {
        let pattern = format!("{}*", subjects::TOOL_PREFIX);

        let mut subscription = match client.subscribe(pattern.clone()).await {
            Ok(sub) => sub,
            Err(err) => {
                tracing::error!(%pattern, %err, "araç aboneliği kurulamadı");
                return;
            }
        };

        tracing::info!(%pattern, "araç dinleyicisi hazır");

        while let Some(message) = subscription.next().await {
            let Some(reply) = message.reply.clone() else {
                // İstek/cevap deseni dışında gelen mesaj; cevaplanacak yer yok.
                tracing::warn!(subject = %message.subject, "reply adresi olmayan araç çağrısı");
                continue;
            };

            let tool = message
                .subject
                .strip_prefix(subjects::TOOL_PREFIX)
                .unwrap_or_default()
                .to_string();

            let state = state.clone();
            let client = client.clone();

            // Her çağrı ayrı görevde: uzun süren bir yakınlaştırma, sıradaki
            // araç isteklerini bekletmesin.
            tokio::spawn(async move {
                let payload: Value = serde_json::from_slice(&message.payload).unwrap_or(Value::Null);

                let response = match dispatch(&state, &tool, payload).await {
                    Ok(value) => value,
                    Err(err) => error_payload(&err),
                };

                let bytes = serde_json::to_vec(&response).unwrap_or_else(|e| {
                    serde_json::to_vec(&error_payload(&ToolError {
                        code: ToolErrorCode::Internal,
                        message: e.to_string(),
                    }))
                    .unwrap_or_default()
                });

                if let Err(err) = client.publish(reply, bytes.into()).await {
                    tracing::warn!(%tool, %err, "araç cevabı gönderilemedi");
                }
            });
        }

        tracing::warn!("araç aboneliği kapandı");
    });
}

/// Araç hatasını cevap gövdesine çevirir.
///
/// Hata da geçerli bir cevaptır: ajan `error` alanını okuyup başka bir aralık
/// deneyebilmeli, zaman aşımına düşmemeli.
fn error_payload(err: &ToolError) -> Value {
    serde_json::json!({
        "error": {
            "code": err.code,
            "message": err.message,
        }
    })
}
