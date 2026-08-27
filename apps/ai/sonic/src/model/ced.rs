use std::path::PathBuf;

use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::TensorRef;

use crate::audio::mel::N_MELS;
use crate::config::Config;
use crate::error::InferenceError;

pub const NUM_CLASSES: usize = 527;

/// CED'in eğitim penceresi: 1012 log-mel karesi (~10.12 s).
///
/// Bu sınır aşıldığında modelin konumsal gömmeleri uyuşmuyor ve ONNX Runtime
/// yayılım (broadcast) hatasıyla düşüyor — 40 s'lik bir dosyayı tek pencere
/// olarak vermek bunu tetikliyor. Aşılmasa bile eğitim uzunluğunun ötesinde
/// isabet düşer, dolayısıyla bu hem teknik hem doğruluk sınırı.
pub const MAX_WINDOW_FRAMES: usize = 1012;

/// Çıkarımın nerede koştuğu.
///
/// `Local` bugünkü davranış: ONNX oturumu bu süreçte. `Remote` ise çağrıyı
/// host'taki `model-host` ikilisine taşıyor — DirectML yalnız Windows'ta
/// çalıştığı ve sonic Linux konteynerinde koştuğu için, kartı kullanmanın tek
/// yolu bu. Konteynerden çıkan tek şey tensör→tensör çağrısı; çözme, log-mel,
/// olay bölütleme ve güvenlik kuralları yerinde kalıyor.
pub enum Backend {
    Local(Session),
    Remote { url: String },
}

/// `feats` ve `prob` ham little-endian f32 olarak taşınıyor, JSON olarak değil.
/// 12 dakikalık bir videoda ~73 MB f32 gidiyor; JSON'a çevirmek bunu yüz
/// megabaytlarca metne şişirir ve taşımanın maliyeti kazancı yerdi.
fn run_remote(
    url: &str,
    feats: &[f32],
    batch: usize,
    frames: usize,
) -> Result<Vec<f32>, InferenceError> {
    let mut body = Vec::with_capacity(feats.len() * 4);
    for v in feats {
        body.extend_from_slice(&v.to_le_bytes());
    }

    let response = ureq::post(&format!("{url}/v1/infer"))
        .set("Content-Type", "application/octet-stream")
        .set("X-Batch", &batch.to_string())
        .set("X-Frames", &frames.to_string())
        .send_bytes(&body)
        .map_err(|e| InferenceError::Model(format!("model host'a ulaşılamadı ({url}): {e}")))?;

    let mut raw = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut raw)
        .map_err(|e| InferenceError::Model(format!("model host yanıtı okunamadı: {e}")))?;

    if raw.len() % 4 != 0 {
        return Err(InferenceError::Model(format!(
            "model host {} bayt döndü; 4'ün katı olmalı",
            raw.len()
        )));
    }

    let probs: Vec<f32> = raw
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();

    let beklenen = batch * NUM_CLASSES;
    if probs.len() != beklenen {
        return Err(InferenceError::Model(format!(
            "model host {} skor döndü, {beklenen} bekleniyordu",
            probs.len()
        )));
    }

    Ok(probs)
}

/// Oturumu her profilin pencere boyutu için önceden ısıtır.
///
/// GPU sağlayıcıları (özellikle DirectML) çekirdekleri **giriş şekli başına** ilk
/// kullanımda derliyor. Ölçüldü: ilk istek 4.8 s, sonrakiler 1.0 s. Isıtmazsak
/// demoda görülen ilk analiz en yavaş olan olur. Bu maliyeti açılışta,
/// kullanıcıyı bekletmeden ödüyoruz.
pub fn warmup(backend: &mut Backend, batch: usize, window_frames: &[usize]) {
    for &frames in window_frames {
        let feats = vec![-100.0f32; batch * N_MELS * frames];
        let started = std::time::Instant::now();
        match run_batch(backend, &feats, batch, frames) {
            Ok(_) => tracing::debug!(
                frames,
                batch,
                ms = started.elapsed().as_millis(),
                "ısıtma tamamlandı"
            ),
            // Isıtma başarısız olsa da servis çalışmalı; ilk istek yavaş olur.
            Err(err) => tracing::warn!(frames, "ısıtma başarısız: {}", err),
        }
    }
}

