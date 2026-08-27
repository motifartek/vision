//! Çıkarım servisine giden yükün oluşturulması ve önizlemesi.
//!
//! Sistemin en kolay gözden kaçan sorusu şu: **modele tam olarak ne gidiyor?**
//! Aralık seçimi doğru görünse bile modelin gördüğü şey beklediğimizden farklı
//! olabilir — pencere kaymış, klip ağır çekimde, bağlam şişmiş. Bu modül o yükü
//! tek yerde kurar ve aynı yapıyı hem orkestratöre hem test arayüzüne verir;
//! yani arayüzde görülen şey modele gidenin **birebir aynısı** olur, temsili
//! bir gösterim değil.
//!
//! # Prompt burada üretilmiyor
//!
//! Bir zamanlar üretiliyordu ve `apps/ai/vision` ile ayrışmıştı: panel
//! gönderilmeyen bir metni *"tam olarak bu gidiyor"* diye gösteriyor, üstelik
//! modele servisin desteklemediği araçları tanıtıyordu. Metnin tek kaynağı
//! artık `packages/prompt`; panel istemi `vision`'ın
//! `POST /v1/prompts/preview` ucundan alıyor.
//!
//! Burada kalan iş klibin kendisi ve maliyeti.
//!
//! # Teslim biçimi klip
//!
//! Bu modül önce seçilmiş kareleri JPEG olarak yollamak üzere yazılmıştı. EVREN
//! servisi en fazla iki görüntü kabul ediyor, `vlm` ise hiç kabul etmiyor —
//! zamansal içeriğin tek teslim yolu video. Kare seçimi ortadan kalkmadı ama
//! artık **hangi saniye aralığının kesileceğini** belirliyor; modele giden şey
//! o aralığın klibi.

use motif_event_sdk::{ClipRef, FrameRef, SamplingPass};
use motif_optics::VideoInfo;
use serde::Serialize;

/// Qwen-VL ailesinde bir görüntü karesinin kapladığı token'ın kaba tahmini.
///
/// Görsel kodlayıcı 14x14 piksellik yamalar üretip 2x2 birleştirdiği için
/// etkin adım 28 piksel olur: `token ≈ (G/28) × (Y/28)`.
///
/// EVREN üzerinde ölçülen değerlerle mertebe olarak tutuyor: 35 saniyelik 480p
/// kayıt 11.064 giriş token'ı harcadı, kabaca **saniyede ~300 token**.
const VLM_PATCH_STRIDE: u32 = 28;

/// Aralığı seçtiren an.
///
/// **Modele gitmiyor.** Hareket profilinin hangi anları önemli bulduğunu
/// gösteriyor; kesilecek pencere bunlara bakılarak belirleniyor. Arayüzde
/// kanıt olarak duruyor, yükün parçası değil.
#[derive(Debug, Clone, Serialize)]
pub struct PayloadFrame {
    pub ord: usize,
    pub t_ms: u64,
    /// `"00:15.2"` biçiminde okunabilir zaman.
    pub time: String,
    /// Karenin inceleme için sunulacağı adres.
    pub url: String,
    pub motion_score: f32,
    pub is_scene_cut: bool,
}

