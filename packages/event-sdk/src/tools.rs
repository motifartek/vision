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
    /// Bir aralığı klip olarak üretir (ağır çekim olmadan).
    pub const CLIP_RANGE: &str = "clip_range";

    /// Ajana sunulacak tüm araçlar.
    pub const ALL: &[&str] = &[
        VIDEO_INFO,
        MOTION_PROFILE,
        SAMPLE_OVERVIEW,
        ZOOM_RANGE,
        GET_FRAME,
        CROP_REGION,
        CLIP_RANGE,
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
    /// Bu aralıktan kaç kare görmek istendiği.
    ///
    /// Servis sabit 2 fps örneklediği için dar bir pencere göndermek tek
    /// başına çözünürlüğü artırmıyor: 2 saniyelik klipten yine 4 kare çıkar.
    /// İstenen kare sayısı bundan fazlaysa klip otomatik olarak ağır çekime
    /// alınır.
    pub budget: usize,
}

/// Üretilmiş bir video klibi.
///
/// Çıkarım servisi kare kümesi kabul etmiyor — `vlm` görüntüyü tamamen
/// reddediyor, diğerleri en fazla iki tane alıyor. Bu yüzden zamansal içeriğin
/// teslim birimi klip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipRef {
    /// Kaynak videodaki başlangıç.
    pub t0_ms: u64,
    /// Kaynak videodaki bitiş.
    pub t1_ms: u64,
    pub object_key: String,
    /// Klibin kendi süresi. Ağır çekimde kaynak aralığından uzun olur.
    pub duration_ms: u64,
    /// Zaman ölçeği. 1.0 gerçek zaman, 10.0 on kat ağır çekim.
    pub time_scale: f32,
    /// Servisin bu klipten çıkaracağı kare sayısı (2 fps).
    pub service_frames: u32,
    /// Kaynak aralığa göre etkin kare hızı.
    ///
    /// Ajanın ne kadar detay aldığını bilmesi için taşınıyor: 2.0 gerçek
    /// zaman demek, 20.0 ise on kat yavaşlatılmış bir pencere.
    pub effective_fps: f64,
}

impl ClipRef {
    /// Klip içindeki bir zamanı kaynak videodaki zamana çevirir.
    ///
    /// Ağır çekim kullanıldığında model klibin **kendi** saatini raporluyor.
    /// Ölçüldü: 12.0-15.0 sn aralığı 8 kat yavaşlatılıp gönderildiğinde model
    /// olayları 00:20-00:22 olarak verdi, oysa kaynakta 00:12-00:15. Prompt'ta
    /// dönüşüm formülü açıkça verilmesine rağmen düzelmedi — model bu aritmetiği
    /// güvenilir yapmıyor.
    ///
    /// Bu yüzden dönüşüm modele bırakılmıyor, burada yapılıyor.
    pub fn to_source_ms(&self, clip_ms: u64) -> u64 {
        if self.time_scale <= 0.0 {
            return self.t0_ms + clip_ms;
        }
        let kaynak_offset = (clip_ms as f64 / self.time_scale as f64).round() as u64;
        (self.t0_ms + kaynak_offset).min(self.t1_ms)
    }

    /// Modelin klip saatiyle verdiği olayları kaynak zamanına taşır.
    pub fn rebase_events(&self, events: &mut [crate::DetectedEvent]) {
        for e in events {
            e.t_ms = self.to_source_ms(e.t_ms);
            e.time = crate::format_timestamp(e.t_ms);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipResponse {
    pub clip: ClipRef,
}

/// Kare döndüren araçların ortak cevabı.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FramesResponse {
    pub frames: Vec<FrameRef>,
}

// --- clip_range ---

/// Bir zaman aralığını klip olarak ister.
///
/// `zoom_range`'den farkı: kare hedefi yok, aralık gerçek zamanda çıkarılır.
/// Videonun tamamını ya da geniş bir bölümünü göndermek için.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipRangeRequest {
    pub video_id: VideoId,
    pub t0_ms: u64,
    pub t1_ms: u64,
    /// Uzun kenar sınırı; verilmezse kaynak çözünürlüğü korunur.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_dim: Option<u32>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DetectedEvent, RiskLevel};

    fn clip(t0: u64, t1: u64, scale: f32) -> ClipRef {
        ClipRef {
            t0_ms: t0,
            t1_ms: t1,
            object_key: "clips/x.mp4".into(),
            duration_ms: ((t1 - t0) as f32 * scale) as u64,
            time_scale: scale,
            service_frames: 0,
            effective_fps: 2.0 * scale as f64,
        }
    }

    #[test]
    fn agir_cekim_zamani_kaynaga_tasinir() {
        // Ölçülen gerçek durum: 12.0-15.0 sn aralığı 8 kat yavaşlatıldı,
        // model olayı klibin 20. saniyesinde gördü. Kaynakta 12 + 20/8 = 14.5 sn.
        let c = clip(12_000, 15_000, 8.0);
        assert_eq!(c.to_source_ms(20_000), 14_500);
        assert_eq!(c.to_source_ms(0), 12_000);
        // Klip sonunu aşan değer kaynak aralığın dışına taşmamalı
        assert_eq!(c.to_source_ms(999_000), 15_000);
    }

    #[test]
    fn gercek_zamanda_sadece_kaydirma_olur() {
        let c = clip(30_000, 40_000, 1.0);
        assert_eq!(c.to_source_ms(0), 30_000);
        assert_eq!(c.to_source_ms(5_000), 35_000);
    }

    #[test]
    fn olaylar_toplu_tasinir_ve_metin_guncellenir() {
        let c = clip(12_000, 15_000, 8.0);
        let mut olaylar = vec![
            DetectedEvent::new(8_000, "Raf devrildi", RiskLevel::Yuksek),
            DetectedEvent::new(20_000, "Toz bulutu yayıldı", RiskLevel::Orta),
        ];
        c.rebase_events(&mut olaylar);

        assert_eq!(olaylar[0].t_ms, 13_000);
        assert_eq!(olaylar[0].time, "00:13");
        assert_eq!(olaylar[1].t_ms, 14_500);
        assert_eq!(olaylar[1].time, "00:14");
    }
}