/// `feats [batch, 64, frames]` → `prob [batch, 527]`.
///
/// `feats` mel-öncelikli düz dizi (`LogMel::push_window` bu düzende üretir);
/// uzunluğu `batch * N_MELS * frames` olmalı. Dönen dizi `batch * NUM_CLASSES`
/// uzunluğunda sigmoid olasılıklardır.
pub fn run_batch(
    backend: &mut Backend,
    feats: &[f32],
    batch: usize,
    frames: usize,
) -> Result<Vec<f32>, InferenceError> {
    debug_assert_eq!(feats.len(), batch * frames * N_MELS);

    if frames > MAX_WINDOW_FRAMES {
        return Err(InferenceError::Config(format!(
            "pencere {frames} kare; model en fazla {MAX_WINDOW_FRAMES} kare (~{:.2} s) kabul ediyor",
            MAX_WINDOW_FRAMES as f32 / 100.0
        )));
    }

    // Sınır denetimi ortak: uzak yol da aynı sözleşmeye tabi, host'a geçersiz
    // şekil göndermenin anlamı yok.
    let session = match backend {
        Backend::Remote { url } => return run_remote(url, feats, batch, frames),
        Backend::Local(session) => session,
    };

    let shape = vec![batch as i64, N_MELS as i64, frames as i64];
    let tensor = TensorRef::from_array_view((shape, feats))
        .map_err(|e| InferenceError::Model(e.to_string()))?;

    let outputs = session
        .run(ort::inputs!["feats" => tensor])
        .map_err(|e| InferenceError::Model(e.to_string()))?;

    let (shape, data) = outputs["prob"]
        .try_extract_tensor::<f32>()
        .map_err(|e| InferenceError::Model(e.to_string()))?;

    if shape.len() != 2 || shape[0] as usize != batch || shape[1] as usize != NUM_CLASSES {
        return Err(InferenceError::Model(format!(
            "beklenmeyen prob şekli {shape:?}, [{batch}, {NUM_CLASSES}] bekleniyordu"
        )));
    }

    Ok(data.to_vec())
}

pub struct LoadedModel {
    pub backend: Backend,
    pub model_name: String,
    pub weights_file: String,
    /// İstenen sağlayıcı zinciri; ONNX Runtime bulamadığını sessizce atlayıp
    /// CPU'ya düşer. **Bu alan neyin istendiğini söyler, neyin etkin olduğunu
    /// değil** — hız doğrulaması ölçümle yapılmalı, buraya bakarak değil.
    pub providers: Vec<String>,
}

