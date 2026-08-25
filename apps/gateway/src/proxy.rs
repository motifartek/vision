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

    let kratos_url = &state.auth.kratos_url;
    let target_uri = format!("{}{}", kratos_url, path_query.replace("/api/auth", ""));

    let client = &state.auth.kratos_client;
    let method = req.method().clone();
    
    let mut headers = req.headers().clone();
    headers.remove(http::header::HOST);

    let body_bytes = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .map_err(|_| GatewayError::InternalError)?;

    let reqwest_req = client
        .request(method, target_uri)
        .headers(headers)
        .body(body_bytes);

    let res = reqwest_req.send().await.map_err(|_| GatewayError::InternalError)?;
    let mut response_builder = Response::builder().status(res.status());
    
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

pub async fn stream_proxy_handler(
    State(state): State<AppState>,
    req: Request,
) -> Result<Response, GatewayError> {
    let path = req.uri().path();
    let path_query = req
        .uri()
        .path_and_query()
        .map(|v| v.as_str())
        .unwrap_or(path);

    let stream_url = std::env::var("STREAM_URL").unwrap_or_else(|_| "http://stream:8100".to_string());
    let target_uri = format!("{}{}", stream_url, path_query.replace("/api/stream", ""));

    let client = reqwest::Client::new();
    let method = req.method().clone();
    
    tracing::info!("Stream Proxy: İletiliyor -> {} {}", method, target_uri);

    let mut headers = req.headers().clone();
    headers.remove(http::header::HOST);

    // Videolar büyük olduğu için body'yi belleğe (to_bytes) almadan akışkan (streaming) proxy yapıyoruz.
    let body_stream = req.into_body().into_data_stream();
    let reqwest_body = reqwest::Body::wrap_stream(body_stream);

    let reqwest_req = client
        .request(method, target_uri)
        .headers(headers)
        .body(reqwest_body);

    let res = reqwest_req.send().await.map_err(|e| {
        tracing::error!("Stream proxy error: {}", e);
        GatewayError::InternalError
    })?;

    let mut response_builder = Response::builder().status(res.status());
    
    if let Some(headers_mut) = response_builder.headers_mut() {
        for (k, v) in res.headers().iter() {
            headers_mut.insert(k.clone(), v.clone());
        }
    }

    // Dönüşü de stream olarak döndür
    let res_stream = res.bytes_stream();
    let body = Body::from_stream(res_stream);
    
    let response = response_builder
        .body(body)
        .map_err(|_| GatewayError::InternalError)?;

    Ok(response)
}