//! Hareket profilinin görselleştirilmesi.
//!
//! Eğriyi SVG olarak çizer. İki işe yarıyor:
//!
//! 1. **Hata ayıklama.** Sayı dizisine bakarak örneklemenin neden oraya
//!    baktığını anlamak zor; eğriye bakarak bir bakışta görülüyor.
//! 2. **Jüri sunumu.** "Sistem videoyu nasıl okudu" sorusunun görsel cevabı.
//!    Şartname açıklanabilirliği puanlıyor.
//!
//! Dışa bağımlılık yok, çıktı tek dosyalık SVG metni.

use std::fmt::Write as _;

use crate::motion::MotionProfile;

/// Renk paleti.
///
/// `apps/dashboard/src/app/globals.css` ile aynı değerler: SVG panele
/// gömüldüğünde tema tutarlı kalsın.
mod palette {
    pub const BACKGROUND: &str = "#0d0d0d";
    pub const FOREGROUND: &str = "#f5f5f5";
    pub const MUTED: &str = "#8a8a8a";
    pub const GRID: &str = "#292929";
    pub const CURVE: &str = "#2ea7ff";
    pub const SCENE_CUT: &str = "#b52bd6";
}

/// Çizim ayarları.
#[derive(Debug, Clone, Copy)]
pub struct ChartOptions {
    pub width: u32,
    pub height: u32,
    /// Sahne kesitlerini dikey çizgiyle işaretle.
    pub show_scene_cuts: bool,
}

impl Default for ChartOptions {
    fn default() -> Self {
        Self {
            width: 1200,
            height: 320,
            show_scene_cuts: true,
        }
    }
}

/// Zaman eksenindeki etiket aralığını süreye göre seçer.
///
/// Hedef ~10 etiket; insan gözünün beklediği yuvarlak sayılardan en yakını
/// alınır (1, 2, 5, 10, 15, 30 sn ve dakika katları).
fn tick_interval_ms(duration_ms: u64) -> u64 {
    const CANDIDATES: [u64; 10] = [
        1_000, 2_000, 5_000, 10_000, 15_000, 30_000, 60_000, 120_000, 300_000, 600_000,
    ];
    let target = duration_ms / 10;
    *CANDIDATES
        .iter()
        .find(|&&c| c >= target)
        .unwrap_or(CANDIDATES.last().unwrap())
}

fn format_time(t_ms: u64) -> String {
    let total = t_ms / 1000;
    format!("{:02}:{:02}", total / 60, total % 60)
}

