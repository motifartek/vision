//! Adaptif kare örnekleme: hareket ekseninde eşit aralıklı seçim.
//!
//! Pass 2. Hareket profilinden, verilen bütçe kadar kareyi **hareketin yoğun
//! olduğu yerlerden sık, sakin yerlerden seyrek** seçer.
//!
//! # Neden zaman ekseninde değil de hareket ekseninde
//!
//! "Saniyede bir kare al" sabit bir kuraldır: sakin bölümde israf, olay
//! anında yetersiz kalır. Bunun yerine hareket eğrisinin **kümülatif
//! toplamı** alınır ve N nokta bu eksende eşit aralıklarla yerleştirilir
//! (ters dönüşüm örneklemesi). Yoğunluk kendiliğinden ayarlanır ve **elle
//! hiçbir eşik seçilmez**.
//!
//! # Uniform prior (α) ve kapsama garantisi
//!
//! Saf hareket odaklı seçim tehlikelidir: hareketsiz bir bölge sıfır ağırlık
//! alır ve hiç örneklenmez. Oysa şartnamenin saydığı olaylardan biri
//! doğrudan "yerde **hareketsiz** kişi". Bu yüzden ağırlık, hareket dağılımı
//! ile düzgün dağılımın karışımıdır:
//!
//! ```text
//! w[i] = (1 - α) * m[i] / Σm  +  α * (1 / n)
//! ```
//!
//! α = 0 saf hareket, α = 1 saf düzgün tarama. Aradaki her değer ikisini
//! harmanlar — ve α tek başına **bir üst sınır garantisi** verir:
//!
//! Ardışık iki seçim arasında kümülatif ağırlık tam olarak `1/N` artar.
//! Düzgün bileşen bu aralığa en az `α * Δ/n` katkı verdiğinden `α * Δ/n ≤ 1/N`,
//! yani:
//!
//! ```text
//! en büyük boşluk ≤ (1 / α) × ortalama aralık
//! ```
//!
//! α = 0.25 için hiçbir boşluk ortalama aralığın 4 katını geçemez. Ayrı bir
//! `max_gap` parametresine gerek yok; α zaten kapsama düğmesidir.
//!
//! # Tekrar eleme
//!
//! Seçilen kareler süzülür: aynı karenin ikinci kopyası modele bilgi katmaz,
//! sadece bağlam yer. Eleme sonucu bütçenin altında kalmak **başarıdır**:
//! aynı bilgi daha az token ile taşınmıştır.
//!
//! Eleme için **iki koşul birden** aranır:
//!
//! 1. Görsel parmak izleri yakın (Hamming mesafesi eşiğin altında), **ve**
//! 2. İki kare arasında biriken hareket ihmal edilebilir.
//!
//! İkinci koşul zorunlu çıktı. Yalnız parmak izine bakan bir sürüm ölçüldü ve
//! zararlıydı: dHash kareyi 9x8'e indirgediği için, büyük ölçekli yerleşimi
//! benzeyen ama içeriği belirgin biçimde değişen kareler aynı sanıldı. Gerçek
//! test videosunda 14 karelik seçimin 9'u yanlışlıkla elendi ve olay
//! penceresinden tek kare kaldı.
//!
//! Hareket eğrisi bu soruyu doğrudan cevaplıyor: iki kare arasında hareket
//! birikmişse arada bir şey olmuştur, parmak izleri ne kadar benzerse benzesin.

use motif_core::{Error, Result};
use serde::{Deserialize, Serialize};

use crate::motion::{hamming_distance, MotionProfile};

/// İki kare arasında biriken hareketin, bir örnekleme adımına oranı bu değerin
/// altındaysa "arada bir şey olmadı" kabul edilir.
///
/// Mutlak değil göreli: ölçek videonun kendi toplam hareketi ve seçilen bütçe.
/// Golden dataset (#5) geldiğinde event coverage recall üzerinden ayarlanacak.
const DEDUP_MOTION_TOLERANCE: f64 = 0.1;

/// Örnekleme ayarları.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SamplingConfig {
    /// Kaç kare seçilecek. Donanım belli olmadığı için çalışma zamanı ayarı.
    pub budget: usize,
    /// Düzgün dağılımın ağırlığı, 0..1. Kapsama garantisini bu belirler.
    pub uniform_prior: f32,
    /// Bu Hamming mesafesinden yakın kareler tekrar sayılır. 0 = eleme kapalı.
    pub dedup_hamming: u32,
    /// Sahne kesitlerini seçime zorla dahil et.
    pub force_scene_cuts: bool,
    /// Gürültü tabanını (medyan hareket) ağırlıktan düş.
    ///
    /// Sensör gürültüsü sabit kameralı kayıtlarda her kareye benzer bir
    /// hareket skoru ekler. Taban düşülmezse bu skor toplamda gerçek olayı
    /// bastırır ve seçim düzgün dağılıma çöker — ölçüldü, bkz. modül dokümanı.
    pub subtract_noise_floor: bool,
}

