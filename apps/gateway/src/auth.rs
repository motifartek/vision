use axum::{
    async_trait,
    extract::FromRequestParts,
    http::request::Parts,
};
use moka::future::Cache;
use reqwest::Client;
use serde::Deserialize;
use crate::error::GatewayError;

#[derive(Debug, Deserialize, Clone)]
pub struct KratosIdentity {
    pub id: String,
    pub schema_id: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct KratosSession {
    pub id: String,
    pub identity: KratosIdentity,
}

#[derive(Clone)]
pub struct AuthenticatedUser {
    pub identity_id: String,
}

#[derive(Clone)]
pub struct AuthState {
    pub kratos_client: Client,
    pub kratos_url: String,
    pub session_cache: Cache<String, AuthenticatedUser>,
}

#[async_trait]
impl FromRequestParts<crate::AppState> for AuthenticatedUser {
    type Rejection = GatewayError;

    async fn from_request_parts(parts: &mut Parts, state: &crate::AppState) -> Result<Self, Self::Rejection> {
        let auth_state = &state.auth;

        let cookie = parts
            .headers
            .get(http::header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();

        let auth_header = parts
            .headers
            .get(http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();

        if cookie.is_empty() && auth_header.is_empty() {
            return Err(GatewayError::Unauthorized);
        }

        // Cache key oluştur (cookie + header birleşimi)
        let cache_key = format!("c:{}_h:{}", cookie, auth_header);

        // Cache'te varsa hemen dön (Latency: ~1 mikrosaniye)
        if let Some(user) = auth_state.session_cache.get(&cache_key).await {
            return Ok(user);
        }

        // Cache'te yoksa Kratos'a sor (Latency: Ağ süresi)
        let res = auth_state
            .kratos_client
            .get(&format!("{}/sessions/whoami", auth_state.kratos_url))
            .header(http::header::COOKIE, cookie)
            .header(http::header::AUTHORIZATION, auth_header)
            .send()
            .await?;

        if res.status().is_success() {
            let session = res
                .json::<KratosSession>()
                .await
                .map_err(|_| GatewayError::InternalError)?;

            let user = AuthenticatedUser {
                identity_id: session.identity.id,
            };

            // Cache'e ekle
            auth_state.session_cache.insert(cache_key.clone(), user.clone()).await;

            Ok(user)
        } else {
            Err(GatewayError::Unauthorized)
        }
    }
}
