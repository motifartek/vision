mod audio;
mod auth;
mod authz;
mod error;
mod proxy;

use audio::InferenceState;
use auth::{AuthState, AuthenticatedUser};
use authz::{check_permission, AuthzState, keto::check_service_client::CheckServiceClient};
use axum::{
    extract::{Path, State},
    http::Method,
    routing::{get, any},
    Router,
    response::sse::{Event, Sse},
};
use axum_prometheus::PrometheusMetricLayer;
use error::GatewayError;
use moka::future::Cache;
use proxy::{kratos_proxy_handler, stream_proxy_handler};
use reqwest::Client;
use std::time::Duration;
use tonic::transport::Channel;
use tower_http::cors::CorsLayer;
use std::convert::Infallible;
use tokio_stream::Stream;
use sqlx::Row;
use serde_json::json;

#[derive(Clone)]
pub struct AppState {
    pub auth: AuthState,
    pub authz: AuthzState,
    pub sonic: InferenceState,
    /// `stream` servisinin adresi. Videolar buradan geçiyor.
    pub stream_url: String,
    /// Toolbox API servisinin adresi.
    pub toolbox_url: String,
    /// Ayrı istemci: video gövdeleri büyük, zaman aşımı Kratos'unkinden uzun.
    pub stream_client: Client,
    pub db_pool: sqlx::PgPool,
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

async fn stream_video(
    State(state): State<AppState>,
    user: AuthenticatedUser, // Kimlik doğrulandı
    Path(video_id): Path<String>,
) -> Result<String, GatewayError> {
    let has_access = check_permission(
        &state.authz,
        &user.identity_id,
        "videos", // namespace
        &video_id,
        "view",   // relation
    )
    .await?;

    if !has_access {
        return Err(GatewayError::Forbidden);
    }

    tracing::info!(
        kullanici = %user.identity_id,
        video = %video_id,
        "video akışı istendi ama uç nokta henüz uygulanmadı"
    );

    Err(GatewayError::NotImplemented(
        "Video akışı henüz gateway üzerinden servis edilmiyor.",
    ))
}

async fn get_video_events(
    State(state): State<AppState>,
    Path(video_id): Path<String>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    tracing::info!("SSE stream baslatiliyor: {}", video_id);
    
    let mut listener = sqlx::postgres::PgListener::connect_with(&state.db_pool).await.unwrap();
    listener.listen("ai_events").await.unwrap();
    listener.listen("ai_trace").await.unwrap();
    listener.listen("tool_alerts").await.unwrap();

    // Veritabanindaki mevcut son durumu cek
    let initial_row = sqlx::query("SELECT summary, events, risk, actions FROM ai_events WHERE video_id = $1")
        .bind(&video_id)
        .fetch_optional(&state.db_pool)
        .await
        .unwrap_or(None);

    let stream = async_stream::stream! {
        // Istemci baglanir baglanmaz eger onceden analiz varsa hemen gonder
        if let Some(row) = initial_row {
            let data = json!({
                "summary": row.try_get::<String, _>("summary").unwrap_or_default(),
                "events": row.try_get::<serde_json::Value, _>("events").unwrap_or_default(),
                "risk": row.try_get::<String, _>("risk").unwrap_or_default(),
                "actions": row.try_get::<serde_json::Value, _>("actions").unwrap_or_default(),
            });
            yield Ok(Event::default().event("report").data(data.to_string()));
        }

        loop {
            match listener.recv().await {
                Ok(notification) => {
                    let payload = notification.payload();
                    
                    if notification.channel() == "tool_alerts" {
                        if let Ok(alert_data) = serde_json::from_str::<serde_json::Value>(payload) {
                            if alert_data["video_id"].as_str() == Some(video_id.as_str()) {
                                yield Ok(Event::default().event("alert").data(payload));
                            }
                        }
                    } else if notification.channel() == "ai_trace" {
                        // Gelen trace payload: {"video_id": "...", "message": "..."}
                        if let Ok(trace_data) = serde_json::from_str::<serde_json::Value>(payload) {
                            if trace_data["video_id"].as_str() == Some(video_id.as_str()) {
                                yield Ok(Event::default().event("trace").data(payload));
                            }
                        }
                    } else if notification.channel() == "ai_events" {
                        // Gelen events payload aslinda sadece video_id string'i
                        if payload == video_id {
                            // Postgres'ten guncel datayi cekip firlat
                            if let Ok(Some(row)) = sqlx::query("SELECT summary, events, risk, actions FROM ai_events WHERE video_id = $1")
                                .bind(&video_id)
                                .fetch_optional(&state.db_pool)
                                .await
                            {
                                let data = json!({
                                    "summary": row.try_get::<String, _>("summary").unwrap_or_default(),
                                    "events": row.try_get::<serde_json::Value, _>("events").unwrap_or_default(),
                                    "risk": row.try_get::<String, _>("risk").unwrap_or_default(),
                                    "actions": row.try_get::<serde_json::Value, _>("actions").unwrap_or_default(),
                                });
                                yield Ok(Event::default().event("report").data(data.to_string()));
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("PgListener hatasi: {}", e);
                    break;
                }
            }
        }
    };

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::new())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    motif_observer::init("gateway");

    // Yönlendirmeler **takip edilmiyor**. Bu bir vekil; Kratos'un 303'ünü
    // tarayıcıya olduğu gibi geçirmesi gerekiyor.
    //
    // Varsayılan davranış sonsuz döngü üretiyordu: Kratos giriş/kayıt akışını
    // `303 See Other` + `Location: .../auth/register?flow=<id>` ile başlatıyor.
    // reqwest bu yönlendirmeyi kendisi izleyip **panelin kendi sayfasını**
    // çekiyor ve tarayıcıya 200 olarak veriyordu. Tarayıcı API adresinde
    // kalıyor, kayıt sayfası adreste `?flow` göremiyor ve akışı yeniden
    // başlatıyordu — ekran sürekli yenileniyor, düğme "Bağlanıyor..." da
    // takılı kalıyordu.
    let kratos_client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .pool_idle_timeout(Duration::from_secs(90))
        .build()?;

    let session_cache = Cache::builder()
        .time_to_live(Duration::from_secs(5))
        .max_capacity(100_000)
        .build();

    let auth_state = AuthState {
        kratos_client,
        kratos_url: env_or("GATEWAY_KRATOS_URL", "http://127.0.0.1:4433"),
        session_cache,
    };

    let sonic_state = InferenceState {
        client: Client::builder()
            .timeout(Duration::from_secs(600))
            .build()?,
        base_url: env_or("GATEWAY_INFERENCE_URL", "http://127.0.0.1:8081"),
    };

    tracing::info!("Keto gRPC kanalına bağlanılıyor...");
    let keto_url: &'static str =
        Box::leak(env_or("GATEWAY_KETO_URL", "http://127.0.0.1:4466").into_boxed_str());
    let keto_channel = Channel::from_static(keto_url)
        .tcp_keepalive(Some(Duration::from_secs(15)))
        .http2_keep_alive_interval(Duration::from_secs(15))
        .connect()
        .await?;

    let keto_client = CheckServiceClient::new(keto_channel);
    let authz_state = AuthzState {
        client: keto_client,
    };

    let db_url = env_or("DATABASE_URL", "postgres://motif:motif@127.0.0.1:5433/motif");
    tracing::info!("Veritabanina (Postgres) baglaniliyor...");
    let db_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;
    tracing::info!("Veritabani baglantisi basarili.");

    let state = AppState {
        auth: auth_state,
        authz: authz_state,
        sonic: sonic_state,
        stream_url: env_or("GATEWAY_STREAM_URL", "http://127.0.0.1:8100"),
        toolbox_url: env_or("TOOLBOX_URL", "http://127.0.0.1:8116"),
        // Klip üretimi ffmpeg çalıştırıyor; uzun videoda dakikaları bulabiliyor.
        stream_client: Client::builder()
            .timeout(Duration::from_secs(1800))
            .build()?,
        db_pool,
    };

    let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();

    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers(tower_http::cors::Any)
        .allow_credentials(false);

    let app = Router::new()
        .route("/metrics", get(|| async move { metric_handle.render() }))
        .route("/api/auth/*path", any(kratos_proxy_handler))
        .route("/api/auth", any(kratos_proxy_handler))
        .route("/api/stream/*path", any(stream_proxy_handler))
        .route("/api/stream", any(stream_proxy_handler))
        .route("/api/tools/*path", any(proxy::toolbox_proxy_handler))
        .route("/api/tools", any(proxy::toolbox_proxy_handler))
        .route("/api/videos/:video_id/stream", get(stream_video))
        .route("/api/videos/:video_id/audio-events", get(audio::audio_events))
        .route("/api/videos/:video_id/events", get(get_video_events))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .layer(prometheus_layer)
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await?;
    tracing::info!("Gateway sunucusu {} adresinde dinleniyor...", listener.local_addr()?);
    
    let axum_server = axum::serve(listener, app);
    tokio::select! {
        res = axum_server => res?,
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Kapatılıyor...");
        }
    }
    
    motif_observer::shutdown();
    Ok(())
}
