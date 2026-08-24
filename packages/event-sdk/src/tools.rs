//! Ajanın çağırabileceği stream araçları (pass 3).
//!
//! Bu araçlar sistemin ayırt edici parçasıdır: ajan videoyu tek seferde
//! izlemek yerine, ilgisini çeken anlara **yakınlaşarak soruşturur**.
//! Şartnamenin "mock fonksiyonların ajanın araçları olarak kullanılması"
//! ve "dinamik araç seçimi / çok adımlı karar zincirleri" maddeleri
//! doğrudan bu yüzeyle karşılanır.
//!
//! Taşıma katmanı NATS istek/cevaptır; konu adları için
//! [`crate::subjects::tool`] kullanılır.

use motif_core::VideoId;
use serde::{Deserialize, Serialize};

use crate::FrameRef;

/// Araç adları. Konu üretiminde ve ajana sunulan şemada kullanılır.
pub mod names {
    pub const VIDEO_INFO: &str = "video_info";
    pub const MOTION_PROFILE: &str = "motion_profile";
    pub const SAMPLE_OVERVIEW: &str = "sample_overview";
    pub const ZOOM_RANGE: &str = "zoom_range";
    pub const GET_FRAME: &str = "get_frame";
    pub const CROP_REGION: &str = "crop_region";

    /// Ajana sunulacak tüm araçlar.
    pub const ALL: &[&str] = &[
        VIDEO_INFO,
        MOTION_PROFILE,
        SAMPLE_OVERVIEW,
        ZOOM_RANGE,
        GET_FRAME,
        CROP_REGION,
    ];
}

// --- video_info ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoInfoRequest {
    pub video_id: VideoId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoInfoResponse {
    pub duration_ms: u64,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
    pub codec: String,
}

// --- motion_profile ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionProfileRequest {
    pub video_id: VideoId,
    /// Örnekleri bu genişlikte kovalara indirger. Ajanın bağlamını
    /// şişirmemek için; ham profil binlerce örnek olabilir.
    pub bucket_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionBucket {
    pub t_ms: u64,
    pub score: f32,
    pub is_scene_cut: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionProfileResponse {
    pub duration_ms: u64,
    pub buckets: Vec<MotionBucket>,
}

// --- sample_overview ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleOverviewRequest {
    pub video_id: VideoId,
    /// Kaç kare istendiği. Donanım belli olmadığı için sabit değil.
    pub budget: usize,
}

// --- zoom_range ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoomRangeRequest {
    pub video_id: VideoId,
    pub t0_ms: u64,
    pub t1_ms: u64,
    pub budget: usize,
}

/// Kare döndüren araçların ortak cevabı.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FramesResponse {
    pub frames: Vec<FrameRef>,
}

// --- get_frame ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFrameRequest {
    pub video_id: VideoId,
    pub t_ms: u64,
    /// Uzun kenarın piksel sınırı; kare bunu aşmayacak şekilde küçültülür.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_dim: Option<u32>,
}

// --- crop_region ---

/// Normalize edilmiş kırpma kutusu (0.0..1.0).
///
/// Piksel yerine oran kullanılır; ajan çözünürlüğü bilmek zorunda kalmaz
/// ve kutu farklı çözünürlüklerde geçerli kalır.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CropRegionRequest {
    pub video_id: VideoId,
    pub t_ms: u64,
    pub bbox: BBox,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameResponse {
    pub frame: FrameRef,
}

/// Araç çağrısı başarısız olduğunda dönen hata.
///
/// Ajanın toparlanabilmesi için makine tarafından okunabilir bir kod ve
/// modele gösterilebilecek Türkçe bir mesaj taşır.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolError {
    pub code: ToolErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolErrorCode {
    /// Video kimliği bilinmiyor.
    UnknownVideo,
    /// İstenen zaman videonun dışında.
    OutOfRange,
    /// Ajan izin verilen yakınlaşma derinliğini aştı.
    ZoomLimitExceeded,
    /// Geçersiz parametre (ör. t1 <= t0, budget 0).
    InvalidArgument,
    /// Sunucu tarafı hata.
    Internal,
}
