//! Veri kümesi: ground truth biçimi ve sentetik senaryo üretimi.
//!
//! # Neden sentetik
//!
//! Golden Dataset (#5) gerçek İSG videolarından oluşacak ve asıl ölçüt o.
//! Ama örnekleme algoritmasını ayarlamak için **kesin** ground truth gerekiyor
//! ve elle etiketleme hem yavaş hem hatalı. Sentetik senaryoları ffmpeg ile
//! biz kurduğumuz için olayların tam olarak kaçıncı milisaniyede olduğunu
//! biliyoruz — etiketleme hatası sıfır.
//!
//! Sentetik küme görsel karmaşıklığı taklit etmez; ettiğini de iddia etmiyor.
//! Ölçtüğü tek şey şu: **örnekleme, olayın olduğu ana kare ayırıyor mu?**
//! Bu soru modelden bağımsızdır ve tam da stream tarafının sorumluluğudur.
//!
//! Ground truth biçimi `motif_event_sdk::DetectedEvent` ile aynı; Golden
//! Dataset geldiğinde aynı harness hiç değişmeden çalışır.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use motif_event_sdk::{DetectedEvent, RiskLevel};
use serde::{Deserialize, Serialize};

/// Bir videonun bilinen olayları.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundTruth {
    /// Video dosyasının adı (ground truth dosyasıyla aynı dizinde).
    pub video: String,
    pub duration_ms: u64,
    /// Senaryonun ne sınadığı.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub events: Vec<DetectedEvent>,
}

impl GroundTruth {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("ground truth okunamadı: {}", path.display()))?;
        serde_json::from_str(&text)
            .with_context(|| format!("ground truth ayrıştırılamadı: {}", path.display()))
    }

    /// Video dosyasının tam yolu.
    pub fn video_path(&self, dataset_dir: &Path) -> PathBuf {
        dataset_dir.join(&self.video)
    }
}

/// Veri kümesindeki tüm ground truth dosyalarını yükler.
pub fn load_dataset(dir: &Path) -> Result<Vec<GroundTruth>> {
    if !dir.exists() {
        bail!(
            "veri kümesi dizini yok: {}\nÖnce `bench generate --out {}` çalıştırın.",
            dir.display(),
            dir.display()
        );
    }

    let mut items = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "json") {
            items.push(GroundTruth::load(&path)?);
        }
    }

    if items.is_empty() {
        bail!("veri kümesinde hiç ground truth dosyası yok: {}", dir.display());
    }

    items.sort_by(|a, b| a.video.cmp(&b.video));
    Ok(items)
}

// --- Sentetik senaryo üretimi ---

/// Bir segmentin görsel içeriği.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    /// Tamamen durağan sahne.
    Sabit,
    /// Sürekli ve belirgin hareket.
    Hareket,
}

