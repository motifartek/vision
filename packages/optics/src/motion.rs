//! Hareket profili: videonun "nerede bir şey oluyor" eğrisi.
//!
//! Pass 1'in çıktısı. Ardışık gri kareler arasındaki farkı ölçüp videonun
//! tamamı için tek boyutlu bir hareketlilik eğrisi üretir. GPU gerekmez,
//! ölçülen hız gerçek zamanın ~90 katı.
//!
//! # Neden kare farkı, neden optik akış değil
//!
//! Bize **ne kadar değişti** lazım, **hangi yöne** değil. Yön bilgisini VLM
//! zaten yorumluyor. Optik akış birkaç kat pahalıya, ürettiğinin çoğunu
//! attığımız bir bilgi veriyor.
//!
//! # Neden burada eşik yok
//!
//! Şartname §4 sabit kurallı çözümleri açıkça düşük puanlıyor. Bu modül
//! hiçbir yerde "şu değeri geçerse olay var" demiyor:
//!
//! - Normalizasyon videonun **kendi** 99. yüzdeliğine göre yapılıyor, sabit
//!   bir piksel eşiğine göre değil.
//! - Sahne kesiti, hareketin son bir saniyelik tabanına göre yaptığı
//!   **basamak** ile bulunuyor; ölçek yine videonun kendi tepe değeri.
//!
//! Modül **kanıt** üretir: nerede hareket var, hangi kareler birbirinin aynısı,
//! hangi anlar sahne değişimi. "Kaza oldu" kararını sadece model verir.

use std::path::Path;

use motif_core::{Error, Result};
use serde::{Deserialize, Serialize};

use crate::decode::decode_gray;
use crate::types::{AnalysisConfig, AnalysisFrame};

/// Sahne kesiti sayılmak için gereken basamak yüksekliği.
///
/// Videonun kendi tepe hareketinin bu oranı kadar bir sıçrama aranır. Mutlak
/// bir piksel eşiği değil; ölçüt videonun kendi dağılımı.
const SCENE_CUT_STEP_RATIO: f32 = 0.5;

/// Basamağın karşılaştırıldığı geçmiş pencerenin uzunluğu (saniye).
const SCENE_CUT_BASELINE_SECONDS: f64 = 1.0;

/// Gerçek bir sahne kesiti için gereken en küçük parmak izi mesafesi (64 bitte).
///
/// Hareket sıçraması tek başına yetmiyor. Gerçek bir İSG kaydında ölçüldü:
/// kepçe kadrajı süpürdüğünde SAD tavana vuruyor ama içerik perceptual olarak
/// aynı kalıyor. 56 saniyelik **tek çekimlik** kayıtta 12 sahte kesit üretildi
/// ve hepsinin parmak izi mesafesi 0-7 bit çıktı.
///
/// dHash mutlak bir piksel eşiği değil, istatistiksel bir büyüklük: birbiriyle
/// alakasız iki görüntü ortalama 32 bit (yarısı) farklıdır. 16 bit "içeriğin
/// dörtte biri değişti" demek ve gerçek bir kesit için makul bir alt sınır.
const SCENE_CUT_MIN_HASH_DISTANCE: u32 = 16;

/// Tek bir analiz karesinin hareket ölçümü.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MotionSample {
    /// Analiz akışındaki sıra numarası.
    pub index: u32,
    /// Karenin videodaki gerçek zamanı.
    pub t_ms: u64,
    /// Videonun kendi dağılımına göre normalize edilmiş hareket, 0..1.
    ///
    /// Örnekleme bunu kullanır: önemli olan bu videoda neyin hareketli
    /// sayıldığı, mutlak bir piksel farkı değil.
    pub score: f32,
    /// Mutlak ortalama piksel farkı, 0..1. Videolar arası karşılaştırılabilir.
    pub raw: f32,
    /// Bu kare bir sahne değişimi mi.
    pub is_scene_cut: bool,
    /// Görsel parmak izi. Tekrar eden kareleri elemek için.
    pub dhash: u64,
}

/// Videonun tamamının hareket profili.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionProfile {
    pub analysis_fps: f64,
    pub width: u32,
    pub height: u32,
    /// Profilin kapsadığı süre (son karenin zamanı + bir kare aralığı).
    pub duration_ms: u64,
    pub samples: Vec<MotionSample>,
}

