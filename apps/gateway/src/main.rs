mod audio;
mod auth;
mod authz;
mod error;

use audio::InferenceState;
use auth::{AuthState, AuthenticatedUser};
use authz::{check_permission, AuthzState, keto::check_service_client::CheckServiceClient};
use axum::{
    extract::{Path, State},
    http::{header, HeaderValue, Method},
    routing::get,
    Router,
};
use error::GatewayError;
use moka::future::Cache;
use reqwest::Client;
use std::time::Duration;
use tonic::transport::Channel;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
pub struct AppState {
    pub auth: AuthState,
    pub authz: AuthzState,
    pub inference: InferenceState,
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Video akışı — **henüz uygulanmadı**.
///
/// Kimlik doğrulama ve yetki kontrolü çalışıyor ama akışın kendisi yok: medyayı
/// şu an dashboard kendi statik klasöründen (`public/media`) servis ediyor ve bu
/// yol gateway'e hiç uğramıyor. Uç nokta eskiden "stream ediliyor..." diyen bir
/// metin döndürüyordu — çalışıyormuş gibi görünen bir taslak, olmayandan kötü.
/// Gerçek akış (byte-range, medyanın `public/` dışına taşınması) gateway devreye
/// alınırken yapılacak.
async fn stream_video(
    State(state): State<AppState>,
    user: AuthenticatedUser, // Kimlik doğrulandı
    Path(video_id): Path<String>,
) -> Result<String, GatewayError> {
    // Yetki kontrolü (Bu kullanıcı bu videoyu izleyebilir mi?)
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
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "gateway=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 1. Kratos için HTTP Client ve Moka Cache
    let kratos_client = Client::builder()
        .pool_idle_timeout(Duration::from_secs(90))
        .build()?;

    // 5 saniye TTL ile in-memory cache (Milyonlarca isteği Gateway seviyesinde eritmek için)
    let session_cache = Cache::builder()
        .time_to_live(Duration::from_secs(5))
        .max_capacity(100_000)
        .build();

    let auth_state = AuthState {
        kratos_client,
        kratos_url: env_or("GATEWAY_KRATOS_URL", "http://127.0.0.1:4433"),
        session_cache,
    };

    // Ses çözümleme servisi (apps/ai/inference); yalnız 127.0.0.1 dinler.
    let inference_state = InferenceState {
        client: Client::builder()
            // Uzun medyada çözümleme dakikalar sürebilir.
            .timeout(Duration::from_secs(600))
            .build()?,
        base_url: env_or("GATEWAY_INFERENCE_URL", "http://127.0.0.1:8081"),
    };

    // 2. Keto için gRPC Channel
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
        inference: inference_state,
    };

    // Dashboard tarayıcıdan çağırdığı için oturum çerezinin gitmesi gerekiyor;
    // bu yüzden joker origin değil, açık liste kullanılıyor.
    let cors = CorsLayer::new()
        .allow_origin([
            HeaderValue::from_static("http://localhost:3000"),
            HeaderValue::from_static("http://127.0.0.1:3000"),
        ])
        .allow_methods([Method::GET])
        .allow_headers([header::CONTENT_TYPE])
        .allow_credentials(true);

    let app = Router::new()
        .route("/api/videos/:video_id/stream", get(stream_video))
        .route("/api/videos/:video_id/audio-events", get(audio::audio_events))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await?;
    tracing::info!("Gateway sunucusu {} adresinde dinleniyor...", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}
