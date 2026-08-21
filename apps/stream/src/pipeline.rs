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
    build_profile, extract_jpegs, probe, select_frames, ExtractOptions, MotionProfile,
    SamplingConfig, Selection,
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
    let info = tokio::task::spawn_blocking(move || probe(&video_path))
        .await
        .map_err(|e| Error::Config(format!("probe görevi düştü: {e}")))??;

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