impl MotionProfile {
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Eğrinin altındaki toplam alan. Faz 3'te örnekleme bunu böler.
    pub fn total_motion(&self) -> f64 {
        self.samples.iter().map(|s| s.score as f64).sum()
    }

    pub fn mean_score(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        (self.total_motion() / self.samples.len() as f64) as f32
    }

    pub fn max_score(&self) -> f32 {
        self.samples.iter().map(|s| s.score).fold(0.0, f32::max)
    }

    pub fn scene_cuts(&self) -> impl Iterator<Item = &MotionSample> {
        self.samples.iter().filter(|s| s.is_scene_cut)
    }

    /// Profilin verilen zaman aralığına düşen kesiti.
    ///
    /// Yakınlaştırma (pass 3) bunun üzerine kurulu: ajan bir aralık işaret
    /// ettiğinde, o aralığın profili kesilip aynı örnekleme algoritması daha
    /// küçük bir bütçeyle yeniden koşturulur. Videoyu tekrar çözmeye gerek
    /// yok — profil bir kez hesaplanıp saklandığı için yakınlaştırma
    /// neredeyse bedava.
    ///
    /// Skorlar **yeniden normalize edilmez**: aralık içindeki hareketin
    /// videonun geneline göre ne kadar güçlü olduğu bilgisi korunur. Yeniden
    /// normalize etmek, tamamen sakin bir aralıkta gürültüyü olay gibi
    /// gösterirdi.
    pub fn slice(&self, t0_ms: u64, t1_ms: u64) -> MotionProfile {
        let (start, end) = if t0_ms <= t1_ms {
            (t0_ms, t1_ms)
        } else {
            (t1_ms, t0_ms)
        };

        let samples: Vec<MotionSample> = self
            .samples
            .iter()
            .filter(|s| s.t_ms >= start && s.t_ms <= end)
            .copied()
            .collect();

        let duration_ms = match (samples.first(), samples.last()) {
            (Some(f), Some(l)) => {
                let frame_interval = (1000.0 / self.analysis_fps).round() as u64;
                l.t_ms - f.t_ms + frame_interval
            }
            _ => 0,
        };

        MotionProfile {
            analysis_fps: self.analysis_fps,
            width: self.width,
            height: self.height,
            duration_ms,
            samples,
        }
    }

    /// Profili daha kaba zaman kovalarına indirger.
    ///
    /// Ajana ham profili vermek bağlamı şişirir: 2 dakikalık videoda 1800
    /// örnek var. Kova başına **maksimum** alınır, ortalama değil — ortalama
    /// tek karelik bir sıçramayı silip götürür ve tam da o sıçrama aradığımız
    /// şeydir.
    pub fn bucketed(&self, bucket_ms: u64) -> Vec<(u64, f32, bool)> {
        if bucket_ms == 0 || self.samples.is_empty() {
            return Vec::new();
        }

        let mut out: Vec<(u64, f32, bool)> = Vec::new();
        for sample in &self.samples {
            let bucket_start = sample.t_ms / bucket_ms * bucket_ms;
            match out.last_mut() {
                Some((t, score, cut)) if *t == bucket_start => {
                    *score = score.max(sample.score);
                    *cut |= sample.is_scene_cut;
                }
                _ => out.push((bucket_start, sample.score, sample.is_scene_cut)),
            }
        }
        out
    }
}

/// İki gri kare arasındaki mutlak fark toplamı.
///
/// Piksel matematiğinin tamamı bu: iki diziyi çıkar, mutlak değerleri topla.
/// OpenCV'ye gerek duymamamızın sebebi.
fn sum_absolute_difference(a: &[u8], b: &[u8]) -> u64 {
    debug_assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| x.abs_diff(*y) as u64)
        .sum()
}