/// Hareket profilini SVG olarak çizer.
pub fn motion_chart(profile: &MotionProfile, opts: ChartOptions) -> String {
    let w = opts.width as f64;
    let h = opts.height as f64;

    // Kenar boşlukları: solda skor ekseni, altta zaman ekseni, üstte başlık.
    let (ml, mr, mt, mb) = (48.0, 16.0, 40.0, 32.0);
    let plot_w = (w - ml - mr).max(1.0);
    let plot_h = (h - mt - mb).max(1.0);
    let base_y = mt + plot_h;

    let duration = profile.duration_ms.max(1) as f64;
    let x_of = |t_ms: u64| ml + (t_ms as f64 / duration) * plot_w;
    let y_of = |score: f32| base_y - (score as f64).clamp(0.0, 1.0) * plot_h;

    let mut svg = String::with_capacity(profile.len() * 24 + 2048);

    let _ = write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" font-family="system-ui, sans-serif">"#
    );
    let _ = write!(
        svg,
        r#"<rect width="{w}" height="{h}" fill="{bg}"/>"#,
        bg = palette::BACKGROUND
    );

    // --- Yatay ızgara ve skor ekseni ---
    for step in 0..=4 {
        let score = step as f32 / 4.0;
        let y = y_of(score);
        let _ = write!(
            svg,
            r#"<line x1="{ml:.1}" y1="{y:.1}" x2="{x2:.1}" y2="{y:.1}" stroke="{c}" stroke-width="1"/>"#,
            x2 = ml + plot_w,
            c = palette::GRID
        );
        let _ = write!(
            svg,
            r#"<text x="{x:.1}" y="{ty:.1}" fill="{c}" font-size="11" text-anchor="end">{score:.2}</text>"#,
            x = ml - 8.0,
            ty = y + 4.0,
            c = palette::MUTED
        );
    }

    // --- Zaman ekseni ---
    let interval = tick_interval_ms(profile.duration_ms);
    let mut t = 0u64;
    while t <= profile.duration_ms {
        let x = x_of(t);
        let _ = write!(
            svg,
            r#"<line x1="{x:.1}" y1="{mt:.1}" x2="{x:.1}" y2="{base_y:.1}" stroke="{c}" stroke-width="1"/>"#,
            c = palette::GRID
        );
        let _ = write!(
            svg,
            r#"<text x="{x:.1}" y="{ty:.1}" fill="{c}" font-size="11" text-anchor="middle">{label}</text>"#,
            ty = base_y + 18.0,
            c = palette::MUTED,
            label = format_time(t)
        );
        t += interval;
    }

    // --- Hareket eğrisi (dolgulu alan) ---
    if !profile.is_empty() {
        let first = &profile.samples[0];
        let last = &profile.samples[profile.len() - 1];

        let mut area = String::with_capacity(profile.len() * 20);
        let _ = write!(area, "M {:.1} {:.1}", x_of(first.t_ms), base_y);
        for s in &profile.samples {
            let _ = write!(area, " L {:.1} {:.1}", x_of(s.t_ms), y_of(s.score));
        }
        let _ = write!(area, " L {:.1} {:.1} Z", x_of(last.t_ms), base_y);

        let _ = write!(
            svg,
            r#"<path d="{area}" fill="{c}" fill-opacity="0.28"/>"#,
            c = palette::CURVE
        );

        let mut line = String::with_capacity(profile.len() * 20);
        for (i, s) in profile.samples.iter().enumerate() {
            let _ = write!(
                line,
                "{} {:.1} {:.1}",
                if i == 0 { "M" } else { "L" },
                x_of(s.t_ms),
                y_of(s.score)
            );
        }
        let _ = write!(
            svg,
            r#"<path d="{line}" fill="none" stroke="{c}" stroke-width="1.2"/>"#,
            c = palette::CURVE
        );
    }

    // --- Sahne kesitleri ---
    let cut_count = profile.scene_cuts().count();
    if opts.show_scene_cuts {
        for cut in profile.scene_cuts() {
            let x = x_of(cut.t_ms);
            let _ = write!(
                svg,
                r#"<line x1="{x:.1}" y1="{mt:.1}" x2="{x:.1}" y2="{base_y:.1}" stroke="{c}" stroke-width="1.5" stroke-dasharray="3 3"/>"#,
                c = palette::SCENE_CUT
            );
        }
    }

    // --- Başlık ---
    let _ = write!(
        svg,
        r#"<text x="{ml:.1}" y="24" fill="{c}" font-size="13">Hareket profili</text>"#,
        c = palette::FOREGROUND
    );
    let _ = write!(
        svg,
        r#"<text x="{x:.1}" y="24" fill="{c}" font-size="11" text-anchor="end">{n} örnek · {fps:.0} fps · ortalama {mean:.3} · tepe {peak:.3} · {cuts} sahne kesiti</text>"#,
        x = w - mr,
        c = palette::MUTED,
        n = profile.len(),
        fps = profile.analysis_fps,
        mean = profile.mean_score(),
        peak = profile.max_score(),
        cuts = cut_count
    );

    svg.push_str("</svg>");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::MotionSample;

    fn ornek_profil() -> MotionProfile {
        let samples = (0..30u32)
            .map(|i| MotionSample {
                index: i,
                t_ms: i as u64 * 100,
                score: if i == 10 { 1.0 } else { 0.1 },
                raw: 0.01,
                is_scene_cut: i == 10,
                dhash: 0,
                grid: Vec::new(),
                cell_peak: 0.0,
            })
            .collect();

        MotionProfile {
            analysis_fps: 10.0,
            width: 160,
            height: 90,
            duration_ms: 3000,
            samples,
        }
    }

    #[test]
    fn svg_gecerli_kok_etiketiyle_baslar_ve_biter() {
        let svg = motion_chart(&ornek_profil(), ChartOptions::default());
        assert!(svg.starts_with("<svg "));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains(r#"viewBox="0 0 1200 320""#));
    }

    #[test]
    fn sahne_kesiti_cizilir_ve_sayilir() {
        let profil = ornek_profil();

        let acik = motion_chart(&profil, ChartOptions::default());
        assert!(acik.contains("stroke-dasharray"), "kesit çizgisi yok");
        assert!(acik.contains("1 sahne kesiti"));

        let kapali = motion_chart(
            &profil,
            ChartOptions {
                show_scene_cuts: false,
                ..Default::default()
            },
        );
        assert!(!kapali.contains("stroke-dasharray"));
        // Çizim kapalı olsa da sayı başlıkta bildirilmeli.
        assert!(kapali.contains("1 sahne kesiti"));
    }

    #[test]
    fn bos_profil_cokmez() {
        let bos = MotionProfile {
            analysis_fps: 15.0,
            width: 160,
            height: 90,
            duration_ms: 0,
            samples: Vec::new(),
        };
        let svg = motion_chart(&bos, ChartOptions::default());
        assert!(svg.starts_with("<svg "));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn zaman_etiketi_araligi_sureye_gore_secilir() {
        assert_eq!(tick_interval_ms(10_000), 1_000);
        assert_eq!(tick_interval_ms(120_000), 15_000);
        assert_eq!(format_time(95_000), "01:35");
    }
}