/// Modele giden klip.
#[derive(Debug, Clone, Serialize)]
pub struct PayloadClip {
    pub t0_ms: u64,
    pub t1_ms: u64,
    /// Kaynaktaki gerçek aralık uzunluğu.
    pub source_span_ms: u64,
    /// Klibin kendi uzunluğu. Ağır çekimde kaynaktan uzundur.
    pub duration_ms: u64,
    pub time_scale: f32,
    /// Servisin bu klipten çıkaracağı kare sayısı (2 fps).
    pub service_frames: u32,
    /// Ağır çekim hesaba katıldığında kaynaktan örneklenen efektif fps.
    pub effective_fps: f64,
    pub size_bytes: u64,
    pub object_key: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TokenEstimate {
    pub frame_width: u32,
    pub frame_height: u32,
    pub per_frame: u32,
    pub frames_total: u32,
    /// Klibin kare maliyeti. İstem metninin token'ı buraya **dahil değil**;
    /// onu `vision` önizleme ucu bildiriyor, panel ikisini topluyor.
    pub total: u32,
    pub note: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct Reduction {
    /// Videonun ham kare sayısı.
    pub source_frames: u64,
    /// Servisin klipten göreceği kare sayısı.
    pub sent_frames: u32,
    pub ratio: f64,
    /// Ham videonun tamamı gönderilseydi tahmini token.
    pub tokens_if_naive: u64,
}

/// Modele gönderilen tam yük.
#[derive(Debug, Clone, Serialize)]
pub struct VlmPayload {
    pub pass: SamplingPass,
    pub clip: PayloadClip,
    pub tokens: TokenEstimate,
    pub reduction: Reduction,
    /// Aralığı seçtiren anlar. Modele gitmiyor, inceleme içindir.
    pub evidence_frames: Vec<PayloadFrame>,
}

/// Karenin ölçeklendikten sonraki boyutu.
///
/// Kutuya sığdırılır, en-boy oranı korunur, büyütme yapılmaz — `optics`
/// tarafındaki `scale=...:force_original_aspect_ratio=decrease` ile aynı kural.
fn scaled_dims(info: &VideoInfo, max_dim: u32) -> (u32, u32) {
    let uzun = info.width.max(info.height);
    if uzun == 0 || uzun <= max_dim {
        return (info.width, info.height);
    }
    let oran = max_dim as f64 / uzun as f64;
    (
        ((info.width as f64 * oran).round() as u32).max(1),
        ((info.height as f64 * oran).round() as u32).max(1),
    )
}

fn estimate_frame_tokens(w: u32, h: u32) -> u32 {
    let gw = w.div_ceil(VLM_PATCH_STRIDE).max(1);
    let gh = h.div_ceil(VLM_PATCH_STRIDE).max(1);
    gw * gh
}

/// Klip ve onu seçtiren karelerden modele gidecek yükü kurar.
pub fn build(
    pass: SamplingPass,
    clip: &ClipRef,
    size_bytes: u64,
    evidence: &[FrameRef],
    info: &VideoInfo,
    max_dim: u32,
) -> VlmPayload {
    let payload_clip = PayloadClip {
        t0_ms: clip.t0_ms,
        t1_ms: clip.t1_ms,
        source_span_ms: clip.t1_ms.saturating_sub(clip.t0_ms),
        duration_ms: clip.duration_ms,
        time_scale: clip.time_scale,
        service_frames: clip.service_frames,
        effective_fps: clip.effective_fps,
        size_bytes,
        url: format!("/v1/blobs/{}", clip.object_key),
        object_key: clip.object_key.clone(),
    };

    let evidence_frames: Vec<PayloadFrame> = evidence
        .iter()
        .enumerate()
        .map(|(i, f)| PayloadFrame {
            ord: i + 1,
            t_ms: f.t_ms,
            time: format_timestamp_ms(f.t_ms),
            url: format!("/v1/blobs/{}", f.object_key),
            motion_score: f.motion_score,
            is_scene_cut: f.is_scene_cut,
        })
        .collect();

    let (fw, fh) = scaled_dims(info, max_dim);
    let per_frame = estimate_frame_tokens(fw, fh);
    let frames_total = per_frame * payload_clip.service_frames;

    let source_frames = (info.duration_ms as f64 / 1000.0 * info.fps).round() as u64;

    VlmPayload {
        pass,
        tokens: TokenEstimate {
            frame_width: fw,
            frame_height: fh,
            per_frame,
            frames_total,
            total: frames_total,
            note: "Qwen-VL ailesi için tahmin: (G/28)×(Y/28) × servisin çıkaracağı \
                   kare. Servis her videoyu 2 fps örneklüyor.",
        },
        reduction: Reduction {
            source_frames,
            sent_frames: payload_clip.service_frames,
            ratio: source_frames as f64 / payload_clip.service_frames.max(1) as f64,
            tokens_if_naive: source_frames * per_frame as u64,
        },
        clip: payload_clip,
        evidence_frames,
    }
}

/// Saniyenin onda birini de gösteren zaman biçimi.
///
/// `format_timestamp` saniye hassasiyetinde; kare seçimi ise saniyenin altında
/// çalışıyor. Aynı saniyeye düşen iki karenin ayırt edilebilmesi gerekiyor.
fn format_timestamp_ms(t_ms: u64) -> String {
    let toplam = t_ms / 1000;
    let salise = (t_ms % 1000) / 100;
    format!("{:02}:{:02}.{}", toplam / 60, toplam % 60, salise)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip_ref(t0: u64, t1: u64, scale: f32, frames: u32) -> ClipRef {
        ClipRef {
            t0_ms: t0,
            t1_ms: t1,
            object_key: "clips/x/000-001.mp4".into(),
            duration_ms: ((t1 - t0) as f32 * scale) as u64,
            time_scale: scale,
            service_frames: frames,
            effective_fps: 2.0 * scale as f64,
        }
    }

    fn info() -> VideoInfo {
        VideoInfo {
            duration_ms: 35_000,
            fps: 30.0,
            width: 854,
            height: 480,
            size_bytes: 4_000_000,
            codec: "h264".into(),
        }
    }

    fn frames(n: usize) -> Vec<FrameRef> {
        (0..n)
            .map(|i| FrameRef {
                t_ms: i as u64 * 3_000,
                object_key: format!("frames/v/{:09}.jpg", i * 3000),
                motion_score: 0.5,
                is_scene_cut: false,
            })
            .collect()
    }

    #[test]
    fn olcekleme_orani_korur_ve_buyutmez() {
        let mut i = info();
        i.width = 480;
        i.height = 480;
        assert_eq!(scaled_dims(&i, 768), (480, 480), "kucuk kare buyutulmemeli");

        i.width = 1920;
        i.height = 1080;
        let (w, h) = scaled_dims(&i, 768);
        assert_eq!(w, 768);
        assert_eq!(h, 432, "en-boy orani korunmali");
    }

    #[test]
    fn zaman_biciminde_salise_var() {
        // Ayni saniyeye dusen iki kare ayirt edilebilmeli.
        assert_eq!(format_timestamp_ms(15_200), "00:15.2");
        assert_eq!(format_timestamp_ms(15_800), "00:15.8");
        assert_ne!(format_timestamp_ms(15_200), format_timestamp_ms(15_800));
    }

    #[test]
    fn kanit_kareleri_token_hesabina_karismaz() {
        // Kareler modele gitmiyor; yuke dahil edilirlerse tahmin sisiyor.
        let c = clip_ref(0, 35_000, 1.0, 70);
        let bos = build(SamplingPass::Overview, &c, 900_000, &[], &info(), 768);
        let dolu = build(SamplingPass::Overview, &c, 900_000, &frames(16), &info(), 768);

        assert_eq!(bos.tokens.total, dolu.tokens.total);
        assert_eq!(dolu.evidence_frames.len(), 16);
        assert_eq!(dolu.reduction.sent_frames, 70, "gonderilen kare klipten gelir");
    }

    // Not: istem metnine bakan testler `packages/prompt`e taşındı. Burası
    // artık prompt üretmiyor; kalan iddialar klibin kendisiyle ilgili.

    #[test]
    fn agir_cekim_klibin_suresine_yansir() {
        let c = clip_ref(12_000, 15_000, 8.0, 47);
        let p = build(SamplingPass::Zoom, &c, 480_000, &[], &info(), 768);

        assert_eq!(p.clip.source_span_ms, 3_000);
        assert_eq!(p.clip.duration_ms, 24_000);
    }

    #[test]
    fn gercek_zamanli_klipte_sure_kaynakla_ayni() {
        let c = clip_ref(10_000, 22_000, 1.0, 24);
        let p = build(SamplingPass::Zoom, &c, 200_000, &[], &info(), 768);

        assert_eq!(p.clip.source_span_ms, p.clip.duration_ms);
    }

    #[test]
    fn token_tahmini_servis_kare_sayisina_dayanir() {
        let c = clip_ref(0, 35_000, 1.0, 70);
        let p = build(SamplingPass::Overview, &c, 900_000, &[], &info(), 768);

        assert_eq!(p.reduction.sent_frames, 70);
        assert_eq!(p.tokens.frames_total, p.tokens.per_frame * 70);
        // 35 sn × 30 fps = 1050 ham kare
        assert_eq!(p.reduction.source_frames, 1_050);
        assert!(p.reduction.ratio > 14.0 && p.reduction.ratio < 15.5);
    }
}