/// Fark hash'i: karenin 64 bitlik görsel parmak izi.
///
/// Kare 9x8'e indirgenir, her satırda yan yana piksel çiftleri karşılaştırılır.
/// Sonuç parlaklığa ve ölçeğe karşı dayanıklıdır; iki kare görsel olarak
/// aynıysa hash'ler de yakın olur (Hamming mesafesi küçük).
fn dhash(data: &[u8], width: u32, height: u32) -> u64 {
    const HW: u32 = 9;
    const HH: u32 = 8;

    if width == 0 || height == 0 {
        return 0;
    }

    // Kutu ortalamasıyla 9x8'e indirge.
    let mut small = [0u16; (HW * HH) as usize];
    for (sy, cell) in small.chunks_exact_mut(HW as usize).enumerate() {
        let y0 = sy as u32 * height / HH;
        let y1 = ((sy as u32 + 1) * height / HH).max(y0 + 1).min(height);
        for (sx, out) in cell.iter_mut().enumerate() {
            let x0 = sx as u32 * width / HW;
            let x1 = ((sx as u32 + 1) * width / HW).max(x0 + 1).min(width);

            let mut sum = 0u32;
            let mut count = 0u32;
            for y in y0..y1 {
                let row = (y * width) as usize;
                for x in x0..x1 {
                    sum += data[row + x as usize] as u32;
                    count += 1;
                }
            }
            *out = sum.checked_div(count).unwrap_or(0) as u16;
        }
    }

    // Satır içi komşu karşılaştırması -> 8x8 = 64 bit.
    let mut hash = 0u64;
    let mut bit = 0;
    for row in small.chunks_exact(HW as usize) {
        for x in 0..(HW - 1) as usize {
            if row[x] > row[x + 1] {
                hash |= 1 << bit;
            }
            bit += 1;
        }
    }
    hash
}

/// İki parmak izi arasındaki Hamming mesafesi.
///
/// 0 = birebir aynı. Pratikte < 5 "görsel olarak aynı kare" sayılır.
pub fn hamming_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Sıralanmış dilimin verilen yüzdeliği (lineer interpolasyonsuz).
pub(crate) fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

pub(crate) fn median(sorted: &[f64]) -> f64 {
    percentile(sorted, 0.5)
}

