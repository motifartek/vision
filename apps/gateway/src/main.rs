mod auth;
mod authz;
mod error;
mod proxy;

use auth::{AuthState, AuthenticatedUser};
use authz::{check_permission, AuthzState, keto::check_service_client::CheckServiceClient};
use axum::{
    extract::{Path, State},
    routing::{get, any},
    Router,
};
use error::GatewayError;
use moka::future::Cache;
use proxy::kratos_proxy_handler;
use reqwest::Client;
use std::time::Duration;

use tower_http::cors::CorsLayer;
use http::{HeaderValue, Method};
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

    let session_cache = Cache::builder()
        .time_to_live(Duration::from_secs(5))
        .max_capacity(100_000)
        .build();

    // Ortam değişkenlerinden URL'leri al (Docker için gerekli)
    let kratos_url = std::env::var("KRATOS_URL").unwrap_or_else(|_| "http://127.0.0.1:4433".to_string());
    let keto_url = std::env::var("KETO_URL").unwrap_or_else(|_| "http://127.0.0.1:4466".to_string());

    let auth_state = AuthState {
        kratos_client,
        kratos_url,
        session_cache,
    };

    // 2. Keto için gRPC Channel
    tracing::info!("Keto gRPC kanalına bağlanılıyor: {}", keto_url);
    // URL'in statik olmaktan çıkıp dinamik channel olması için Endpoint kullanıyoruz
    let keto_endpoint = tonic::transport::Endpoint::from_shared(keto_url)?
        .tcp_keepalive(Some(Duration::from_secs(15)))
        .http2_keep_alive_interval(Duration::from_secs(15));
        
    let keto_channel = keto_endpoint.connect().await?;

    let keto_client = CheckServiceClient::new(keto_channel);
    let authz_state = AuthzState {
        client: keto_client,
    };

    let state = AppState {
        auth: auth_state,
        authz: authz_state,
    };

    // CORS Configuration (Allow frontend requests)
    let cors = CorsLayer::new()
        .allow_origin([
            "http://localhost:3000".parse::<HeaderValue>().unwrap(),
            "http://127.0.0.1:3000".parse::<HeaderValue>().unwrap(),
        ])
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers([
            http::header::AUTHORIZATION,
            http::header::CONTENT_TYPE,
            http::header::COOKIE,
            http::header::ACCEPT,
        ])
        .allow_credentials(true);

    let app = Router::new()
        // Auth Proxy
        .route("/api/auth/*path", any(kratos_proxy_handler))
        .route("/api/auth", any(kratos_proxy_handler))
        // Protected Microservice Endpoints
        .route("/api/videos/:video_id/stream", get(stream_video))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await?;
    tracing::info!("Gateway sunucusu {} adresinde dinleniyor...", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}
