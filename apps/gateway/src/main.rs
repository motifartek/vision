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
    http::{header, HeaderValue, Method},
    routing::{get, any},
    Router,
};
use axum_prometheus::PrometheusMetricLayer;
use error::GatewayError;
use moka::future::Cache;
use proxy::{kratos_proxy_handler, stream_proxy_handler};
use reqwest::Client;
use std::time::Duration;
use tonic::transport::Channel;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
pub struct AppState {
    pub auth: AuthState,
    pub authz: AuthzState,
    pub sonic: InferenceState,
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    motif_observer::init("gateway");

    let kratos_client = Client::builder()
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

    let state = AppState {
        auth: auth_state,
        authz: authz_state,
        sonic: sonic_state,
    };

    let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();

    let cors = CorsLayer::new()
        .allow_origin([
            HeaderValue::from_static("http://localhost:3000"),
            HeaderValue::from_static("http://127.0.0.1:3000"),
        ])
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::COOKIE,
            header::ACCEPT,
        ])
        .allow_credentials(true);

    let app = Router::new()
        .route("/metrics", get(|| async move { metric_handle.render() }))
        .route("/api/auth/*path", any(kratos_proxy_handler))
        .route("/api/auth", any(kratos_proxy_handler))
        .route("/api/stream/*path", any(stream_proxy_handler))
        .route("/api/stream", any(stream_proxy_handler))
        .route("/api/videos/:video_id/stream", get(stream_video))
        .route("/api/videos/:video_id/audio-events", get(audio::audio_events))
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
