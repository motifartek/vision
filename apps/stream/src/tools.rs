//! Ajanın çağırabileceği araçlar.
//!
//! Sistemin ayırt edici parçası burası. Ajan videoyu tek seferde izlemiyor;
//! önce kaba bir bakış alıyor, ilgisini çeken ana **kendi kararıyla**
//! yakınlaşıyor, gerekirse bölgeyi kırpıp büyütüyor. Yani videoyu izlemiyor,
//! **soruşturuyor**.
//!
//! Şartname açısından karşılığı:
//! - §7 "mock fonksiyonların ajanın araçları olarak başarıyla kullanılması"
//!   (%35 fonksiyonellik içinde)
//! - §7 "Otonomi ve Zeka" (%20): dinamik araç seçimi, çok adımlı karar
//!   zincirleri
//!
//! Buradaki fonksiyonlar taşımadan bağımsızdır: HTTP uçları da NATS
//! istek/cevap tüketicisi de aynı gövdeyi çağırır. Taşıma katmanı yalnızca
//! serileştirme yapar, iş mantığı tek yerde durur.

use std::sync::Arc;

use motif_core::{Error, VideoId};
use motif_event_sdk::tools::{
    ClipRangeRequest, ClipRef, ClipResponse, CropRegionRequest, FrameResponse, FramesResponse,
    GetFrameRequest, MotionBucket, MotionProfileRequest, MotionProfileResponse,
    SampleOverviewRequest, ToolError, ToolErrorCode, VideoInfoRequest, VideoInfoResponse,
    ZoomRangeRequest,
};
use motif_event_sdk::FrameRef;
use motif_optics::{extract_jpegs, CropBox, ExtractOptions};
use uuid::Uuid;

use crate::catalog::VideoRecord;
use crate::pipeline;
use crate::state::AppState;

pub type ToolResult<T> = std::result::Result<T, ToolError>;

/// Dahili hatayı ajana gösterilebilir bir araç hatasına çevirir.
///
/// Ajanın toparlanabilmesi için makine tarafından okunabilir bir kod ve
/// Türkçe bir mesaj taşınır: model hatayı okuyup başka bir aralık deneyebilsin.
fn to_tool_error(err: Error) -> ToolError {
    let code = match &err {
        Error::NotFound(_) => ToolErrorCode::UnknownVideo,
        Error::Config(_) => ToolErrorCode::InvalidArgument,
        _ => ToolErrorCode::Internal,
    };
    ToolError {
        code,
        message: err.to_string(),
    }
}

fn invalid(message: impl Into<String>) -> ToolError {
    ToolError {
        code: ToolErrorCode::InvalidArgument,
        message: message.into(),
    }
}

/// Videonun var olduğunu doğrular ve kaydını döndürür.
fn require_video(state: &Arc<AppState>, id: &VideoId) -> ToolResult<VideoRecord> {
    VideoRecord::load(state.store.as_ref(), id).map_err(|_| ToolError {
        code: ToolErrorCode::UnknownVideo,
        message: format!("bilinmeyen video: {id}"),
    })
}

/// İstenen zamanın video sınırları içinde olduğunu doğrular.
fn require_in_range(record: &VideoRecord, t_ms: u64) -> ToolResult<()> {
    if t_ms > record.info.duration_ms {
        return Err(ToolError {
            code: ToolErrorCode::OutOfRange,
            message: format!(
                "{} ms video süresinin ({} ms) dışında",
                t_ms, record.info.duration_ms
            ),
        });
    }
    Ok(())
}

// --- video_info ---

pub async fn video_info(
    state: &Arc<AppState>,
    req: VideoInfoRequest,
) -> ToolResult<VideoInfoResponse> {
    let record = require_video(state, &req.video_id)?;
    Ok(VideoInfoResponse {
        duration_ms: record.info.duration_ms,
        fps: record.info.fps,
        width: record.info.width,
        height: record.info.height,
        size_bytes: record.info.size_bytes,
        codec: record.info.codec,
    })
}

// --- motion_profile ---

pub async fn motion_profile(
    state: &Arc<AppState>,
    req: MotionProfileRequest,
) -> ToolResult<MotionProfileResponse> {
    require_video(state, &req.video_id)?;

    if req.bucket_ms == 0 {
        return Err(invalid("bucket_ms sıfır olamaz"));
    }

    let profile = pipeline::profile(state, &req.video_id)
        .await
        .map_err(to_tool_error)?;

    // Ham profil binlerce örnek içerir; ajana kovalanmış hali gider.
    let buckets = profile
        .bucketed(req.bucket_ms)
        .into_iter()
        .map(|(t_ms, score, is_scene_cut)| MotionBucket {
            t_ms,
            score,
            is_scene_cut,
        })
        .collect();

    Ok(MotionProfileResponse {
        duration_ms: profile.duration_ms,
        buckets,
    })
}

// --- sample_overview ---

