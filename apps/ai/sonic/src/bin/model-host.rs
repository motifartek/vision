//! Host tarafında koşan çıkarım sunucusu.
//!
//! # Neden var
//!
//! DirectML bir Windows DirectX 12 API'si; ONNX Runtime'ın DirectML sağlayıcısı
//! yalnız Windows için dağıtılıyor. Sonic ise Linux konteynerinde koşuyor, yani
//! konteynerin içinden karta erişmenin yolu yok. Docker'da CUDA denendi ve
//! **CPU'dan yavaş** çıktı (ölçüm: `apps/ai/sonic/compose.gpu.yaml`), çünkü int8
//! nicemleme CPU'ya özgü ve GPU fp32'ye çıkmak zorunda.
//!
//! Bu ikili tek bir işi yapıyor: modeli host'ta, kartın üstünde tutmak ve
//! tensör→tensör çağrısını karşılamak. Ölçüme göre sonic'in toplam süresinin
//! %94'ü (13,7 sn'nin 12,9'u) bu tek çağrıda geçiyor; çözme, log-mel, olay
//! bölütleme ve güvenlik kuralları konteynerde kalabiliyor.
//!
//! # Çalıştırma
//!
//! ```text
//! cargo build -p sonic --release --features directml --bin model-host
//! SONIC_DML_DEVICE=1 SONIC_MODEL_HOST_BIND=0.0.0.0:8082 model-host
//! ```
//!
//! `SONIC_DML_DEVICE` **önemli**: DirectML varsayılan olarak 0 numaralı
//! adaptörü seçiyor ve çift GPU'lu laptoplarda bu genelde tümleşik karttır.
//! Ayrık kart boşta beklerken tümleşikte koşmak CPU'dan bile yavaş olabilir.
//!
//! Konteynerden `host.docker.internal` üzerinden gelindiği için loopback'e
//! bağlanmak sunucuyu görünmez yapar; varsayılan bu yüzden `0.0.0.0`.
//! Kimlik doğrulaması yok — 8082'yi yerel ağa açmayın, güvenlik duvarı
//! kuralını WSL/Docker alt ağıyla sınırlayın.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use sonic::config::Config;
use sonic::model::ced::{self, Backend};

struct HostState {
    backend: Mutex<Backend>,
    model_name: String,
    weights_file: String,
    providers: Vec<String>,
    classes: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sonic=debug,model_host=debug,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Aynı `Config`: model dizini, model adı, int8 tercihi, batch — hepsi
    // sonic'le aynı değişkenlerden okunuyor, ikinci bir yapılandırma yüzeyi yok.
    let mut cfg = Config::from_env();
    // Kendi kendine bağlanmasın: bu süreç modelin ta kendisini tutuyor.
    cfg.model_host = None;

    let labels_path = cfg.models_dir.join(&cfg.model).join("class_labels_indices.csv");
    let labels = sonic::model::labels::load(&labels_path)?;

    let mut loaded = ced::load(&cfg)?;

    // Isıtma burada asıl kazanç: DirectML çekirdekleri **giriş şekli başına**
    // ilk kullanımda derleniyor. Isıtmazsak ilk isteğin bedelini sonic ödüyor
    // ve hızlandırma tam da ilk analizde görünmüyor.
    let window_frames: Vec<usize> = sonic::config::PROFILES
        .iter()
        .map(|p| (p.window_sec * 100.0).round() as usize)
        .collect();
    let started = std::time::Instant::now();
    ced::warmup(&mut loaded.backend, cfg.batch_size, &window_frames);
    tracing::info!(
        ms = started.elapsed().as_millis(),
        sekil = window_frames.len(),
        "model ısıtıldı"
    );

    let state = Arc::new(HostState {
        backend: Mutex::new(loaded.backend),
        model_name: loaded.model_name,
        weights_file: loaded.weights_file,
        providers: loaded.providers,
        classes: labels.len(),
    });

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/infer", post(infer))
        // Gövde sınırı kalkmalı: axum varsayılanı 2 MB, oysa tek bir batch bunu
        // rahatlıkla aşıyor. `isabetli` profilinde pencere 1000 kare ve
        // batch 32 iken gövde 32 × 64 × 1000 × 4 = 8,2 MB. Sınır açıkken
        // küçük pencereler geçiyor, büyükler "broken pipe" ile düşüyordu —
        // yani arıza profile göre ortaya çıkan sinsi bir arızaydı.
        .layer(axum::extract::DefaultBodyLimit::disable())
        .with_state(state);

    let bind: SocketAddr = std::env::var("SONIC_MODEL_HOST_BIND")
        .unwrap_or_else(|_| "0.0.0.0:8082".into())
        .parse()
        .map_err(|e| format!("SONIC_MODEL_HOST_BIND çözümlenemedi: {e}"))?;

    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!("model host {} adresinde dinliyor", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}

/// Sonic açılışta buraya bakıp model adını, ağırlık dosyasını ve sağlayıcı
/// zincirini kendi `/healthz`'inde bildiriyor.
async fn healthz(State(state): State<Arc<HostState>>) -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "model": {
            "name": state.model_name,
            "weights": state.weights_file,
            "providers": state.providers,
            "classes": state.classes,
        },
    }))
}

/// `feats [batch, 64, frames]` → `prob [batch, 527]`.
///
/// Gövde ham little-endian f32; JSON değil. 12 dakikalık bir videoda ~73 MB
/// f32 gidiyor ve JSON'a çevirmek bunu yüz megabaytlarca metne şişirirdi —
/// taşımanın maliyeti kazancı yerdi.
async fn infer(
    State(state): State<Arc<HostState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let header_usize = |name: &str| -> Result<usize, (StatusCode, String)> {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<usize>().ok())
            .ok_or_else(|| (StatusCode::BAD_REQUEST, format!("{name} başlığı eksik ya da geçersiz")))
    };

    let batch = header_usize("X-Batch")?;
    let frames = header_usize("X-Frames")?;

    if body.len() % 4 != 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("gövde {} bayt; 4'ün katı olmalı", body.len()),
        ));
    }

    let feats: Vec<f32> = body
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();

    // Şekil denetimi burada da yapılıyor: bozuk bir istek `run_batch` içindeki
    // `debug_assert`'e kadar gitmemeli, release derlemesinde o denetim yok.
    let beklenen = batch * frames * 64;
    if feats.len() != beklenen {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("{} f32 geldi, batch({batch})×frames({frames})×64 = {beklenen} bekleniyordu", feats.len()),
        ));
    }

    // Çıkarım bloke edici; tokio çalışanını tutmasın.
    let probs = tokio::task::spawn_blocking(move || {
        let mut backend = state
            .backend
            .lock()
            .map_err(|_| "model oturumu önceki bir çökme sonrası kullanılamaz".to_string())?;
        ced::run_batch(&mut backend, &feats, batch, frames).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("çıkarım görevi tamamlanamadı: {e}")))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let mut out = Vec::with_capacity(probs.len() * 4);
    for v in &probs {
        out.extend_from_slice(&v.to_le_bytes());
    }
    Ok(out)
}
