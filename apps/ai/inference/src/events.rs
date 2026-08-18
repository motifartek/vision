//! Pencere başına sınıf skorlarını zaman damgalı olaylara çevirir.
//!
//! Histerezis eşikleme: bir sınıf `threshold`'u geçince olay başlar, ancak
//! `release` (daha düşük) eşiğinin altına inince biter. Tek eşikli yaklaşım
//! eşik civarında salınan skorlarda olayı onlarca parçaya bölerdi.

use serde::Serialize;

use crate::model::ced::NUM_CLASSES;
use crate::model::labels::ClassLabel;

#[derive(Debug, Clone, Serialize)]
pub struct AudioEvent {
    pub class_index: usize,
    pub label: String,
    pub label_tr: String,
    pub mid: String,
    pub start_sec: f32,
    pub end_sec: f32,
    pub peak_sec: f32,
    pub confidence: f32,
    pub mean_confidence: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClassSummary {
    pub class_index: usize,
    pub label: String,
    pub label_tr: String,
    pub total_sec: f32,
    pub event_count: usize,
    pub peak_confidence: f32,
}

#[derive(Debug, Clone)]
pub struct SegmentParams {
    pub threshold: f32,
    /// Histerezis çıkış eşiği; `threshold`'un altında olmalı.
    pub release: f32,
    pub min_duration_sec: f32,
    /// Bu kadar veya daha kısa boşlukla ayrılan aynı sınıf segmentleri birleşir.
    pub gap_sec: f32,
    pub max_events: usize,
    pub window_sec: f32,
    pub hop_sec: f32,
    pub duration_sec: f32,
}

/// Ham segment: pencere indeksleriyle tutulur, zamana en sonda çevrilir.
struct Run {
    start_w: usize,
    end_w: usize,
    peak_w: usize,
    peak: f32,
    sum: f32,
    count: usize,
}

impl Run {
    fn start_sec(&self, p: &SegmentParams) -> f32 {
        self.start_w as f32 * p.hop_sec
    }

