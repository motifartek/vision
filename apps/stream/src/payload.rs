//! VLM'e giden yükün oluşturulması ve önizlemesi.
//!
//! Sistemin en kolay gözden kaçan sorusu şu: **modele tam olarak ne gidiyor?**
//! Kare seçimi doğru görünse bile modelin gördüğü şey beklediğimizden farklı
//! olabilir — sıra bozuk, zaman damgası yok, çözünürlük düşük, bağlam şişmiş.
//! Bu modül o yükü tek yerde kurar ve aynı yapıyı hem orkestratöre hem test
//! arayüzüne verir; yani arayüzde görülen şey modele gidenin **birebir aynısı**
//! olur, temsili bir gösterim değil.

use motif_event_sdk::{format_timestamp, FrameRef, SamplingPass};
use motif_optics::VideoInfo;
use serde::Serialize;

/// Qwen2-VL ailesinde bir görüntü karesinin kapladığı token'ın kaba tahmini.
///
/// Görsel kodlayıcı 14x14 piksellik yamalar üretip 2x2 birleştirdiği için
/// etkin adım 28 piksel olur: `token ≈ (G/28) × (Y/28)`.
///
/// **Tahmindir.** Kesin sayı modele ve sunucu yapılandırmasına göre değişir;
/// hedef donanım ve model seçildiğinde (#8) gerçek sayımla değiştirilmeli.
/// Yine de mertebeyi doğru verdiği için bütçe kararlarını yönlendirmeye yeter.
const VLM_PATCH_STRIDE: u32 = 28;

/// Türkçe metinde kabaca bir token'a düşen karakter sayısı.
const CHARS_PER_TOKEN: usize = 4;

