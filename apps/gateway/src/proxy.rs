use axum::{
    body::Body,
    extract::{Request, State},
    response::Response,
};
use http::HeaderMap;

use crate::{AppState, error::GatewayError};

/// Cevaptan çıkarılması gereken başlıklar.
///
/// İki grup var ve ikisi de gerçek hataya yol açtı:
///
/// - **`content-encoding` / `content-length`:** reqwest gövdeyi kendisi açıyor,
///   yani elimize gelen baytlar sıkıştırılmış değil. Kratos'un
///   `content-encoding: gzip` başlığını olduğu gibi iletmek, istemciye düz
///   metni "bu gzip" diye sunmak demek; tarayıcı ve curl bağlantıyı düşürüyor.
///   Belirti aldatıcıydı: ağ geçidi günlüğe `status=200` yazıyor ama istemci
///   "empty reply from server" alıyordu. `content-length` de açılmış gövdeyle
///   uyuşmuyor; ikisi de yeniden hesaplanmalı.
///
/// - **Atlama başlıkları:** `connection`, `transfer-encoding` ve arkadaşları tek
///   bir bağlantıya ait; vekil bunları aktarmaz (RFC 9110 §7.6.1). Sabit
///   gövdeyle birlikte `transfer-encoding: chunked` iletmek protokolü bozuyor.
const AKTARILMAZ: &[&str] = &[
    "content-encoding",
    "content-length",
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

/// Yukarı akış cevabının başlıklarını istemciye iletilecek hâle getirir.
///
/// `append` kullanılıyor, `insert` değil: Kratos oturum akışında **birden çok
/// `set-cookie`** gönderiyor (CSRF ve oturum çerezleri ayrı ayrı).
/// `insert` her seferinde öncekini silip yalnız sonuncusunu bıraktığı için
/// giriş akışı sessizce kırılıyordu.
pub fn iletilecek_basliklar(kaynak: &HeaderMap) -> HeaderMap {
    let mut hedef = HeaderMap::new();
    for (ad, deger) in kaynak.iter() {
        if AKTARILMAZ.contains(&ad.as_str()) {
            continue;
        }
        hedef.append(ad.clone(), deger.clone());
    }
    hedef
}

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
    // Sıkıştırma pazarlığı reqwest'e bırakılıyor: istemcinin listesini olduğu
    // gibi iletmek, açılmış gövdeyle uyuşmayan bir cevap üretme riskini
    // gereksiz yere artırıyor.
    headers.remove(http::header::ACCEPT_ENCODING);

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

    let durum = res.status();
    let basliklar = iletilecek_basliklar(res.headers());

    let res_body_bytes = res.bytes().await.map_err(|_| GatewayError::InternalError)?;

    let mut response_builder = Response::builder().status(durum);
    if let Some(headers_mut) = response_builder.headers_mut() {
        *headers_mut = basliklar;
    }

    let response = response_builder
        .body(Body::from(res_body_bytes))
        .map_err(|_| GatewayError::InternalError)?;

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;

    #[test]
    fn sikistirma_basliklari_aktarilmaz() {
        // reqwest gövdeyi açtığı için bu başlıklar artık gövdeyi anlatmıyor.
        // İletilirlerse istemci "empty reply from server" alıyor.
        let mut h = HeaderMap::new();
        h.insert("content-encoding", HeaderValue::from_static("gzip"));
        h.insert("content-length", HeaderValue::from_static("1234"));
        h.insert("content-type", HeaderValue::from_static("application/json"));

        let out = iletilecek_basliklar(&h);
        assert!(out.get("content-encoding").is_none());
        assert!(out.get("content-length").is_none());
        assert_eq!(out.get("content-type").unwrap(), "application/json");
    }

    #[test]
    fn atlama_basliklari_aktarilmaz() {
        let mut h = HeaderMap::new();
        h.insert("transfer-encoding", HeaderValue::from_static("chunked"));
        h.insert("connection", HeaderValue::from_static("keep-alive"));
        h.insert("location", HeaderValue::from_static("/auth/login"));

        let out = iletilecek_basliklar(&h);
        assert!(out.get("transfer-encoding").is_none());
        assert!(out.get("connection").is_none());
        assert_eq!(out.get("location").unwrap(), "/auth/login");
    }

    #[test]
    fn butun_set_cookie_basliklari_korunur() {
        // Kratos giriş akışında CSRF ve oturum çerezlerini ayrı başlıklarda
        // gönderiyor. `insert` kullanıldığında yalnız sonuncusu kalıyor ve
        // akış sessizce kırılıyordu.
        let mut h = HeaderMap::new();
        h.append("set-cookie", HeaderValue::from_static("csrf_token=abc; Path=/"));
        h.append("set-cookie", HeaderValue::from_static("ory_session=xyz; Path=/"));

        let out = iletilecek_basliklar(&h);
        let cerezler: Vec<_> = out.get_all("set-cookie").iter().collect();
        assert_eq!(cerezler.len(), 2, "iki çerez de iletilmeli");
    }
}