/// Çözülmüş kare akışından hareket profili üretir.
///
/// Akışı tüketirken aynı anda yalnızca iki kare bellekte tutulur; tepe bellek
/// video uzunluğundan bağımsız kalır.
pub fn analyze_frames<I>(frames: I, cfg: AnalysisConfig) -> Result<MotionProfile>
where
    I: Iterator<Item = Result<AnalysisFrame>>,
{
    let mut samples: Vec<MotionSample> = Vec::new();
    let mut previous: Option<Vec<u8>> = None;
    let pixel_count = cfg.frame_bytes() as f64;

    for frame in frames {
        let frame = frame?;

        let raw = match previous.as_deref() {
            // İlk karenin öncesi yok; farkı tanımsız, sıfır kabul ediliyor.
            None => 0.0,
            Some(prev) => sum_absolute_difference(prev, &frame.data) as f64 / (pixel_count * 255.0),
        };

        samples.push(MotionSample {
            index: frame.index,
            t_ms: frame.t_ms,
            score: 0.0, // ikinci geçişte doldurulacak
            raw: raw as f32,
            is_scene_cut: false,
            dhash: dhash(&frame.data, cfg.width, cfg.height),
        });

        previous = Some(frame.data);
    }

    if samples.is_empty() {
        return Err(Error::InvalidVideo("videodan hiç kare çözülemedi".into()));
    }

    // --- Normalizasyon ---
    //
    // Videonun kendi 99. yüzdeliğine bölünür. Maksimuma bölmek tek bir aykırı
    // sıçramanın (sahne kesiti, flaş) geri kalan her şeyi sıfıra ezmesine yol
    // açardı; 99. yüzdelik buna dayanıklı. Üstünü 1.0'a kırpıyoruz.
    let mut sorted: Vec<f64> = samples.iter().map(|s| s.raw as f64).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p99 = percentile(&sorted, 0.99);

    if p99 > 0.0 {
        for sample in &mut samples {
            sample.score = ((sample.raw as f64 / p99).min(1.0)) as f32;
        }
    }

    // --- Sahne kesiti tespiti ---
    //
    // Sahne kesiti **basamaktır**, yükseklik değil: hareketin son bir saniyelik
    // tabanına göre ani sıçraması. Bu ayrım pratikte kritik çıktı — önce mutlak
    // yüksekliğe bakan bir ölçüt denendi ve 3 saniyelik sürekli hareketli bir
    // bölümün her karesini kesit sanıp 22 kesit üretti. Oysa süregiden hareketin
    // ortasında kesit yoktur; kesit sadece başladığı ve bittiği yerdedir.
    //
    // Yön ayrımı yapılmıyor: hareketin başlaması kadar durması da içerik
    // değişimidir. "Yerde hareketsiz kişi" senaryosunda önemli an, tam da
    // hareketin durduğu andır.
    let max_score = samples.iter().map(|s| s.score).fold(0.0, f32::max);
    let step_threshold = max_score * SCENE_CUT_STEP_RATIO;
    let window = ((cfg.analysis_fps * SCENE_CUT_BASELINE_SECONDS).round() as usize).max(1);

    if step_threshold > 0.0 {
        // Aynı geçişin birkaç kare boyunca tekrar tekrar işaretlenmesini
        // önlemek için, bir kesitten sonra bir pencere boyunca susulur.
        //
        // Yarım pencere denendi ve yetmedi: taban medyanı geçişi yakalamakta
        // geciktiği için her sınır iki kez işaretleniyordu. Kesitler Faz 3'te
        // örneklemeye zorla dahil edildiğinden fazladan işaret doğrudan kare
        // bütçesinden yiyor. Bir saniyeden kısa aralıkla iki gerçek sahne
        // kesiti güvenlik kamerası kaydında beklenmez.
        let suppress_for = window.max(1);
        let mut last_cut: Option<usize> = None;

        for i in 0..samples.len() {
            let start = i.saturating_sub(window);
            if start == i {
                continue; // önünde taban yok
            }

            let mut baseline: Vec<f64> = samples[start..i].iter().map(|s| s.score as f64).collect();
            baseline.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let step = (samples[i].score as f64 - median(&baseline)).abs() as f32;

            if step <= step_threshold {
                continue;
            }
            if last_cut.is_some_and(|prev| i - prev < suppress_for) {
                continue;
            }

            // İkinci ve bağımsız koşul: içerik gerçekten değişmiş olmalı.
            //
            // Hareket sıçraması "bir şey oldu" der ama neyin olduğunu ayırt
            // etmez. Kadrajı süpüren büyük bir nesne de, sahnenin tamamen
            // değişmesi de aynı sıçramayı üretir. Parmak izi bu ikisini ayırır:
            // süpürme geçicidir, kesit kalıcıdır.
            if hamming_distance(samples[i - 1].dhash, samples[i].dhash)
                < SCENE_CUT_MIN_HASH_DISTANCE
            {
                continue;
            }

            samples[i].is_scene_cut = true;
            last_cut = Some(i);
        }
    }

    let last_t = samples.last().map(|s| s.t_ms).unwrap_or(0);
    let frame_interval_ms = (1000.0 / cfg.analysis_fps).round() as u64;

    Ok(MotionProfile {
        analysis_fps: cfg.analysis_fps,
        width: cfg.width,
        height: cfg.height,
        duration_ms: last_t + frame_interval_ms,
        samples,
    })
}

