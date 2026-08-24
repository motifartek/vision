use axum::{
    body::Body,
    extract::{Request, State},
    response::Response,
};

use crate::{AppState, error::GatewayError};

pub async fn kratos_proxy_handler(
    State(state): State<AppState>,
    req: Request,
) -> Result<Response, GatewayError> {
    let path = req.uri().path();
    let path_query = req
        .uri()
        .path_and_query()
        .map(|v| v.as_str())
        .unwrap_or(path);

    // Kratos Public Port (4433)
    let kratos_url = &state.auth.kratos_url;
    
    // /api/auth/ ile başlayan kısmı Kratos'a yönlendir
    // Örn: /api/auth/sessions/whoami -> http://127.0.0.1:4433/sessions/whoami
    let target_uri = format!("{}{}", kratos_url, path_query.replace("/api/auth", ""));

    let client = &state.auth.kratos_client;
    let method = req.method().clone();
    
    tracing::info!("Kratos Proxy: İletiliyor -> {} {}", method, target_uri);

    let mut headers = req.headers().clone();
    
    // Host header'ını kaldır (reqwest kendi oluşturacak)
    headers.remove(http::header::HOST);

    // Body'i al (Bytes olarak, şimdilik basitlik açısından)
    let body_bytes = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .map_err(|_| GatewayError::InternalError)?;

    let reqwest_req = client
        .request(method, target_uri)
        .headers(headers)
        .body(body_bytes);

    let res = reqwest_req.send().await.map_err(|e| {
        tracing::error!("Proxy error: {}", e);
        GatewayError::InternalError
    })?;

    let mut response_builder = Response::builder().status(res.status());
    
    // Headerları kopyala
    if let Some(headers_mut) = response_builder.headers_mut() {
        for (k, v) in res.headers().iter() {
            headers_mut.insert(k.clone(), v.clone());
        }
    }

    let res_body_bytes = res.bytes().await.map_err(|_| GatewayError::InternalError)?;
    
    let response = response_builder
        .body(Body::from(res_body_bytes))
        .map_err(|_| GatewayError::InternalError)?;

    Ok(response)
}
