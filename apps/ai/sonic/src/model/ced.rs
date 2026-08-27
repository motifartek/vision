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

/// Oturumu her profilin pencere boyutu için önceden ısıtır.
///
/// GPU sağlayıcıları (özellikle DirectML) çekirdekleri **giriş şekli başına** ilk
/// kullanımda derliyor. Ölçüldü: ilk istek 4.8 s, sonrakiler 1.0 s. Isıtmazsak
/// demoda görülen ilk analiz en yavaş olan olur. Bu maliyeti açılışta,
/// kullanıcıyı bekletmeden ödüyoruz.
pub fn warmup(session: &mut Session, batch: usize, window_frames: &[usize]) {
    for &frames in window_frames {
        let feats = vec![-100.0f32; batch * N_MELS * frames];
        let started = std::time::Instant::now();
        match run_batch(session, &feats, batch, frames) {
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
    session: &mut Session,
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
    pub session: Session,
    pub model_name: String,
    pub weights_file: String,
    /// İstenen sağlayıcı zinciri; ONNX Runtime bulamadığını sessizce atlayıp
    /// CPU'ya düşer.
    pub providers: Vec<&'static str>,
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
    let path = weights_path(cfg)?;

    let mut providers: Vec<&'static str> = Vec::new();
    #[cfg(feature = "tensorrt")]
    providers.push("TensorRT");
    #[cfg(feature = "cuda")]
    providers.push("CUDA");
    #[cfg(feature = "directml")]
    providers.push("DirectML");
    providers.push("CPU");

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
        session,
        model_name: cfg.model.clone(),
        weights_file,
        providers,
    })
}
