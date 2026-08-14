mod auth;
mod authz;
mod error;

use auth::{AuthState, AuthenticatedUser};
use authz::{check_permission, AuthzState, keto::check_service_client::CheckServiceClient};
use axum::{
    extract::{Path, State},
    routing::get,
    Router,
};
use error::GatewayError;
use moka::future::Cache;
use reqwest::Client;
use std::time::Duration;
use tonic::transport::Channel;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
pub struct AppState {
    pub auth: AuthState,
    pub authz: AuthzState,
}

// Örnek korumalı endpoint
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

    Ok(format!(
        "{} kimlikli kullanıcı için {} videosu stream ediliyor...",
        user.identity_id, video_id
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
        kratos_url: "http://127.0.0.1:4433".to_string(), // Docker-compose public port
        session_cache,
    };

    // 2. Keto için gRPC Channel
    tracing::info!("Keto gRPC kanalına bağlanılıyor...");
    let keto_channel = Channel::from_static("http://127.0.0.1:4466")
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
    };

    let app = Router::new()
        .route("/api/videos/:video_id/stream", get(stream_video))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await?;
    tracing::info!("Gateway sunucusu {} adresinde dinleniyor...", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}
