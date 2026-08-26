//! Boru hattı işlemleri: alma, profil, genel bakış, yakınlaştırma.
//!
//! ffmpeg alt süreç çağırdığı için tüm ağır işler `spawn_blocking` içinde
//! koşar; aksi halde tek bir video çözme işi tokio çalışan iş parçacığını
//! bloke eder ve eşzamanlı istekler kuyruğa girerdi.

use std::sync::Arc;

use chrono::Utc;
use motif_core::{Error, Result, VideoId};
use motif_event_sdk::{FrameExtracted, FrameRef, SamplingPass, VideoIngested, SCHEMA_VERSION};
use motif_optics::{
    build_profile, extract_clip, extract_jpegs, probe, scale_for_frames, select_frames, Clip,
    ClipOptions, ExtractOptions, MotionProfile, SamplingConfig, Selection,
};
use uuid::Uuid;

use crate::catalog::VideoRecord;
use crate::state::AppState;

/// Yüklenen videoyu depoya yazar, metadata çıkarır ve kütüğe kaydeder.
pub async fn ingest(
    state: &Arc<AppState>,
    bytes: Vec<u8>,
    original_name: String,
) -> Result<VideoRecord> {
    let id = VideoId::new();
    let extension = std::path::Path::new(&original_name)
        .extension()
        .and_then(|e| e.to_str())
        .filter(|e| e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or("mp4")
        .to_ascii_lowercase();

    let object_key = id.raw_object_key(&extension);
    state.store.put(&object_key, &bytes)?;

    // Metadata çıkarımı ffmpeg'e gider; bloke edici.
    let video_path = state.store.local_path(&object_key)?;
    let info = tokio::task::spawn_blocking({
        let p = video_path.clone();
        move || probe(&p)
    })
    .await
    .map_err(|e| Error::Config(format!("probe görevi düştü: {e}")))?
    // ffprobe'un okuyamadığı dosya bozuk ya da video değil demektir; bu
    // istemcinin hatası, sunucunun değil. Yüklenen dosya diskte kalmasın.
    .map_err(|e| {
        let _ = state.store.delete(&object_key);
        Error::InvalidVideo(format!(
            "dosya video olarak okunamadı ({original_name}): {e}"
        ))
    })?;

    // --- H.264 normalizasyonu ---
    //
    // Çıkarım servisinin çözücüsü AV1'i açamıyor: ölçüldü, tek kare bile
    // çıkaramayıp HTTP 400 döndürdü. Aynı video H.264'e çevrilince çalıştı.
    // Final test videolarının kodlaması bilinmediği için bu adım zorunlu ve
    // alımda yapılıyor — sonraki her aşama güvenli codec'le çalışsın.
    let info = if info.codec != "h264" {
        tracing::info!(%id, codec = %info.codec, "codec servis tarafından açılamıyor, H.264'e çevriliyor");

        let gecici = std::env::temp_dir().join(format!("motif-norm-{}.mp4", Uuid::new_v4()));
        let (kaynak, hedef) = (video_path.clone(), gecici.clone());
        let opts = ClipOptions::default();

        tokio::task::spawn_blocking(move || motif_optics::normalize(&kaynak, &hedef, &opts))
            .await
            .map_err(|e| Error::Config(format!("normalizasyon görevi düştü: {e}")))??;

        let cevrilmis = std::fs::read(&gecici)?;
        state.store.put(&object_key, &cevrilmis)?;
        let _ = std::fs::remove_file(&gecici);

        let yeni_yol = state.store.local_path(&object_key)?;
        let yeni = tokio::task::spawn_blocking(move || probe(&yeni_yol))
            .await
            .map_err(|e| Error::Config(format!("probe görevi düştü: {e}")))??;

        tracing::info!(%id, "normalize edildi: {} -> h264", info.codec);
        yeni
    } else {
        info
    };

    // Durağan görüntüyü video sanmayı engelle.
    //
    // ffprobe bir JPEG'i tek karelik video gibi okuyor ve boru hattı sessizce
    // anlamsız bir profil üretiyor. Yanlış dosya yüklendiğinde hata yerine
    // boş sonuç dönmek, test sırasında en kafa karıştırıcı davranış.
    const MIN_VIDEO_MS: u64 = 200;
    if info.duration_ms < MIN_VIDEO_MS {
        let _ = state.store.delete(&object_key);
        return Err(Error::InvalidVideo(format!(
            "süre {} ms — bu bir video değil, durağan görüntü olabilir",
            info.duration_ms
        )));
    }

    let record = VideoRecord {
        id: id.clone(),
        original_name,
        object_key: object_key.clone(),
        info: info.clone(),
        uploaded_at: Utc::now(),
    };
    record.save(state.store.as_ref())?;

    state
        .events
        .video_ingested(&VideoIngested {
            schema_version: SCHEMA_VERSION,
            video_id: id,
            object_key,
            duration_ms: info.duration_ms,
            fps: info.fps,
            width: info.width,
            height: info.height,
            size_bytes: info.size_bytes,
            codec: info.codec,
            ingested_at: record.uploaded_at,
        })
        .await;

    Ok(record)
}

/// Videonun hareket profilini döndürür.
///
/// Üç kademeli: bellek önbelleği → depodaki JSON → hesapla. Profil video başına
/// bir kez çıkarılır; yakınlaştırmanın ucuz olmasının sebebi bu.
pub async fn profile(state: &Arc<AppState>, id: &VideoId) -> Result<Arc<MotionProfile>> {
    if let Some(cached) = state.cached_profile(id).await {
        return Ok(cached);
    }

    let profile_key = id.profile_object_key();
    if let Ok(bytes) = state.store.get(&profile_key) {
        if let Ok(profile) = serde_json::from_slice::<MotionProfile>(&bytes) {
            let profile = Arc::new(profile);
            state.cache_profile(id.clone(), profile.clone()).await;
            return Ok(profile);
        }
        tracing::warn!(%id, "saklanmış profil okunamadı, yeniden hesaplanıyor");
    }

    let record = VideoRecord::load(state.store.as_ref(), id)?;
    let video_path = state.store.local_path(&record.object_key)?;
    let analysis = state.config.analysis;

    let started = std::time::Instant::now();
    let computed = tokio::task::spawn_blocking(move || build_profile(&video_path, analysis))
        .await
        .map_err(|e| Error::Config(format!("profil görevi düştü: {e}")))??;

    tracing::info!(
        %id,
        samples = computed.len(),
        scene_cuts = computed.scene_cuts().count(),
        elapsed_ms = started.elapsed().as_millis() as u64,
        "hareket profili çıkarıldı"
    );

    if let Ok(bytes) = serde_json::to_vec(&computed) {
        let _ = state.store.put(&profile_key, &bytes);
    }

    let profile = Arc::new(computed);
    state.cache_profile(id.clone(), profile.clone()).await;
    Ok(profile)
}

/// Seçilen kareleri tam kalitede çıkarır ve depoya yazar.
async fn materialize(
    state: &Arc<AppState>,
    id: &VideoId,
    selection: &Selection,
    pass: SamplingPass,
) -> Result<Vec<FrameRef>> {
    if selection.frames.is_empty() {
        return Ok(Vec::new());
    }

    let record = VideoRecord::load(state.store.as_ref(), id)?;
    let video_path = state.store.local_path(&record.object_key)?;
    let timestamps = selection.timestamps();

    // Kareler önce geçici bir dizine çıkarılır, sonra depoya taşınır. Depo
    // soyutlamasının arkasında S3 olabileceği için ffmpeg doğrudan oraya
    // yazamaz.
    let scratch = std::env::temp_dir().join(format!("motif-frames-{}", Uuid::new_v4()));
    let opts = ExtractOptions {
        crop: None,
        max_dim: Some(state.config.frame_max_dim),
        quality: 2,
        timestamp_overlay: state.config.timestamp_overlay,
        font_path: None,
    };

    let scratch_for_task = scratch.clone();
    let ts_for_task = timestamps.clone();
    let extraction = tokio::task::spawn_blocking(move || {
        extract_jpegs(&video_path, &ts_for_task, &scratch_for_task, &opts)
    })
    .await
    .map_err(|e| Error::Config(format!("çıkarma görevi düştü: {e}")))??;

    let mut refs = Vec::with_capacity(extraction.frames.len());
    for frame in &extraction.frames {
        let bytes = std::fs::read(&frame.path)?;
        let key = id.frame_object_key(frame.t_ms);
        state.store.put(&key, &bytes)?;

        let sample = selection
            .frames
            .iter()
            .find(|f| f.t_ms == frame.t_ms);

        refs.push(FrameRef {
            t_ms: frame.t_ms,
            object_key: key,
            motion_score: sample.map(|f| f.motion_score).unwrap_or(0.0),
            is_scene_cut: sample.map(|f| f.is_scene_cut).unwrap_or(false),
        });
    }

    let _ = std::fs::remove_dir_all(&scratch);

    tracing::info!(
        %id,
        ?pass,
        frames = refs.len(),
        elapsed_ms = extraction.elapsed.as_millis() as u64,
        "kareler çıkarıldı"
    );

    state
        .events
        .frames_extracted(&FrameExtracted {
            schema_version: SCHEMA_VERSION,
            video_id: id.clone(),
            batch_id: Uuid::new_v4().to_string(),
            pass,
            frames: refs.clone(),
        })
        .await;

    Ok(refs)
}

/// Pass 2 — videonun tamamının kaba taraması.
pub async fn overview(
    state: &Arc<AppState>,
    id: &VideoId,
    budget: Option<usize>,
) -> Result<Vec<FrameRef>> {
    let profile = profile(state, id).await?;

    let cfg = SamplingConfig {
        budget: budget.unwrap_or(state.config.overview_budget),
        ..state.config.sampling
    };
    let selection = select_frames(&profile, cfg)?;

    tracing::info!(
        %id,
        selected = selection.frames.len(),
        dropped = selection.dropped_duplicates,
        max_gap_ms = selection.max_gap_ms,
        "genel bakış seçildi"
    );

    materialize(state, id, &selection, SamplingPass::Overview).await
}

/// Pass 3 — ajanın işaret ettiği aralığa yakınlaşma.
///
/// Videoyu yeniden çözmez: profilin kesitini alıp aynı örnekleme algoritmasını
/// daha küçük bir bütçeyle koşturur.
pub async fn zoom(
    state: &Arc<AppState>,
    id: &VideoId,
    t0_ms: u64,
    t1_ms: u64,
    budget: Option<usize>,
) -> Result<Vec<FrameRef>> {
    let profile = profile(state, id).await?;
    let sliced = profile.slice(t0_ms, t1_ms);

    if sliced.is_empty() {
        return Err(Error::NotFound(format!(
            "{}–{} ms aralığında kare yok",
            t0_ms, t1_ms
        )));
    }

    let cfg = SamplingConfig {
        budget: budget.unwrap_or(state.config.zoom_budget),
        ..state.config.sampling
    };
    let selection = select_frames(&sliced, cfg)?;

    tracing::info!(
        %id,
        t0_ms,
        t1_ms,
        selected = selection.frames.len(),
        "yakınlaştırma seçildi"
    );

    materialize(state, id, &selection, SamplingPass::Zoom).await
}

/// Tek bir zaman noktasının karesi.
pub async fn single_frame(
    state: &Arc<AppState>,
    id: &VideoId,
    t_ms: u64,
    max_dim: Option<u32>,
) -> Result<FrameRef> {
    let profile = profile(state, id).await?;

    // En yakın analiz örneğinin skorunu taşı: ajan karenin ne kadar hareketli
    // bir ana denk geldiğini görebilsin.
    let nearest = profile
        .samples
        .iter()
        .min_by_key(|s| s.t_ms.abs_diff(t_ms));

    let record = VideoRecord::load(state.store.as_ref(), id)?;
    let video_path = state.store.local_path(&record.object_key)?;

    let scratch = std::env::temp_dir().join(format!("motif-frame-{}", Uuid::new_v4()));
    let opts = ExtractOptions {
        crop: None,
        max_dim: max_dim.or(Some(state.config.frame_max_dim)),
        quality: 2,
        timestamp_overlay: state.config.timestamp_overlay,
        font_path: None,
    };

    let scratch_for_task = scratch.clone();
    let extraction = tokio::task::spawn_blocking(move || {
        extract_jpegs(&video_path, &[t_ms], &scratch_for_task, &opts)
    })
    .await
    .map_err(|e| Error::Config(format!("çıkarma görevi düştü: {e}")))??;

    let frame = extraction
        .frames
        .first()
        .ok_or_else(|| Error::NotFound(format!("{t_ms} ms için kare çıkarılamadı")))?;

    let bytes = std::fs::read(&frame.path)?;
    let key = id.frame_object_key(frame.t_ms);
    state.store.put(&key, &bytes)?;
    let _ = std::fs::remove_dir_all(&scratch);

    Ok(FrameRef {
        t_ms: frame.t_ms,
        object_key: key,
        motion_score: nearest.map(|s| s.score).unwrap_or(0.0),
        is_scene_cut: nearest.map(|s| s.is_scene_cut).unwrap_or(false),
    })
}

// --- Klip üretimi ---
//
// Çıkarım servisi kare kümesi kabul etmiyor: `vlm` görüntüyü tamamen
// reddediyor, diğer modeller en fazla iki tane alıyor. Zamansal içeriğin tek
// teslim biçimi klip. Hareket profili burada "hangi kareleri seçeyim" değil,
// "hangi aralığı keseyim" sorusunu cevaplıyor.

/// Kaynak videodan bir aralığı klip olarak üretir ve depoya yazar.
async fn produce_clip(
    state: &Arc<AppState>,
    id: &VideoId,
    t0_ms: u64,
    t1_ms: u64,
    opts: ClipOptions,
) -> Result<(Clip, String)> {
    let record = VideoRecord::load(state.store.as_ref(), id)?;
    let video_path = state.store.local_path(&record.object_key)?;

    // ffmpeg depo soyutlamasının arkasına yazamaz; önce geçici dosyaya.
    let scratch = std::env::temp_dir().join(format!("motif-clip-{}.mp4", Uuid::new_v4()));
    let scratch_for_task = scratch.clone();

    let clip = tokio::task::spawn_blocking(move || {
        extract_clip(&video_path, t0_ms, t1_ms, &scratch_for_task, &opts)
    })
    .await
    .map_err(|e| Error::Config(format!("klip görevi düştü: {e}")))??;

    let bytes = std::fs::read(&clip.path)?;
    let key = format!(
        "clips/{id}/{:09}-{:09}-x{:.0}.mp4",
        t0_ms,
        t1_ms,
        clip.time_scale * 10.0
    );
    state.store.put(&key, &bytes)?;
    let _ = std::fs::remove_file(&clip.path);

    tracing::info!(
        %id, t0_ms, t1_ms,
        sure_ms = clip.duration_ms,
        olcek = clip.time_scale,
        servis_kare = clip.service_frames,
        etkin_fps = clip.effective_fps,
        boyut_mb = clip.size_bytes as f64 / 1e6,
        "klip üretildi"
    );

    Ok((clip, key))
}

/// Ajanın işaret ettiği aralığı, istenen detay düzeyinde klip olarak üretir.
///
/// Servis sabit 2 fps örneklediği için dar bir pencere göndermek tek başına
/// çözünürlüğü artırmıyor: 2 saniyelik klipten yine 4 kare çıkar. İstenen kare
/// sayısı bundan fazlaysa klip ağır çekime alınıyor — 2 saniyelik aralık 20
/// saniyeye yayılırsa servis 40 kare örnekler, bu da orijinalde 20 fps eder.
pub async fn zoom_clip(
    state: &Arc<AppState>,
    id: &VideoId,
    t0_ms: u64,
    t1_ms: u64,
    budget: usize,
) -> Result<(Clip, String)> {
    let time_scale = scale_for_frames(t1_ms.saturating_sub(t0_ms), budget as u32);

    produce_clip(
        state,
        id,
        t0_ms,
        t1_ms,
        ClipOptions {
            time_scale,
            max_dim: Some(state.config.frame_max_dim),
            ..Default::default()
        },
    )
    .await
}

/// Bir aralığı gerçek zamanda klip olarak üretir (ağır çekim yok).
pub async fn range_clip(
    state: &Arc<AppState>,
    id: &VideoId,
    t0_ms: u64,
    t1_ms: u64,
    max_dim: Option<u32>,
) -> Result<(Clip, String)> {
    produce_clip(
        state,
        id,
        t0_ms,
        t1_ms,
        ClipOptions {
            time_scale: 1.0,
            max_dim: max_dim.or(Some(state.config.frame_max_dim)),
            ..Default::default()
        },
    )
    .await
}