/// Uzak model host'una bağlanır: ağırlık yüklenmez, model bilgisi host'tan
/// sorulur.
///
/// Ulaşılamazsa **hata döner, sessizce yerel çıkarıma düşmez.** Sessiz geri
/// düşme bu serviste daha önce pahalıya mal oldu: ölü log filtresi yüzünden
/// konteyner hiç konuşmuyordu ve `providers` alanı hâlâ istenen zinciri
/// gösterdiği için "GPU'da koşuyor" sanılabiliyordu. Hızlandırma açıkça
/// istendiyse, çalışmadığında gürültü çıkarmalı.
fn load_remote(url: &str) -> Result<LoadedModel, InferenceError> {
    let response = ureq::get(&format!("{url}/healthz"))
        .call()
        .map_err(|e| InferenceError::Config(format!(
            "SONIC_MODEL_HOST={url} ayarlı ama host'a ulaşılamıyor: {e}. \
             Host tarafında `model-host` çalışıyor mu?"
        )))?;

    let info: serde_json::Value = response.into_json().map_err(|e| {
        InferenceError::Config(format!("model host yanıtı çözümlenemedi: {e}"))
    })?;

    let model_name = info["model"]["name"].as_str().unwrap_or("bilinmiyor").to_string();
    let weights_file = info["model"]["weights"].as_str().unwrap_or("bilinmiyor").to_string();
    let providers: Vec<String> = info["model"]["providers"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    tracing::info!(
        host = %url,
        model = %model_name,
        dosya = %weights_file,
        saglayicilar = ?providers,
        "çıkarım host'taki model sunucusuna taşındı"
    );

    Ok(LoadedModel {
        backend: Backend::Remote { url: url.to_string() },
        model_name,
        weights_file,
        providers,
    })
}

fn weights_path(cfg: &Config) -> Result<PathBuf, InferenceError> {
    let dir = cfg.models_dir.join(&cfg.model);
    let fp32 = dir.join("model.onnx");
    let int8 = dir.join("model.int8.onnx");

    if cfg.prefer_int8 && int8.is_file() {
        Ok(int8)
    } else if fp32.is_file() {
        Ok(fp32)
    } else if int8.is_file() {
        Ok(int8)
    } else {
        Err(InferenceError::Config(format!(
            "{} altında model.onnx / model.int8.onnx yok; önce scripts/fetch-models çalıştırın",
            dir.display()
        )))
    }
}

pub fn load(cfg: &Config) -> Result<LoadedModel, InferenceError> {
    // Çıkarım host'a taşındıysa bu süreçte ağırlığa gerek yok: konteynerde
    // model dosyası aramak da, yüklemek de gereksiz.
    if let Some(url) = &cfg.model_host {
        return load_remote(url);
    }

    let path = weights_path(cfg)?;

    let mut providers: Vec<String> = Vec::new();
    #[cfg(feature = "tensorrt")]
    providers.push("TensorRT".into());
    #[cfg(feature = "cuda")]
    providers.push("CUDA".into());
    #[cfg(feature = "directml")]
    providers.push("DirectML".into());
    providers.push("CPU".into());

    // Builder metotları `self`'i tüketip kurtarılabilir hata (`Error<SessionBuilder>`)
    // döndürüyor; bu yüzden zincir yerine adım adım yeniden bağlanıyor.
    let mut builder = Session::builder().map_err(|e| InferenceError::Model(e.to_string()))?;
    builder = builder
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|e| InferenceError::Model(e.to_string()))?;
    builder = builder
        .with_intra_threads(cfg.intra_threads)
        .map_err(|e| InferenceError::Model(e.to_string()))?;

    #[cfg(any(feature = "cuda", feature = "tensorrt", feature = "directml"))]
    {
        let mut eps: Vec<ort::ep::ExecutionProviderDispatch> = Vec::new();
        #[cfg(feature = "tensorrt")]
        eps.push(ort::ep::TensorRT::default().build());
        #[cfg(feature = "cuda")]
        eps.push(ort::ep::CUDA::default().build());
        #[cfg(feature = "directml")]
        {
            // DirectML varsayılan olarak **0 numaralı** adaptörü seçiyor. Çift
            // GPU'lu laptoplarda bu çoğu zaman tümleşik Intel kartı oluyor ve
            // ayrık kart boşta beklerken kazanç hayal kırıklığı yaratıyor.
            // `SONIC_DML_DEVICE=1` ile doğru adaptöre geçilebilsin.
            let dml = match std::env::var("SONIC_DML_DEVICE")
                .ok()
                .and_then(|v| v.parse::<i32>().ok())
            {
                Some(id) => {
                    tracing::info!(cihaz = id, "DirectML adaptörü elle seçildi");
                    ort::ep::DirectML::default().with_device_id(id)
                }
                None => {
                    tracing::info!("DirectML varsayılan adaptör (0); değiştirmek için SONIC_DML_DEVICE");
                    ort::ep::DirectML::default()
                }
            };
            eps.push(dml.build());
        }
        builder = builder
            .with_execution_providers(eps)
            .map_err(|e| InferenceError::Model(e.to_string()))?;
    }

    let session = builder
        .commit_from_file(&path)
        .map_err(|e| InferenceError::Model(format!("{}: {e}", path.display())))?;

    // CED sözleşmesi: girdi feats [batch, time, 64], çıktı prob [batch, 527].
    let inputs: Vec<String> = session.inputs().iter().map(|o| o.name().to_string()).collect();
    let outputs: Vec<String> = session.outputs().iter().map(|o| o.name().to_string()).collect();
    if inputs.first().map(String::as_str) != Some("feats")
        || outputs.first().map(String::as_str) != Some("prob")
    {
        return Err(InferenceError::Model(format!(
            "beklenmeyen model imzası: girişler {inputs:?}, çıkışlar {outputs:?} (feats/prob bekleniyordu) — {}",
            path.display()
        )));
    }

    let weights_file = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    tracing::info!(
        model = %cfg.model,
        dosya = %weights_file,
        saglayicilar = ?providers,
        girisler = ?inputs,
        cikislar = ?outputs,
        "CED modeli yüklendi"
    );

    Ok(LoadedModel {
        backend: Backend::Local(session),
        model_name: cfg.model.clone(),
        weights_file,
        providers,
    })
}