impl Source {
    fn lavfi(self, duration_s: f64) -> String {
        match self {
            Source::Sabit => format!("color=c=gray:s=640x360:d={duration_s}:r=30"),
            Source::Hareket => format!("testsrc2=s=640x360:r=30:d={duration_s}"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Segment {
    source: Source,
    duration_s: f64,
}

const fn sabit(duration_s: f64) -> Segment {
    Segment {
        source: Source::Sabit,
        duration_s,
    }
}

const fn hareket(duration_s: f64) -> Segment {
    Segment {
        source: Source::Hareket,
        duration_s,
    }
}

/// Senaryonun nasıl kurulduğu.
enum Build {
    /// Ardışık segmentler birleştirilir; olaylar segment sınırlarından türer.
    Segments(&'static [Segment]),
    /// Geniş ve durağan bir sahnede küçük bir nesne kısa süre hareket eder.
    ///
    /// Kritik fark: bu olay **sahne kesiti üretmez**. Kare farkı yalnızca
    /// nesnenin kapladığı alanda değişir, sinyal zayıftır. Segment tabanlı
    /// senaryolarda olaylar tam da sahne kesitleriyle çakıştığı için, zorla
    /// dahil edilen kesitler kapsamayı garantiliyor ve ölçüt doyuyordu —
    /// yani hiçbir ayarı diğerinden ayırt edemiyordu. Bu senaryo o boşluğu
    /// kapatıyor ve gerçek CCTV'deki "geniş açının köşesinde biri düştü"
    /// durumuna karşılık geliyor.
    MovingObject {
        total_s: f64,
        /// Nesnenin kenar uzunluğu (piksel). Küçüldükçe zorlaşır.
        box_px: u32,
        appear_s: f64,
        visible_s: f64,
    },
}

struct Scenario {
    name: &'static str,
    notes: &'static str,
    build: Build,
    /// Sensör gürültüsü ekle (gerçek kamera kaydına yaklaştırmak için).
    noise: u32,
}

/// Sentetik senaryolar.
///
/// Şartnamenin ve mentör mailinin saydığı durum tiplerini karşılamaya
/// çalışıyor: net olay, hareketsiz kişi, çok kısa an, normal operasyon
/// (yanlış alarm kontrolü), çoklu olay, zorlu görsel koşul.
const SCENARIOS: &[Scenario] = &[
    Scenario {
        name: "net-olay",
        notes: "Sakin sahne, belirgin bir olay, tekrar sakin. Temel durum.",
        build: Build::Segments(&[sabit(8.0), hareket(3.0), sabit(6.0)]),
        noise: 12,
    },
    Scenario {
        name: "hareketsiz-kisi",
        notes: "Kısa hareket, ardından uzun hareketsizlik. Uniform prior'ın \
                asıl sınandığı senaryo: olay tam da hareketin bittiği andır.",
        build: Build::Segments(&[sabit(3.0), hareket(2.0), sabit(15.0)]),
        noise: 12,
    },
    Scenario {
        name: "cok-kisa-an",
        notes: "20 saniyelik sakin kayıtta yarım saniyelik olay.",
        build: Build::Segments(&[sabit(10.0), hareket(0.5), sabit(10.0)]),
        noise: 12,
    },
    Scenario {
        name: "normal-operasyon",
        notes: "Baştan sona sürekli hareket, olay yok. Yanlış alarm kontrolü: \
                sahne kesiti üretilmemeli.",
        build: Build::Segments(&[hareket(18.0)]),
        noise: 0,
    },
    Scenario {
        name: "coklu-olay",
        notes: "Farklı zamanlarda üç ayrı olay. Bütçenin olaylara dağılımı.",
        build: Build::Segments(&[
            sabit(4.0),
            hareket(1.0),
            sabit(5.0),
            hareket(2.0),
            sabit(4.0),
            hareket(1.0),
            sabit(3.0),
        ]),
        noise: 12,
    },
    Scenario {
        name: "agir-gurultu",
        notes: "Yoğun sensör gürültüsü altında tek kısa olay. Gürültü tabanı \
                düşürmenin etkisini ölçer.",
        build: Build::Segments(&[sabit(12.0), hareket(1.0), sabit(12.0)]),
        noise: 30,
    },
    Scenario {
        name: "kucuk-nesne-orta",
        notes: "640x360 sahnede 100 piksellik nesne 2 saniye hareket eder. \
                Sahne kesiti üretmeyecek kadar zayıf sinyal.",
        build: Build::MovingObject {
            total_s: 20.0,
            box_px: 100,
            appear_s: 9.0,
            visible_s: 2.0,
        },
        noise: 8,
    },
    Scenario {
        name: "kucuk-nesne-zor",
        notes: "Aynı sahnede 40 piksellik nesne 1 saniye hareket eder. Karenin \
                yüzde birinden azı değişiyor.",
        build: Build::MovingObject {
            total_s: 20.0,
            box_px: 40,
            appear_s: 12.0,
            visible_s: 1.0,
        },
        noise: 8,
    },
    // --- Bütçe kıtlığı ---
    //
    // Kısa videolarda 16 kare bol geliyor: düzgün dağılım bile her olayı
    // yakalıyor ve ölçüt doyuyor. Ayrım ancak bütçe kıtlaşınca ortaya çıkar.
    // İki dakikalık videoda 16 kare, ortalama 7.5 saniyelik aralık demek;
    // saniyelik bir olay düzgün dağılımda yapısal olarak kaçar, hareket
    // odaklı örneklemede yakalanır. Tasarımın değerini gösteren asıl test bu.
    Scenario {
        name: "uzun-tek-olay",
        notes: "İki dakikalık sakin kayıtta 1.5 saniyelik tek olay. \
                Samanlıkta iğne: bütçe kıtken hareket odaklılık şart.",
        build: Build::Segments(&[sabit(70.0), hareket(1.5), sabit(48.5)]),
        noise: 10,
    },
    Scenario {
        name: "uzun-iki-olay",
        notes: "İki dakikalık kayıtta birbirinden uzak iki kısa olay.",
        build: Build::Segments(&[
            sabit(25.0),
            hareket(1.0),
            sabit(60.0),
            hareket(1.5),
            sabit(32.5),
        ]),
        noise: 10,
    },
];

/// Segment sınırlarından ground truth olaylarını türetir.
///
/// Sabit -> Hareket geçişi olayın başlangıcı, Hareket -> Sabit bitişi.
/// Bitiş de olaydır: "yerde hareketsiz kişi" senaryosunda kritik an tam
/// olarak hareketin durduğu andır.
fn derive_events(segments: &[Segment]) -> Vec<DetectedEvent> {
    let mut events = Vec::new();
    let mut t_ms = 0u64;

    for pair in segments.windows(2) {
        t_ms += (pair[0].duration_s * 1000.0).round() as u64;
        let (from, to) = (pair[0].source, pair[1].source);
        if from == to {
            continue;
        }
        let (aciklama, severity) = match to {
            Source::Hareket => ("Hareket başladı", RiskLevel::Yuksek),
            Source::Sabit => ("Hareket durdu", RiskLevel::Orta),
        };
        events.push(DetectedEvent::new(t_ms, aciklama, severity));
    }

    events
}

/// Segment tabanlı senaryonun ffmpeg komutunu kurar.
fn segments_command(segments: &[Segment], noise: u32, cmd: &mut Command) {
    for segment in segments {
        cmd.args(["-f", "lavfi", "-i"])
            .arg(segment.source.lavfi(segment.duration_s));
    }

    // Gürültü her segmente ayrı ayrı uygulanır, sonra hepsi birleştirilir.
    let n = segments.len();
    let mut filter = String::new();
    let mut labels = String::new();

    for i in 0..n {
        if noise > 0 {
            filter.push_str(&format!("[{i}:v]noise=alls={noise}:allf=t[n{i}];"));
            labels.push_str(&format!("[n{i}]"));
        } else {
            labels.push_str(&format!("[{i}:v]"));
        }
    }
    filter.push_str(&format!("{labels}concat=n={n}:v=1[o]"));

    cmd.args(["-filter_complex", &filter]);
}

/// Hareketli küçük nesne senaryosunun ffmpeg komutunu kurar.
fn moving_object_command(
    total_s: f64,
    box_px: u32,
    appear_s: f64,
    visible_s: f64,
    noise: u32,
    cmd: &mut Command,
) {
    const W: u32 = 640;
    const H: u32 = 360;

    cmd.args(["-f", "lavfi", "-i"])
        .arg(format!("color=c=gray:s={W}x{H}:d={total_s}:r=30"));
    cmd.args(["-f", "lavfi", "-i"])
        .arg(format!("color=c=white:s={box_px}x{box_px}:d={total_s}:r=30"));

    let arka = if noise > 0 {
        format!("[0:v]noise=alls={noise}:allf=t[bg];")
    } else {
        "[0:v]null[bg];".to_string()
    };

    // Nesne yalnızca pencere içinde görünür; dışarıda kadraj dışına atılır.
    let bitis_s = appear_s + visible_s;
    let x_ifade = format!(
        "if(between(t,{appear_s},{bitis_s}),(t-{appear_s})/{visible_s}*{}-{box_px},-{})",
        W + box_px,
        box_px * 4
    );

    let filter = format!(
        "{arka}[bg][1:v]overlay=x='{x_ifade}':y={}[o]",
        H / 2 - box_px / 2
    );
    cmd.args(["-filter_complex", &filter]);
}

/// ffmpeg ile bir senaryoyu videoya dönüştürür.
fn render(scenario: &Scenario, out_dir: &Path) -> Result<GroundTruth> {
    let video_name = format!("{}.mp4", scenario.name);
    let video_path = out_dir.join(&video_name);

    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-y", "-v", "error"]);

    let (duration_ms, events) = match &scenario.build {
        Build::Segments(segments) => {
            segments_command(segments, scenario.noise, &mut cmd);
            let duration = segments
                .iter()
                .map(|s| (s.duration_s * 1000.0).round() as u64)
                .sum();
            (duration, derive_events(segments))
        }
        Build::MovingObject {
            total_s,
            box_px,
            appear_s,
            visible_s,
        } => {
            moving_object_command(
                *total_s,
                *box_px,
                *appear_s,
                *visible_s,
                scenario.noise,
                &mut cmd,
            );
            let appear_ms = (appear_s * 1000.0).round() as u64;
            let disappear_ms = ((appear_s + visible_s) * 1000.0).round() as u64;
            (
                (total_s * 1000.0).round() as u64,
                vec![
                    DetectedEvent::new(appear_ms, "Nesne göründü", RiskLevel::Yuksek),
                    DetectedEvent::new(disappear_ms, "Nesne kayboldu", RiskLevel::Orta),
                ],
            )
        }
    };

    cmd.args(["-map", "[o]", "-pix_fmt", "yuv420p"]);
    cmd.arg(&video_path);

    let output = cmd.output().context("ffmpeg çalıştırılamadı")?;
    if !output.status.success() {
        bail!(
            "{} üretilemedi: {}",
            scenario.name,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let truth = GroundTruth {
        video: video_name,
        duration_ms,
        notes: Some(
            scenario
                .notes
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
        ),
        events,
    };

    let json_path = out_dir.join(format!("{}.json", scenario.name));
    std::fs::write(&json_path, serde_json::to_vec_pretty(&truth)?)?;

    Ok(truth)
}

/// Sentetik veri kümesini üretir.
pub fn generate(out_dir: &Path) -> Result<Vec<GroundTruth>> {
    std::fs::create_dir_all(out_dir)?;

    let mut produced = Vec::new();
    for scenario in SCENARIOS {
        let truth = render(scenario, out_dir)?;
        println!(
            "  {:<18} {:>5.1} sn   {} olay",
            scenario.name,
            truth.duration_ms as f64 / 1000.0,
            truth.events.len()
        );
        produced.push(truth);
    }

    Ok(produced)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn olaylar_segment_sinirlarindan_turer() {
        let segments = [sabit(8.0), hareket(3.0), sabit(6.0)];
        let events = derive_events(&segments);

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].t_ms, 8_000);
        assert_eq!(events[0].event, "Hareket başladı");
        assert_eq!(events[1].t_ms, 11_000);
        assert_eq!(events[1].event, "Hareket durdu");
    }

    #[test]
    fn ayni_tipteki_ardisik_segmentler_olay_uretmez() {
        let segments = [sabit(2.0), sabit(3.0), hareket(1.0)];
        let events = derive_events(&segments);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].t_ms, 5_000);
    }

    #[test]
    fn olaysiz_senaryo_bos_liste_verir() {
        assert!(derive_events(&[hareket(18.0)]).is_empty());
    }

    #[test]
    fn zaman_damgasi_metni_ground_truthta_da_dogru() {
        let events = derive_events(&[sabit(95.0), hareket(1.0)]);
        assert_eq!(events[0].time, "01:35");
    }
}