#[derive(Debug, Clone, Serialize)]
pub struct PayloadFrame {
    /// Modele gidiş sırası (1'den başlar).
    pub ord: usize,
    pub t_ms: u64,
    /// `"00:15.2"` biçiminde okunabilir zaman.
    pub time: String,
    /// Karenin sunulacağı adres.
    pub url: String,
    pub motion_score: f32,
    pub is_scene_cut: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct TokenEstimate {
    pub frame_width: u32,
    pub frame_height: u32,
    pub per_frame: u32,
    pub frames_total: u32,
    pub text: u32,
    pub total: u32,
    pub note: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct Reduction {
    /// Videonun ham kare sayısı.
    pub source_frames: u64,
    /// Modele giden kare sayısı.
    pub sent_frames: usize,
    pub ratio: f64,
    /// Ham videonun tamamı gönderilseydi tahmini token.
    pub tokens_if_naive: u64,
}

/// Modele gönderilen tam yük.
#[derive(Debug, Clone, Serialize)]
pub struct VlmPayload {
    pub pass: SamplingPass,
    pub frames: Vec<PayloadFrame>,
    /// Karelerle birlikte giden metin dizini.
    ///
    /// VLM'ler kare **sırasını** bilir ama saati bilmez; şartname ise puanı tam
    /// da zaman damgasından veriyor. Bu dizin, kareye basılan görsel damgayla
    /// birlikte ikinci bir tutamak sağlıyor.
    pub text_index: String,
    /// Modele gidecek talimatın tamamı.
    pub prompt: String,
    pub tokens: TokenEstimate,
    pub reduction: Reduction,
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

fn overview_prompt(count: usize, duration_ms: u64) -> String {
    format!(
        "Bu bir iş sağlığı ve güvenliği kamera kaydından alınmış {count} karedir. \
         Kayıt {} uzunluğunda ve kareler zaman sırasına göre veriliyor; her karenin \
         sol üst köşesinde kendi zaman damgası yazılı.\n\n\
         Önce sahnede genel olarak ne olduğunu belirle. Ardından riskli ya da \
         olağandışı bir durum olup olmadığını değerlendir.\n\n\
         Bir an dikkatini çekiyor ama kareler yetmiyorsa, o aralığı \
         `zoom_range(t0_ms, t1_ms)` ile isteyebilirsin; sana o aralıktan çok daha \
         sık kare verilecek. Bir bölgeye yakından bakman gerekiyorsa \
         `crop_region(t_ms, bbox)` kullan.",
        format_timestamp(duration_ms)
    )
}

fn zoom_prompt(count: usize, t0_ms: u64, t1_ms: u64) -> String {
    format!(
        "İstediğin {}–{} aralığından {count} kare. Kareler çok daha sık \
         örneklendi.\n\n\
         Bu aralıkta tam olarak ne olduğunu ve **kaçıncı saniyede** olduğunu \
         belirle. Zaman damgaları karelerin üzerinde yazılı.",
        format_timestamp(t0_ms),
        format_timestamp(t1_ms)
    )
}

/// Kare listesinden modele gidecek yükü kurar.
pub fn build(
    pass: SamplingPass,
    frames: &[FrameRef],
    info: &VideoInfo,
    max_dim: u32,
    range: Option<(u64, u64)>,
) -> VlmPayload {
    let payload_frames: Vec<PayloadFrame> = frames
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

    let text_index = payload_frames
        .iter()
        .map(|f| format!("Kare {} → {}", f.ord, f.time))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = match (pass, range) {
        (SamplingPass::Zoom, Some((t0, t1))) => zoom_prompt(payload_frames.len(), t0, t1),
        (SamplingPass::Zoom, None) => zoom_prompt(payload_frames.len(), 0, info.duration_ms),
        (SamplingPass::Overview, _) => overview_prompt(payload_frames.len(), info.duration_ms),
    };

    let (fw, fh) = scaled_dims(info, max_dim);
    let per_frame = estimate_frame_tokens(fw, fh);
    let frames_total = per_frame * payload_frames.len() as u32;
    let text = ((text_index.len() + prompt.len()) / CHARS_PER_TOKEN) as u32;

    let source_frames = (info.duration_ms as f64 / 1000.0 * info.fps).round() as u64;

    VlmPayload {
        pass,
        tokens: TokenEstimate {
            frame_width: fw,
            frame_height: fh,
            per_frame,
            frames_total,
            text,
            total: frames_total + text,
            note: "Qwen2-VL ailesi için tahmin: (G/28)×(Y/28). Model seçilince \
                   gerçek sayımla değiştirilmeli (#8).",
        },
        reduction: Reduction {
            source_frames,
            sent_frames: payload_frames.len(),
            ratio: source_frames as f64 / payload_frames.len().max(1) as f64,
            tokens_if_naive: source_frames * per_frame as u64,
        },
        frames: payload_frames,
        text_index,
        prompt,
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

    fn info() -> VideoInfo {
        VideoInfo {
            duration_ms: 56_500,
            fps: 30.0,
            width: 480,
            height: 480,
            size_bytes: 2_946_692,
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
        assert_eq!(scaled_dims(&i, 768), (480, 480), "küçük kare büyütülmemeli");

        i.width = 1920;
        i.height = 1080;
        let (w, h) = scaled_dims(&i, 768);
        assert_eq!(w, 768);
        assert_eq!(h, 432, "en-boy oranı korunmalı");
    }

    #[test]
    fn zaman_biciminde_salise_var() {
        // Aynı saniyeye düşen iki kare ayırt edilebilmeli.
        assert_eq!(format_timestamp_ms(15_200), "00:15.2");
        assert_eq!(format_timestamp_ms(15_800), "00:15.8");
        assert_ne!(format_timestamp_ms(15_200), format_timestamp_ms(15_800));
    }

    #[test]
    fn metin_dizini_sira_ve_zaman_tasir() {
        let p = build(SamplingPass::Overview, &frames(3), &info(), 768, None);

        assert_eq!(p.frames[0].ord, 1);
        assert_eq!(p.frames[2].ord, 3);
        assert!(p.text_index.contains("Kare 1 → 00:00.0"));
        assert!(p.text_index.contains("Kare 3 → 00:06.0"));
    }

    #[test]
    fn azaltma_orani_ham_videoyla_karsilastirir() {
        let p = build(SamplingPass::Overview, &frames(16), &info(), 768, None);

        // 56.5 sn * 30 fps ≈ 1695 kaynak kare.
        assert!(p.reduction.source_frames > 1600);
        assert_eq!(p.reduction.sent_frames, 16);
        assert!(p.reduction.ratio > 100.0);
        // Boru hattının varlık sebebi bu fark.
        assert!(p.reduction.tokens_if_naive > p.tokens.total as u64 * 50);
    }

    #[test]
    fn yakinlastirma_istemi_araligi_bildirir() {
        let p = build(
            SamplingPass::Zoom,
            &frames(12),
            &info(),
            768,
            Some((13_000, 19_000)),
        );

        assert!(p.prompt.contains("00:13"));
        assert!(p.prompt.contains("00:19"));
    }

    #[test]
    fn genel_bakis_istemi_araclari_tanitir() {
        let p = build(SamplingPass::Overview, &frames(16), &info(), 768, None);

        // Ajanın yakınlaşabileceğini bilmesi gerekiyor; yoksa aracı hiç çağırmaz.
        assert!(p.prompt.contains("zoom_range"));
        assert!(p.prompt.contains("crop_region"));
    }
}