    fn end_sec(&self, p: &SegmentParams) -> f32 {
        (self.end_w as f32 * p.hop_sec + p.window_sec).min(p.duration_sec)
    }
}

/// `scores` düzeni: `scores[window * NUM_CLASSES + class]`.
pub fn segment(
    scores: &[f32],
    n_windows: usize,
    labels: &[ClassLabel],
    p: &SegmentParams,
) -> (Vec<AudioEvent>, Vec<ClassSummary>) {
    let mut events = Vec::new();
    let mut summaries = Vec::new();

    for class in 0..NUM_CLASSES {
        let score = |w: usize| scores[w * NUM_CLASSES + class];

        // Ucuz ön eleme: sınıf hiç eşiği geçmiyorsa hiç uğraşma.
        if (0..n_windows).all(|w| score(w) < p.threshold) {
            continue;
        }

        let mut runs: Vec<Run> = Vec::new();
        let mut active: Option<Run> = None;

        for w in 0..n_windows {
            let s = score(w);
            match &mut active {
                Some(run) => {
                    if s >= p.release {
                        run.end_w = w;
                        run.sum += s;
                        run.count += 1;
                        if s > run.peak {
                            run.peak = s;
                            run.peak_w = w;
                        }
                    } else {
                        runs.push(active.take().expect("aktif segment var"));
                    }
                }
                None => {
                    if s >= p.threshold {
                        active = Some(Run {
                            start_w: w,
                            end_w: w,
                            peak_w: w,
                            peak: s,
                            sum: s,
                            count: 1,
                        });
                    }
                }
            }
        }
        if let Some(run) = active.take() {
            runs.push(run);
        }

        // Boşluk toleransı pencere sayısıyla ölçülür, zamanla değil: pencereler
        // örtüştüğü için (2 s pencere / 0.5 s adım) komşu segmentlerin zaman
        // aralıkları hep birbirine değer ve zaman tabanlı ölçüt her şeyi
        // birleştirirdi. Asıl soru "kaç ardışık pencere eşiğin altına düştü".
        let gap_windows = (p.gap_sec / p.hop_sec).round().max(0.0) as usize;

        let mut merged: Vec<Run> = Vec::with_capacity(runs.len());
        for run in runs {
            match merged.last_mut() {
                Some(prev) if run.start_w - prev.end_w - 1 <= gap_windows => {
                    prev.end_w = run.end_w;
                    prev.sum += run.sum;
                    prev.count += run.count;
                    if run.peak > prev.peak {
                        prev.peak = run.peak;
                        prev.peak_w = run.peak_w;
                    }
                }
                _ => merged.push(run),
            }
        }

        let label = &labels[class];
        let mut total_sec = 0.0;
        let mut event_count = 0;
        let mut peak_confidence = 0.0f32;

        for run in merged {
            let (start_sec, end_sec) = (run.start_sec(p), run.end_sec(p));
            if end_sec - start_sec < p.min_duration_sec {
                continue;
            }

            total_sec += end_sec - start_sec;
            event_count += 1;
            peak_confidence = peak_confidence.max(run.peak);

            events.push(AudioEvent {
                class_index: class,
                label: label.display_name.clone(),
                label_tr: label.display_name_tr().to_string(),
                mid: label.mid.clone(),
                start_sec,
                end_sec,
                // Tepe penceresinin ortası, olayın en belirgin anı olarak raporlanır.
                peak_sec: (run.peak_w as f32 * p.hop_sec + p.window_sec / 2.0)
                    .min(p.duration_sec),
                confidence: run.peak,
                mean_confidence: run.sum / run.count as f32,
            });
        }

        if event_count > 0 {
            summaries.push(ClassSummary {
                class_index: class,
                label: label.display_name.clone(),
                label_tr: label.display_name_tr().to_string(),
                total_sec,
                event_count,
                peak_confidence,
            });
        }
    }

    // En güvenilir olaylar korunacak şekilde kırp, sonra zamana göre sırala.
    if events.len() > p.max_events {
        events.sort_unstable_by(|a, b| b.confidence.total_cmp(&a.confidence));
        events.truncate(p.max_events);
    }
    events.sort_unstable_by(|a, b| {
        a.start_sec.total_cmp(&b.start_sec).then(b.confidence.total_cmp(&a.confidence))
    });

    summaries.sort_unstable_by(|a, b| b.total_sec.total_cmp(&a.total_sec));

    (events, summaries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels() -> Vec<ClassLabel> {
        (0..NUM_CLASSES)
            .map(|i| ClassLabel {
                index: i,
                mid: format!("/m/{i}"),
                display_name: format!("Class{i}"),
                display_name_tr: None,
                severity: None,
                category: None,
            })
            .collect()
    }

    fn params() -> SegmentParams {
        SegmentParams {
            threshold: 0.5,
            release: 0.3,
            min_duration_sec: 0.0,
            gap_sec: 0.0,
            max_events: 100,
            window_sec: 2.0,
            hop_sec: 1.0,
            duration_sec: 100.0,
        }
    }

    /// scores[w][class 0] = verilen dizi, diğer sınıflar 0.
    fn build(series: &[f32]) -> Vec<f32> {
        let mut v = vec![0.0; series.len() * NUM_CLASSES];
        for (w, &s) in series.iter().enumerate() {
            v[w * NUM_CLASSES] = s;
        }
        v
    }

    #[test]
    fn hysteresis_keeps_dipping_event_whole() {
        // 0.4 çıkış eşiğinin (0.3) üstünde kaldığı için olay bölünmemeli.
        let scores = build(&[0.0, 0.9, 0.4, 0.8, 0.0]);
        let (events, _) = segment(&scores, 5, &labels(), &params());
        assert_eq!(events.len(), 1, "{events:#?}");
        assert_eq!(events[0].start_sec, 1.0);
        assert_eq!(events[0].end_sec, 5.0);
        assert_eq!(events[0].confidence, 0.9);
    }

    #[test]
    fn below_release_splits_into_two_events() {
        let scores = build(&[0.9, 0.1, 0.9]);
        let (events, _) = segment(&scores, 3, &labels(), &params());
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn gap_tolerance_merges_neighbours() {
        let mut p = params();
        p.gap_sec = 2.0;
        let scores = build(&[0.9, 0.1, 0.9]);
        let (events, _) = segment(&scores, 3, &labels(), &p);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].start_sec, 0.0);
    }

    #[test]
    fn min_duration_filters_blips() {
        let mut p = params();
        p.min_duration_sec = 5.0;
        let scores = build(&[0.9, 0.0, 0.0]);
        let (events, _) = segment(&scores, 3, &labels(), &p);
        assert!(events.is_empty());
    }

    #[test]
    fn end_time_is_clamped_to_duration() {
        let mut p = params();
        p.duration_sec = 2.5;
        let scores = build(&[0.0, 0.9]);
        let (events, _) = segment(&scores, 2, &labels(), &p);
        assert_eq!(events[0].end_sec, 2.5);
    }

    #[test]
    fn summary_totals_match_events() {
        let scores = build(&[0.9, 0.0, 0.9]);
        let (events, summary) = segment(&scores, 3, &labels(), &params());
        assert_eq!(summary.len(), 1);
        assert_eq!(summary[0].event_count, events.len());
        let total: f32 = events.iter().map(|e| e.end_sec - e.start_sec).sum();
        assert!((summary[0].total_sec - total).abs() < 1e-5);
    }
}