impl Default for SamplingConfig {
    fn default() -> Self {
        Self {
            budget: 16,
            // 4 katlık boşluk garantisi: kapsama ile odaklanma arasında denge.
            uniform_prior: 0.25,
            dedup_hamming: 3,
            force_scene_cuts: true,
            subtract_noise_floor: true,
        }
    }
}

/// Bir karenin neden seçildiği.
///
/// Açıklanabilirlik için taşınıyor: şartname sistemin çıktılarının
/// gerekçelendirilebilir olmasını istiyor, ve hata ayıklarken "bu kare neden
/// burada" sorusunun cevabı gerekiyor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionReason {
    /// Hareket ekseninde örneklemeden geldi.
    Motion,
    /// Sahne kesiti olduğu için zorla eklendi.
    SceneCut,
}

/// Seçilmiş tek bir kare.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SelectedFrame {
    pub index: u32,
    pub t_ms: u64,
    pub motion_score: f32,
    pub is_scene_cut: bool,
    pub reason: SelectionReason,
}

/// Örnekleme sonucu ve istatistikleri.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Selection {
    pub frames: Vec<SelectedFrame>,
    /// İstenen bütçe. `frames.len()` bundan küçük olabilir (tekrar elendiyse).
    pub budget: usize,
    /// Tekrar olduğu için elenen kare sayısı.
    pub dropped_duplicates: usize,
    /// Ardışık seçimler arasındaki en büyük boşluk.
    pub max_gap_ms: u64,
    /// Ortalama boşluk.
    pub mean_gap_ms: u64,
}

impl Selection {
    pub fn timestamps(&self) -> Vec<u64> {
        self.frames.iter().map(|f| f.t_ms).collect()
    }

    pub fn scene_cut_count(&self) -> usize {
        self.frames
            .iter()
            .filter(|f| f.reason == SelectionReason::SceneCut)
            .count()
    }
}

