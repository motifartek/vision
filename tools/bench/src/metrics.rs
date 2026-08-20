//! Ölçütler.
//!
//! Şartname §4 katılımcıların kendi metriklerini tanımlamasını ve sonuçları
//! raporlamasını zorunlu tutuyor. Buradaki ana ölçüt **event coverage
//! recall**: ground truth olaylarının yüzde kaçının, izin verilen tolerans
//! içinde seçilmiş bir karesi var.
//!
//! Bu ölçütün değeri şurada: *"örnekleme mi kaçırdı, model mi anlamadı"*
//! sorusunu ayırıyor. Recall düşükse sorun stream tarafındadır ve modele hiç
//! dokunmadan düzeltilebilir; recall yüksek ama sonuç kötüyse sorun modeldedir.
//! İki başarısızlığı birbirine karıştırmak, 10 günde yanlış yeri optimize
//! etmenin en kolay yolu olurdu.

use std::path::Path;

use anyhow::{Context, Result};
use motif_optics::{build_profile, probe, select_frames, AnalysisConfig, SamplingConfig};
use serde::Serialize;

use crate::dataset::GroundTruth;

/// Tek bir video için ölçüm sonucu.
#[derive(Debug, Clone, Serialize)]
pub struct VideoMetrics {
    pub video: String,
    pub duration_ms: u64,

    // --- Doğruluk ---
    pub events_total: usize,
    pub events_covered: usize,
    /// Kapsanmayan olayların en yakın kareye uzaklığı (en kötüsü).
    pub worst_miss_ms: Option<u64>,
    /// Kapsanan olaylarda kare ile olay arasındaki ortalama uzaklık.
    pub mean_offset_ms: u64,

    // --- Bütçe ---
    pub selected_frames: usize,
    pub dropped_duplicates: usize,
    pub source_frames: u64,
    pub reduction_ratio: f64,

    // --- Kapsama ---
    pub max_gap_ms: u64,
    /// Tekrar elemesinden önceki en büyük boşluk. Garanti bunun için geçerli.
    pub max_gap_before_dedup_ms: u64,
    /// α'dan türeyen üst sınır. Aşılırsa garanti ihlal edilmiş demektir.
    pub gap_limit_ms: u64,

    // --- Yanlış alarm ---
    pub scene_cuts: usize,
    /// Hiçbir ground truth olayına denk gelmeyen sahne kesitleri.
    pub false_scene_cuts: usize,

    // --- Performans ---
    pub profile_ms: u128,
    pub total_ms: u128,
    pub realtime_factor: f64,
}

impl VideoMetrics {
    pub fn recall(&self) -> Option<f64> {
        (self.events_total > 0)
            .then(|| self.events_covered as f64 / self.events_total as f64)
    }

    pub fn gap_violated(&self) -> bool {
        // Garanti ham seçim için geçerli; eleme boşluğu zararsızca genişletebilir.
        // %5 pay: kare zamanları ayrık olduğu için sınıra tam oturmayabilir.
        self.max_gap_before_dedup_ms as f64 > self.gap_limit_ms as f64 * 1.05
    }
}

/// Bir videoyu baştan sona işleyip ölçer.
pub fn evaluate(
    truth: &GroundTruth,
    dataset_dir: &Path,
    analysis: AnalysisConfig,
    sampling: SamplingConfig,
    tolerance_ms: u64,
) -> Result<VideoMetrics> {
    let video = truth.video_path(dataset_dir);
    let info = probe(&video).with_context(|| format!("probe: {}", video.display()))?;

    let started = std::time::Instant::now();
    let profile =
        build_profile(&video, analysis).with_context(|| format!("profil: {}", video.display()))?;
    let profile_ms = started.elapsed().as_millis();

    let selection = select_frames(&profile, sampling)
        .with_context(|| format!("örnekleme: {}", video.display()))?;
    let total_ms = started.elapsed().as_millis();

    let timestamps = selection.timestamps();

    // --- Olay kapsama ---
    //
    // Bir olay, tolerans penceresi içinde en az bir seçilmiş kare varsa
    // kapsanmış sayılır. Kare olayın öncesinde de sonrasında da olabilir:
    // VLM'e giden bağlam açısından ikisi de olayı görünür kılar.
    let mut covered = 0usize;
    let mut worst_miss: Option<u64> = None;
    let mut offsets = Vec::new();

    for event in &truth.events {
        let nearest = timestamps
            .iter()
            .map(|&t| t.abs_diff(event.t_ms))
            .min()
            .unwrap_or(u64::MAX);

        if nearest <= tolerance_ms {
            covered += 1;
            offsets.push(nearest);
        } else {
            worst_miss = Some(worst_miss.map_or(nearest, |w: u64| w.max(nearest)));
        }
    }

    let mean_offset_ms = if offsets.is_empty() {
        0
    } else {
        offsets.iter().sum::<u64>() / offsets.len() as u64
    };

    // --- Yanlış sahne kesiti ---
    let false_scene_cuts = profile
        .scene_cuts()
        .filter(|cut| {
            truth
                .events
                .iter()
                .all(|e| cut.t_ms.abs_diff(e.t_ms) > tolerance_ms)
        })
        .count();

    let source_frames = (info.duration_ms as f64 / 1000.0 * info.fps).round() as u64;
    let selected = selection.frames.len().max(1);

    // α'dan türeyen boşluk garantisi: ortalama aralık / α.
    let gap_limit_ms = if sampling.uniform_prior > 0.0 {
        (profile.duration_ms as f64 / sampling.budget as f64 / sampling.uniform_prior as f64)
            .round() as u64
    } else {
        u64::MAX
    };

    Ok(VideoMetrics {
        video: truth.video.clone(),
        duration_ms: profile.duration_ms,
        events_total: truth.events.len(),
        events_covered: covered,
        worst_miss_ms: worst_miss,
        mean_offset_ms,
        selected_frames: selection.frames.len(),
        dropped_duplicates: selection.dropped_duplicates,
        source_frames,
        reduction_ratio: source_frames as f64 / selected as f64,
        max_gap_ms: selection.max_gap_ms,
        max_gap_before_dedup_ms: selection.max_gap_before_dedup_ms,
        gap_limit_ms,
        scene_cuts: profile.scene_cuts().count(),
        false_scene_cuts,
        profile_ms,
        total_ms,
        realtime_factor: profile.duration_ms as f64 / total_ms.max(1) as f64,
    })
}