/// Video dosyasından doğrudan hareket profili üretir (çözme + analiz).
pub fn build_profile(path: &Path, cfg: AnalysisConfig) -> Result<MotionProfile> {
    let frames = decode_gray(path, cfg)?;
    analyze_frames(frames, cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(index: u32, fill: u8, cfg: AnalysisConfig) -> Result<AnalysisFrame> {
        Ok(AnalysisFrame {
            index,
            t_ms: cfg.timestamp_ms(index),
            data: vec![fill; cfg.frame_bytes()],
        })
    }

    fn tiny_cfg() -> AnalysisConfig {
        AnalysisConfig {
            analysis_fps: 10.0,
            width: 32,
            height: 32,
        }
    }

    /// Yapılı bir kare üretir.
    ///
    /// Düz renk kareler sahne kesiti sınamak için işe yaramaz: dHash komşu
    /// piksel karşılaştırmasına baktığı için düz bir karenin parmak izi her
    /// zaman sıfırdır ve iki düz kare, parlaklıkları ne kadar farklı olursa
    /// olsun, aynı görünür. Gerçek videoda böyle bir kare yok.
    ///
    /// `scene` deseni belirler — değişmesi **içerik değişimi** demek.
    /// `jitter` tüm piksellere sabit eklenir: SAD'yi (hareketi) yükseltir ama
    /// komşuluk sırasını bozmadığı için parmak izini değiştirmez. Gerçek
    /// hayattaki karşılığı tam olarak ölçtüğümüz durum: kadrajı süpüren büyük
    /// bir nesne hareketi tavana vurdurur, içerik ise aynı kalır.
    fn scene_frame(index: u32, scene: u32, jitter: u8, cfg: AnalysisConfig) -> Result<AnalysisFrame> {
        let mut data = vec![0u8; cfg.frame_bytes()];
        for y in 0..cfg.height {
            for x in 0..cfg.width {
                let desen = (x * (scene * 5 + 1) + y * (scene * 3 + 2)) % 200;
                data[(y * cfg.width + x) as usize] = (desen as u8).saturating_add(jitter);
            }
        }
        Ok(AnalysisFrame {
            index,
            t_ms: cfg.timestamp_ms(index),
            data,
        })
    }

    #[test]
    fn ayni_kareler_sifir_hareket_uretir() {
        let cfg = tiny_cfg();
        let frames = (0..5).map(|i| frame(i, 128, cfg));
        let profile = analyze_frames(frames, cfg).unwrap();

        assert_eq!(profile.len(), 5);
        assert!(profile.samples.iter().all(|s| s.raw == 0.0));
        assert_eq!(profile.max_score(), 0.0);
        // Hareket yoksa sahne kesiti de olmamalı.
        assert_eq!(profile.scene_cuts().count(), 0);
    }

    #[test]
    fn ilk_karenin_farki_sifir_kabul_edilir() {
        let cfg = tiny_cfg();
        let frames = vec![frame(0, 0, cfg), frame(1, 255, cfg)].into_iter();
        let profile = analyze_frames(frames, cfg).unwrap();

        assert_eq!(profile.samples[0].raw, 0.0);
        // Siyahtan beyaza tam geçiş: mutlak fark tavanda.
        assert!((profile.samples[1].raw - 1.0).abs() < 1e-6);
    }

    #[test]
    fn sahne_degisimi_kesit_olarak_isaretlenir() {
        let cfg = tiny_cfg();
        // Sahne 15. karede değişiyor ve değişmiş kalıyor: tek geçiş.
        let frames: Vec<_> = (0..30u32)
            .map(|i| scene_frame(i, if i < 15 { 1 } else { 7 }, 0, cfg))
            .collect();
        let profile = analyze_frames(frames.into_iter(), cfg).unwrap();

        let cuts: Vec<_> = profile.scene_cuts().collect();
        assert_eq!(cuts.len(), 1, "tek bir sahne kesiti bekleniyordu");
        assert_eq!(cuts[0].index, 15);
    }

    #[test]
    fn ayni_sahnedeki_buyuk_hareket_kesit_sayilmaz() {
        // Regresyon: gerçek bir İSG kaydında (56 sn, tek çekim, sıfır gerçek
        // kesit) kepçe kadrajı süpürdükçe hareket tavana vuruyor ve 12 sahte
        // kesit üretiliyordu. Hareket sıçraması tek başına kesit demek değil;
        // içeriğin de değişmesi gerekir.
        let cfg = tiny_cfg();
        let frames: Vec<_> = (0..40u32)
            .map(|i| {
                // Sahne hep aynı; yalnızca parlaklık dalgalanıyor, 25. karede
                // de sert bir sıçrama var. Yapı hiç değişmiyor.
                let jitter = if i == 25 { 200 } else { (i * 3 % 25) as u8 };
                scene_frame(i, 1, jitter, cfg)
            })
            .collect();
        let profile = analyze_frames(frames.into_iter(), cfg).unwrap();

        assert!(
            profile.max_score() > 0.5,
            "senaryo belirgin hareket içermeli, yoksa test bir şey sınamıyor"
        );
        assert_eq!(
            profile.scene_cuts().count(),
            0,
            "içerik değişmediği hâlde kesit işaretlendi"
        );
    }

    #[test]
    fn dhash_ayni_kareler_icin_ayni_farkli_kareler_icin_farkli() {
        let (w, h) = (32, 32);
        let düz = vec![100u8; (w * h) as usize];
        // Soldan sağa azalan gradyan: her komşu çift için sol > sağ.
        let azalan: Vec<u8> = (0..w * h)
            .map(|i| (255 - (i % w) * 255 / w) as u8)
            .collect();

        assert_eq!(dhash(&düz, w, h), dhash(&düz, w, h));
        assert_ne!(dhash(&düz, w, h), dhash(&azalan, w, h));

        assert_eq!(hamming_distance(0b1011, 0b1011), 0);
        assert_eq!(hamming_distance(0b1011, 0b1000), 2);
    }

    #[test]
    fn dhash_duz_kareyi_artan_gradyandan_ayirt_edemez() {
        // Bilinen sınır, gizlenmesin diye yazıldı: dHash değişimin *yönünü*
        // kodluyor. Düz karede komşu pikseller eşit, artan gradyanda sol hep
        // küçük — ikisinde de bütün bitler 0 çıkıyor.
        //
        // Pratikte sorun değil: hash yalnızca *aynı* kareleri elemek için
        // kullanılıyor, arama için değil. Kamera görüntüsü de dokuludur, düz
        // bir kare zaten hareketsiz demektir ve hareket eğrisi onu ayıklar.
        let (w, h) = (32, 32);
        let düz = vec![100u8; (w * h) as usize];
        let artan: Vec<u8> = (0..w * h).map(|i| ((i % w) * 255 / w) as u8).collect();

        assert_eq!(dhash(&düz, w, h), 0);
        assert_eq!(dhash(&artan, w, h), 0);
    }

    #[test]
    fn kovalama_maksimumu_korur() {
        let cfg = tiny_cfg(); // 10 fps -> kare başına 100 ms
        let mut frames = Vec::new();
        for i in 0..20u32 {
            let fill = if i == 3 { 255 } else { 10 };
            frames.push(frame(i, fill, cfg));
        }
        let profile = analyze_frames(frames.into_iter(), cfg).unwrap();

        // 1 sn'lik kovalar: ilk kova 0-900 ms arası kareleri kapsar ve
        // içindeki sıçrama (index 3, t=300ms) korunmalı.
        let buckets = profile.bucketed(1000);
        assert_eq!(buckets[0].0, 0);
        assert!(
            (buckets[0].1 - profile.max_score()).abs() < 1e-6,
            "kova maksimumu kaybetti"
        );
    }

    #[test]
    fn kesit_araligi_disini_atar_ve_skorlari_korur() {
        let cfg = tiny_cfg(); // 10 fps -> kare başına 100 ms
        let frames: Vec<_> = (0..30u32)
            .map(|i| scene_frame(i, if i < 15 { 1 } else { 7 }, 0, cfg))
            .collect();
        let profile = analyze_frames(frames.into_iter(), cfg).unwrap();

        let kesit = profile.slice(1000, 2000);

        assert!(kesit.samples.iter().all(|s| s.t_ms >= 1000 && s.t_ms <= 2000));
        assert_eq!(kesit.samples.first().unwrap().t_ms, 1000);
        assert_eq!(kesit.samples.last().unwrap().t_ms, 2000);

        // Sıçrama (index 15, t=1500) kesitte ve skoru değişmemiş olmalı.
        let sicrama = kesit.samples.iter().find(|s| s.t_ms == 1500).unwrap();
        assert_eq!(sicrama.score, profile.samples[15].score);
        assert!(sicrama.is_scene_cut);
    }

    #[test]
    fn ters_sirali_aralik_duzeltilir() {
        let cfg = tiny_cfg();
        let frames: Vec<_> = (0..20u32).map(|i| frame(i, 10, cfg)).collect();
        let profile = analyze_frames(frames.into_iter(), cfg).unwrap();

        let a = profile.slice(500, 1200);
        let b = profile.slice(1200, 500);
        assert_eq!(a.len(), b.len());
    }

    #[test]
    fn video_disi_aralik_bos_kesit_verir() {
        let cfg = tiny_cfg();
        let frames: Vec<_> = (0..10u32).map(|i| frame(i, 10, cfg)).collect();
        let profile = analyze_frames(frames.into_iter(), cfg).unwrap();

        let kesit = profile.slice(9_000, 10_000);
        assert!(kesit.is_empty());
        assert_eq!(kesit.duration_ms, 0);
    }

    #[test]
    fn bos_akis_hata_verir() {
        let cfg = tiny_cfg();
        let frames = std::iter::empty::<Result<AnalysisFrame>>();
        assert!(analyze_frames(frames, cfg).is_err());
    }
}