pub async fn sample_overview(
    state: &Arc<AppState>,
    req: SampleOverviewRequest,
) -> ToolResult<FramesResponse> {
    require_video(state, &req.video_id)?;

    if req.budget == 0 {
        return Err(invalid("budget sıfır olamaz"));
    }

    // Yeni bir analiz turu başlıyor: yakınlaştırma bütçesi tazelenir.
    state.reset_zooms(&req.video_id).await;

    let frames = pipeline::overview(state, &req.video_id, Some(req.budget))
        .await
        .map_err(to_tool_error)?;

    Ok(FramesResponse { frames })
}

// --- zoom_range ---

/// Ajanın işaret ettiği aralığa yakınlaşır.
///
/// Boru hattının değil **ajanın** çağırdığı yer burası: kaba bakışta bir şey
/// dikkatini çektiğinde o aralığı daha ayrıntılı ister.
///
/// Çıkarım servisi kare kümesi kabul etmediği için çıktı bir **klip**. Servis
/// sabit 2 fps örneklediğinden, istenen kare sayısı aralığın gerçek zamanda
/// vereceğinden fazlaysa klip ağır çekime alınır.
pub async fn zoom_range(state: &Arc<AppState>, req: ZoomRangeRequest) -> ToolResult<ClipResponse> {
    let record = require_video(state, &req.video_id)?;

    if req.budget == 0 {
        return Err(invalid("budget sıfır olamaz"));
    }
    if req.t1_ms <= req.t0_ms {
        return Err(invalid(format!(
            "geçersiz aralık: t1 ({}) t0'dan ({}) büyük olmalı",
            req.t1_ms, req.t0_ms
        )));
    }
    require_in_range(&record, req.t0_ms)?;

    // Üst sınır: ajan yakınlaşmaya kendi karar verdiği için kararsız bir model
    // aynı aralığa tekrar tekrar girip gecikmeyi sınırsız büyütebilir.
    if !state.try_consume_zoom(&req.video_id).await {
        return Err(ToolError {
            code: ToolErrorCode::ZoomLimitExceeded,
            message: format!(
                "bu video için yakınlaştırma sınırına ulaşıldı ({}); \
                 eldeki karelerle sonuca varın",
                state.config.max_zooms_per_video
            ),
        });
    }

    let (clip, key) = pipeline::zoom_clip(
        state,
        &req.video_id,
        req.t0_ms,
        req.t1_ms.min(record.info.duration_ms),
        req.budget,
    )
    .await
    .map_err(to_tool_error)?;

    Ok(ClipResponse { clip: to_clip_ref(clip, key) })
}

// --- clip_range ---

/// Bir aralığı gerçek zamanda klip olarak üretir.
///
/// `zoom_range`'den farkı ağır çekim uygulamaması: videonun tamamını ya da
/// geniş bir bölümünü göndermek için.
pub async fn clip_range(state: &Arc<AppState>, req: ClipRangeRequest) -> ToolResult<ClipResponse> {
    let record = require_video(state, &req.video_id)?;

    if req.t1_ms <= req.t0_ms {
        return Err(invalid(format!(
            "geçersiz aralık: t1 ({}) t0'dan ({}) büyük olmalı",
            req.t1_ms, req.t0_ms
        )));
    }
    require_in_range(&record, req.t0_ms)?;

    let (clip, key) = pipeline::range_clip(
        state,
        &req.video_id,
        req.t0_ms,
        req.t1_ms.min(record.info.duration_ms),
        req.max_dim,
    )
    .await
    .map_err(to_tool_error)?;

    Ok(ClipResponse { clip: to_clip_ref(clip, key) })
}

fn to_clip_ref(clip: motif_optics::Clip, object_key: String) -> ClipRef {
    ClipRef {
        t0_ms: clip.t0_ms,
        t1_ms: clip.t1_ms,
        object_key,
        duration_ms: clip.duration_ms,
        time_scale: clip.time_scale,
        service_frames: clip.service_frames,
        effective_fps: clip.effective_fps,
    }
}

// --- get_frame ---

pub async fn get_frame(state: &Arc<AppState>, req: GetFrameRequest) -> ToolResult<FrameResponse> {
    let record = require_video(state, &req.video_id)?;
    require_in_range(&record, req.t_ms)?;

    let frame = pipeline::single_frame(state, &req.video_id, req.t_ms, req.max_dim)
        .await
        .map_err(to_tool_error)?;

    Ok(FrameResponse { frame })
}

// --- crop_region ---