/// Tüm veri kümesi için toplulaştırılmış sonuç.
#[derive(Debug, Clone, Serialize)]
pub struct Aggregate {
    pub videos: usize,
    pub events_total: usize,
    pub events_covered: usize,
    pub recall: f64,
    pub mean_frames: f64,
    pub mean_reduction: f64,
    pub mean_offset_ms: u64,
    pub false_scene_cuts: usize,
    pub gap_violations: usize,
    pub mean_realtime: f64,
    pub total_ms: u128,
}

impl Aggregate {
    pub fn from(results: &[VideoMetrics]) -> Self {
        let videos = results.len().max(1);
        let events_total: usize = results.iter().map(|r| r.events_total).sum();
        let events_covered: usize = results.iter().map(|r| r.events_covered).sum();

        let kapsanan: Vec<u64> = results
            .iter()
            .filter(|r| r.events_covered > 0)
            .map(|r| r.mean_offset_ms)
            .collect();

        Self {
            videos: results.len(),
            events_total,
            events_covered,
            recall: if events_total > 0 {
                events_covered as f64 / events_total as f64
            } else {
                1.0
            },
            mean_frames: results.iter().map(|r| r.selected_frames as f64).sum::<f64>()
                / videos as f64,
            mean_reduction: results.iter().map(|r| r.reduction_ratio).sum::<f64>() / videos as f64,
            mean_offset_ms: if kapsanan.is_empty() {
                0
            } else {
                kapsanan.iter().sum::<u64>() / kapsanan.len() as u64
            },
            false_scene_cuts: results.iter().map(|r| r.false_scene_cuts).sum(),
            gap_violations: results.iter().filter(|r| r.gap_violated()).count(),
            mean_realtime: results.iter().map(|r| r.realtime_factor).sum::<f64>() / videos as f64,
            total_ms: results.iter().map(|r| r.total_ms).sum(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrik(events_total: usize, events_covered: usize, max_gap: u64, limit: u64) -> VideoMetrics {
        VideoMetrics {
            video: "t.mp4".into(),
            duration_ms: 10_000,
            events_total,
            events_covered,
            worst_miss_ms: None,
            mean_offset_ms: 100,
            selected_frames: 10,
            dropped_duplicates: 0,
            source_frames: 300,
            reduction_ratio: 30.0,
            max_gap_ms: max_gap,
            max_gap_before_dedup_ms: max_gap,
            gap_limit_ms: limit,
            scene_cuts: 2,
            false_scene_cuts: 0,
            profile_ms: 50,
            total_ms: 60,
            realtime_factor: 100.0,
        }
    }

    #[test]
    fn olaysiz_video_recall_bildirmez() {
        assert_eq!(metrik(0, 0, 0, 1000).recall(), None);
    }

    #[test]
    fn recall_oran_olarak_hesaplanir() {
        assert_eq!(metrik(4, 3, 0, 1000).recall(), Some(0.75));
    }

    #[test]
    fn bosluk_ihlali_pay_biraktiktan_sonra_bildirilir() {
        // Sınırın hemen altı ve az üstü ihlal sayılmamalı.
        assert!(!metrik(1, 1, 1000, 1000).gap_violated());
        assert!(!metrik(1, 1, 1040, 1000).gap_violated());
        assert!(metrik(1, 1, 1200, 1000).gap_violated());
    }

    #[test]
    fn toplulastirma_olaysiz_videolari_recalla_katmaz() {
        let sonuclar = vec![metrik(2, 2, 0, 1000), metrik(0, 0, 0, 1000)];
        let agg = Aggregate::from(&sonuclar);

        assert_eq!(agg.videos, 2);
        assert_eq!(agg.events_total, 2);
        assert_eq!(agg.recall, 1.0);
    }
}