/// Hareket profilinden kare seçer.
pub fn select_frames(profile: &MotionProfile, cfg: SamplingConfig) -> Result<Selection> {
    if profile.is_empty() {
        return Err(Error::InvalidVideo("hareket profili boş".into()));
    }
    if cfg.budget == 0 {
        return Err(Error::Config("kare bütçesi sıfır olamaz".into()));
    }
    if !(0.0..=1.0).contains(&cfg.uniform_prior) {
        return Err(Error::Config(
            "uniform_prior 0 ile 1 arasında olmalı".into(),
        ));
    }

    let n = profile.len();
    let alpha = cfg.uniform_prior as f64;

    // --- Gürültü tabanı ---
    //
    // Sabit kameralı kayıtta sensör gürültüsü her kareye benzer bir hareket
    // katar. Medyan tam olarak bu tabandır: karelerin yarısı altında, yarısı
    // üstünde. Düşülmezse taban toplamda olayı ezer — 17 sn'lik gürültülü
    // test videosunda ölçüldü: 14 sn sakin bölüm toplam hareketin üçte
    // ikisini üretiyor, 3 sn'lik olay üçte birini. Seçim de o oranda dağılıyor.
    //
    // Taban videonun kendi medyanı olduğu için bu hâlâ uyarlanabilir bir
    // ölçüt; sabit bir piksel eşiği değil.
    let noise_floor = if cfg.subtract_noise_floor {
        let mut scores: Vec<f64> = profile.samples.iter().map(|s| s.score as f64).collect();
        scores.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        crate::motion::median(&scores)
    } else {
        0.0
    };

    let effective: Vec<f64> = profile
        .samples
        .iter()
        .map(|s| (s.score as f64 - noise_floor).max(0.0))
        .collect();
    let total_motion: f64 = effective.iter().sum();

    // --- Ağırlıklar: hareket dağılımı ile düzgün dağılımın karışımı ---
    let weights: Vec<f64> = effective
        .iter()
        .map(|&m| {
            // Hiç hareket yoksa hareket bileşeni tanımsız; tamamen düzgüne düşülür.
            let motion_part = if total_motion > 0.0 {
                (1.0 - alpha) * (m / total_motion)
            } else {
                0.0
            };
            let uniform_part = alpha / n as f64;
            motion_part + uniform_part
        })
        .collect();

    // --- Kümülatif dağılım ---
    let mut cumulative = Vec::with_capacity(n);
    let mut running = 0.0;
    for w in &weights {
        running += w;
        cumulative.push(running);
    }
    let total_weight = running;

    // --- Ters dönüşüm örneklemesi ---
    //
    // N hedef nokta kümülatif eksende eşit aralıklarla yerleştirilir; her
    // biri eğriyi kestiği yerdeki kareye eşlenir. Hareketin yoğun olduğu
    // yerde eğri hızlı yükseldiği için oraya daha çok nokta düşer.
    let mut chosen: Vec<(u32, SelectionReason)> = Vec::with_capacity(cfg.budget);
    let mut seen = vec![false; n];

    for j in 0..cfg.budget {
        let target = (j as f64 + 0.5) / cfg.budget as f64 * total_weight;
        let idx = cumulative.partition_point(|&c| c < target).min(n - 1);
        if !seen[idx] {
            seen[idx] = true;
            chosen.push((idx as u32, SelectionReason::Motion));
        }
    }

    // --- Sahne kesitlerini zorla dahil et ---
    if cfg.force_scene_cuts {
        for (i, sample) in profile.samples.iter().enumerate() {
            if sample.is_scene_cut && !seen[i] {
                seen[i] = true;
                chosen.push((i as u32, SelectionReason::SceneCut));
            }
        }
    }

    chosen.sort_unstable_by_key(|(idx, _)| *idx);

    // --- Tekrar eleme ---
    //
    // Görsel olarak aynı karenin ikinci kopyası modele bilgi katmaz. Sahne
    // kesitleri elenmez: onlar sınır işaretleridir, benzer görünseler bile
    // taşıdıkları bilgi konumsaldır.
    //
    // Parmak izi tek başına yeterli değil (bkz. modül dokümanı); arada hareket
    // birikmemiş olması da şart. Bir örnekleme adımı, toplam hareketin
    // `1/budget`'ı kadar hareket demektir; bunun küçük bir kesrinden azı
    // "arada bir şey olmadı" sayılır.
    let step_motion = total_motion / cfg.budget as f64;
    let motion_tolerance = step_motion * DEDUP_MOTION_TOLERANCE;

    let mut frames: Vec<SelectedFrame> = Vec::with_capacity(chosen.len());
    let mut dropped = 0usize;

    for (idx, reason) in chosen {
        let sample = &profile.samples[idx as usize];

        let is_duplicate = cfg.dedup_hamming > 0
            && reason != SelectionReason::SceneCut
            && frames.iter().any(|kept: &SelectedFrame| {
                let kept_hash = profile.samples[kept.index as usize].dhash;
                if hamming_distance(kept_hash, sample.dhash) > cfg.dedup_hamming {
                    return false;
                }
                // İki kare arasında biriken hareket.
                let (a, b) = (kept.index.min(idx) as usize, kept.index.max(idx) as usize);
                let between: f64 = effective[a..=b].iter().sum();
                between <= motion_tolerance
            });

        if is_duplicate {
            dropped += 1;
            continue;
        }

        frames.push(SelectedFrame {
            index: sample.index,
            t_ms: sample.t_ms,
            motion_score: sample.score,
            is_scene_cut: sample.is_scene_cut,
            reason,
        });
    }

    // --- Boşluk istatistikleri ---
    let (max_gap_ms, mean_gap_ms) = if frames.len() < 2 {
        (0, 0)
    } else {
        let gaps: Vec<u64> = frames.windows(2).map(|w| w[1].t_ms - w[0].t_ms).collect();
        let max = gaps.iter().copied().max().unwrap_or(0);
        let mean = gaps.iter().sum::<u64>() / gaps.len() as u64;
        (max, mean)
    };

    Ok(Selection {
        frames,
        budget: cfg.budget,
        dropped_duplicates: dropped,
        max_gap_ms,
        mean_gap_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::MotionSample;

    /// Verilen skor dizisinden profil kurar (10 fps varsayımıyla).
    fn profil(scores: &[f32]) -> MotionProfile {
        let samples = scores
            .iter()
            .enumerate()
            .map(|(i, &score)| MotionSample {
                index: i as u32,
                t_ms: i as u64 * 100,
                score,
                raw: score * 0.1,
                is_scene_cut: false,
                // Her kare görsel olarak farklı olsun; tekrar elemesi tetiklenmesin.
                dhash: (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15),
            })
            .collect::<Vec<_>>();

        MotionProfile {
            analysis_fps: 10.0,
            width: 160,
            height: 90,
            duration_ms: scores.len() as u64 * 100,
            samples,
        }
    }

    #[test]
    fn hareketli_bolgeden_daha_cok_kare_secilir() {
        // İlk yarı sakin, ikinci yarı hareketli.
        let mut scores = vec![0.0f32; 50];
        scores.extend(std::iter::repeat_n(1.0f32, 50));
        let p = profil(&scores);

        let selection = select_frames(
            &p,
            SamplingConfig {
                budget: 20,
                uniform_prior: 0.25,
                dedup_hamming: 0,
                force_scene_cuts: false,
                subtract_noise_floor: false,
            },
        )
        .unwrap();

        let hareketli = selection.frames.iter().filter(|f| f.index >= 50).count();
        let sakin = selection.frames.len() - hareketli;
        assert!(
            hareketli > sakin,
            "hareketli bölge daha yoğun örneklenmeliydi: {hareketli} / {sakin}"
        );
    }

    #[test]
    fn sakin_bolge_yine_de_ornekleniyor() {
        // Kritik: "yerde hareketsiz kişi" senaryosu. Hareketsiz bölge
        // sıfır ağırlık alsa bile uniform prior sayesinde kare almalı.
        let mut scores = vec![0.0f32; 80];
        scores.extend(std::iter::repeat_n(1.0f32, 20));
        let p = profil(&scores);

        let selection = select_frames(
            &p,
            SamplingConfig {
                budget: 16,
                uniform_prior: 0.25,
                dedup_hamming: 0,
                force_scene_cuts: false,
                subtract_noise_floor: false,
            },
        )
        .unwrap();

        let sakin = selection.frames.iter().filter(|f| f.index < 80).count();
        assert!(
            sakin > 0,
            "tamamen hareketsiz bölgeden hiç kare seçilmedi; uniform prior çalışmıyor"
        );
    }

    #[test]
    fn alfa_sifirda_secim_tamamen_harekete_odaklanir() {
        let mut scores = vec![0.0f32; 90];
        scores.extend(std::iter::repeat_n(1.0f32, 10));
        let p = profil(&scores);

        let selection = select_frames(
            &p,
            SamplingConfig {
                budget: 10,
                uniform_prior: 0.0,
                dedup_hamming: 0,
                force_scene_cuts: false,
                subtract_noise_floor: false,
            },
        )
        .unwrap();

        assert!(
            selection.frames.iter().all(|f| f.index >= 90),
            "alfa=0 iken sakin bölgeden kare seçilmemeliydi"
        );
    }

    #[test]
    fn alfa_birde_secim_duzgun_dagilir() {
        let mut scores = vec![0.0f32; 90];
        scores.extend(std::iter::repeat_n(1.0f32, 10));
        let p = profil(&scores);

        let selection = select_frames(
            &p,
            SamplingConfig {
                budget: 10,
                uniform_prior: 1.0,
                dedup_hamming: 0,
                force_scene_cuts: false,
                subtract_noise_floor: false,
            },
        )
        .unwrap();

        // Düzgün dağılımda boşluklar birbirine çok yakın olmalı.
        assert!(
            selection.max_gap_ms <= selection.mean_gap_ms + 200,
            "alfa=1 iken aralıklar düzgün olmalıydı: max {} ort {}",
            selection.max_gap_ms,
            selection.mean_gap_ms
        );
    }

    #[test]
    fn bosluk_garantisi_alfadan_turer() {
        // Modül dokümanındaki iddia: en büyük boşluk <= (1/α) * ortalama aralık.
        // Örnekleme buna uyuyor mu, en zorlayıcı dağılımda sınanıyor:
        // ağırlığın tamamı tek bir noktada toplanmış.
        let mut scores = vec![0.0f32; 200];
        scores[100] = 1.0;
        let p = profil(&scores);

        for &alpha in &[0.25f32, 0.5, 0.75] {
            let budget = 16;
            let selection = select_frames(
                &p,
                SamplingConfig {
                    budget,
                    uniform_prior: alpha,
                    dedup_hamming: 0,
                    force_scene_cuts: false,
                    subtract_noise_floor: false,
                },
            )
            .unwrap();

            let ortalama_aralik = p.duration_ms as f64 / budget as f64;
            let sinir = ortalama_aralik / alpha as f64;

            assert!(
                (selection.max_gap_ms as f64) <= sinir * 1.05,
                "α={alpha}: boşluk {} ms, sınır {:.0} ms",
                selection.max_gap_ms,
                sinir
            );
        }
    }

    #[test]
    fn sahne_kesitleri_zorla_dahil_edilir() {
        let mut p = profil(&[0.5f32; 40]);
        p.samples[7].is_scene_cut = true;
        p.samples[33].is_scene_cut = true;

        let selection = select_frames(
            &p,
            SamplingConfig {
                budget: 4,
                uniform_prior: 0.5,
                dedup_hamming: 0,
                force_scene_cuts: true,
                subtract_noise_floor: false,
            },
        )
        .unwrap();

        let indeksler: Vec<u32> = selection.frames.iter().map(|f| f.index).collect();
        assert!(indeksler.contains(&7), "sahne kesiti seçime girmedi");
        assert!(indeksler.contains(&33), "sahne kesiti seçime girmedi");
        assert_eq!(selection.scene_cut_count(), 2);
    }

    #[test]
    fn hareketsiz_ve_ayni_gorunen_kareler_elenir() {
        // Hem parmak izleri aynı hem aralarında hiç hareket yok: gerçek tekrar.
        let mut p = profil(&[0.0f32; 50]);
        for sample in &mut p.samples {
            sample.dhash = 0xDEAD_BEEF;
        }

        let selection = select_frames(
            &p,
            SamplingConfig {
                budget: 16,
                uniform_prior: 0.5,
                dedup_hamming: 3,
                force_scene_cuts: false,
                subtract_noise_floor: false,
            },
        )
        .unwrap();

        assert_eq!(selection.frames.len(), 1, "tekrarlar elenmedi");
        assert!(selection.dropped_duplicates > 0);
        // Bütçenin altında kalmak beklenen davranış.
        assert!(selection.frames.len() < selection.budget);
    }

    #[test]
    fn parmak_izi_ayni_olsa_da_arada_hareket_varsa_elenmez() {
        // Bulunan hatanın regresyon testi: dHash kareyi 9x8'e indirgediği için
        // içeriği belirgin biçimde değişen kareler aynı parmak izini
        // taşıyabiliyor. Yalnız parmak izine bakan eleme, gerçek test
        // videosunda olay penceresindeki 9 kareyi silmişti.
        let mut p = profil(&[1.0f32; 50]);
        for sample in &mut p.samples {
            sample.dhash = 0xDEAD_BEEF;
        }

        let selection = select_frames(
            &p,
            SamplingConfig {
                budget: 10,
                uniform_prior: 0.5,
                dedup_hamming: 64, // her parmak izi "yakın" sayılsın
                force_scene_cuts: false,
                subtract_noise_floor: false,
            },
        )
        .unwrap();

        assert_eq!(
            selection.frames.len(),
            10,
            "arada hareket varken kareler elenmemeliydi"
        );
        assert_eq!(selection.dropped_duplicates, 0);
    }

    #[test]
    fn secim_zamana_gore_sirali_gelir() {
        let p = profil(&(0..60).map(|i| (i % 7) as f32 / 7.0).collect::<Vec<_>>());
        let selection = select_frames(&p, SamplingConfig::default()).unwrap();

        for w in selection.frames.windows(2) {
            assert!(w[1].t_ms > w[0].t_ms, "seçim zaman sırasında değil");
        }
    }

    #[test]
    fn gecersiz_ayarlar_hata_verir() {
        let p = profil(&[0.1, 0.2, 0.3]);

        assert!(select_frames(
            &p,
            SamplingConfig {
                budget: 0,
                ..Default::default()
            }
        )
        .is_err());

        assert!(select_frames(
            &p,
            SamplingConfig {
                uniform_prior: 1.5,
                ..Default::default()
            }
        )
        .is_err());
    }

    #[test]
    fn tamamen_hareketsiz_video_duzgun_taranir() {
        let p = profil(&[0.0f32; 100]);
        let selection = select_frames(
            &p,
            SamplingConfig {
                budget: 10,
                uniform_prior: 0.25,
                dedup_hamming: 0,
                force_scene_cuts: false,
                subtract_noise_floor: false,
            },
        )
        .unwrap();

        // Hareket sıfırken bölme yapılmamalı, seçim düzgün dağılmalı.
        assert_eq!(selection.frames.len(), 10);
        assert!(selection.max_gap_ms <= selection.mean_gap_ms + 200);
    }
}