/// Bir karenin belirli bölgesini kırpıp büyütür.
///
/// Kaba bakışta "sağ altta bir şey var" diyen ajan, oraya uzamsal olarak
/// yakınlaşabilsin diye. Kırpma ölçeklemeden önce uygulandığı için bölge
/// gerçekten detaylanır, düşük çözünürlükten büyütülmez.
pub async fn crop_region(
    state: &Arc<AppState>,
    req: CropRegionRequest,
) -> ToolResult<FrameResponse> {
    let record = require_video(state, &req.video_id)?;
    require_in_range(&record, req.t_ms)?;

    let crop = CropBox {
        x: req.bbox.x,
        y: req.bbox.y,
        w: req.bbox.w,
        h: req.bbox.h,
    }
    .clamped();

    let video_path = state
        .store
        .local_path(&record.object_key)
        .map_err(to_tool_error)?;

    let scratch = std::env::temp_dir().join(format!("motif-crop-{}", Uuid::new_v4()));
    let opts = ExtractOptions {
        crop: Some(crop),
        max_dim: Some(state.config.frame_max_dim),
        quality: 2,
        timestamp_overlay: state.config.timestamp_overlay,
        font_path: None,
    };

    let scratch_for_task = scratch.clone();
    let t_ms = req.t_ms;
    let extraction = tokio::task::spawn_blocking(move || {
        extract_jpegs(&video_path, &[t_ms], &scratch_for_task, &opts)
    })
    .await
    .map_err(|e| ToolError {
        code: ToolErrorCode::Internal,
        message: format!("kırpma görevi düştü: {e}"),
    })?
    .map_err(to_tool_error)?;

    let extracted = extraction.frames.first().ok_or_else(|| ToolError {
        code: ToolErrorCode::OutOfRange,
        message: format!("{t_ms} ms için kare çıkarılamadı"),
    })?;

    let bytes = std::fs::read(&extracted.path).map_err(|e| ToolError {
        code: ToolErrorCode::Internal,
        message: e.to_string(),
    })?;

    // Kırpılmış kare ayrı anahtara yazılır; aynı andaki tam kareyi ezmesin.
    let key = format!(
        "frames/{}/crop-{:09}-{}.jpg",
        req.video_id,
        extracted.t_ms,
        Uuid::new_v4().simple()
    );
    state.store.put(&key, &bytes).map_err(to_tool_error)?;
    let _ = std::fs::remove_dir_all(&scratch);

    Ok(FrameResponse {
        frame: FrameRef {
            t_ms: extracted.t_ms,
            object_key: key,
            motion_score: 0.0,
            is_scene_cut: false,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventPublisher;
    use crate::storage::LocalStore;
    use motif_optics::VideoInfo;

    async fn state_with_video(max_zooms: usize) -> (Arc<AppState>, VideoId) {
        let dir = std::env::temp_dir().join(format!("motif-tools-{}", Uuid::new_v4()));
        let config = crate::config::Config {
            max_zooms_per_video: max_zooms,
            storage_root: dir.clone(),
            ..crate::config::Config::from_env()
        };
        let store = Arc::new(LocalStore::new(&dir).unwrap());
        let state = Arc::new(AppState::new(
            config,
            store.clone(),
            EventPublisher::connect(None).await,
        ));

        let id = VideoId::new();
        VideoRecord {
            id: id.clone(),
            original_name: "t.mp4".into(),
            object_key: id.raw_object_key("mp4"),
            info: VideoInfo {
                duration_ms: 20_000,
                fps: 30.0,
                width: 640,
                height: 360,
                size_bytes: 1,
                codec: "h264".into(),
            },
            uploaded_at: chrono::Utc::now(),
        }
        .save(store.as_ref())
        .unwrap();

        (state, id)
    }

    #[tokio::test]
    async fn bilinmeyen_video_ayirt_edilebilir_hata_verir() {
        let (state, _) = state_with_video(8).await;

        let err = video_info(
            &state,
            VideoInfoRequest {
                video_id: VideoId::new(),
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.code, ToolErrorCode::UnknownVideo);
    }

    #[tokio::test]
    async fn video_disi_zaman_out_of_range_verir() {
        let (state, id) = state_with_video(8).await;

        let err = get_frame(
            &state,
            GetFrameRequest {
                video_id: id,
                t_ms: 999_000,
                max_dim: None,
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.code, ToolErrorCode::OutOfRange);
    }

    #[tokio::test]
    async fn ters_aralik_reddedilir() {
        let (state, id) = state_with_video(8).await;

        let err = zoom_range(
            &state,
            ZoomRangeRequest {
                video_id: id,
                t0_ms: 5_000,
                t1_ms: 3_000,
                budget: 8,
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.code, ToolErrorCode::InvalidArgument);
    }

    #[tokio::test]
    async fn yakinlastirma_siniri_asilinca_ajana_bildirilir() {
        let (state, id) = state_with_video(1).await;

        // İlk çağrı bütçeyi tüketir; video dosyası olmadığı için sonrasında
        // başka bir hatayla düşer ama sayaç harcanmış olur.
        let _ = zoom_range(
            &state,
            ZoomRangeRequest {
                video_id: id.clone(),
                t0_ms: 1_000,
                t1_ms: 2_000,
                budget: 4,
            },
        )
        .await;

        let err = zoom_range(
            &state,
            ZoomRangeRequest {
                video_id: id,
                t0_ms: 3_000,
                t1_ms: 4_000,
                budget: 4,
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.code, ToolErrorCode::ZoomLimitExceeded);
        // Mesaj ajana ne yapacağını söylemeli.
        assert!(err.message.contains("sonuca varın"));
    }

    #[tokio::test]
    async fn sifir_butce_reddedilir() {
        let (state, id) = state_with_video(8).await;

        let err = sample_overview(
            &state,
            SampleOverviewRequest {
                video_id: id,
                budget: 0,
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.code, ToolErrorCode::InvalidArgument);
    }
}
