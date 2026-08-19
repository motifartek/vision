//! İş güvenliği katmanı: ses olaylarını güvenlik bulgularına çevirir.
//!
//! Yeni model yok — bu modül `events.rs`'in ürettiği olay listesi üzerinde
//! kural işletir. Kurallar **ihlal kararı vermez**, "şu ana bak" der: tek
//! mikrofonda yön bilgisi olmadığı için sistem kimin nerede durduğunu bilemez.
//!
//! Doğrulama durumu (bkz. bu dosyadaki `#[cfg(test)]` bloğu):
//! - Kural mantığı birim testlerle doğrulandı (deterministik, model gerekmez).
//! - Alarm tespiti sentezlenmiş test videosuyla uçtan uca doğrulandı.
//! - "Geri vites uyarı sesi" sınıfının sahada ne kadar güvenilir tetiklendiği
//!   **doğrulanmadı** — elimizde gerçek forklift kaydı yok.

use serde::Serialize;

use crate::events::AudioEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Anında incelenmeli: alarm, çığlık, patlama.
    Critical,
    /// Riskli durum işareti: darbe, araç, uyarı sesi.
    Warning,
    /// Bağlam bilgisi: makine ve el aleti sesleri.
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Alarm,
    Distress,
    Impact,
    Vehicle,
    Machine,
    Human,
}

pub struct SafetyClass {
    /// AudioSet'in İngilizce adı — sürümler arası en kararlı anahtar.
    pub en: &'static str,
    pub category: Category,
    pub severity: Severity,
}

const fn sc(en: &'static str, category: Category, severity: Severity) -> SafetyClass {
    SafetyClass { en, category, severity }
}

/// 527 sınıfın iş güvenliğiyle ilgili alt kümesi.
pub const SAFETY_CLASSES: &[SafetyClass] = &[
    // — Alarm ve uyarı —
    sc("Fire alarm", Category::Alarm, Severity::Critical),
    sc("Smoke detector, smoke alarm", Category::Alarm, Severity::Critical),
    sc("Siren", Category::Alarm, Severity::Critical),
    sc("Civil defense siren", Category::Alarm, Severity::Critical),
    sc("Alarm", Category::Alarm, Severity::Critical),
    sc("Buzzer", Category::Alarm, Severity::Warning),
    sc("Whistle", Category::Alarm, Severity::Warning),
    sc("Foghorn", Category::Alarm, Severity::Warning),
    sc("Steam whistle", Category::Alarm, Severity::Warning),
    // — İnsan tehlike sinyali —
    sc("Screaming", Category::Distress, Severity::Critical),
    sc("Shout", Category::Distress, Severity::Critical),
    sc("Yell", Category::Distress, Severity::Critical),
    sc("Bellow", Category::Distress, Severity::Critical),
    sc("Children shouting", Category::Distress, Severity::Critical),
    sc("Wail, moan", Category::Distress, Severity::Critical),
    sc("Groan", Category::Distress, Severity::Warning),
    sc("Gasp", Category::Distress, Severity::Warning),
    sc("Crying, sobbing", Category::Distress, Severity::Warning),
    // — Darbe ve kaza —
    sc("Explosion", Category::Impact, Severity::Critical),
    sc("Smash, crash", Category::Impact, Severity::Critical),
    sc("Shatter", Category::Impact, Severity::Critical),
    sc("Breaking", Category::Impact, Severity::Critical),
    sc("Bang", Category::Impact, Severity::Warning),
    sc("Thump, thud", Category::Impact, Severity::Warning),
    sc("Boom", Category::Impact, Severity::Warning),
    sc("Burst, pop", Category::Impact, Severity::Warning),
    sc("Crushing", Category::Impact, Severity::Warning),
    sc("Crack", Category::Impact, Severity::Warning),
    sc("Glass", Category::Impact, Severity::Warning),
    sc("Slam", Category::Impact, Severity::Warning),
    // — Araç —
    sc("Reversing beeps", Category::Vehicle, Severity::Warning),
    sc("Air horn, truck horn", Category::Vehicle, Severity::Warning),
    sc("Vehicle horn, car horn, honking", Category::Vehicle, Severity::Warning),
    sc("Truck", Category::Vehicle, Severity::Warning),
    sc("Air brake", Category::Vehicle, Severity::Warning),
    sc("Motor vehicle (road)", Category::Vehicle, Severity::Warning),
    sc("Vehicle", Category::Vehicle, Severity::Warning),
    sc("Bus", Category::Vehicle, Severity::Warning),
    sc("Motorcycle", Category::Vehicle, Severity::Warning),
    // — Makine ve el aleti (bağlam) —
    sc("Jackhammer", Category::Machine, Severity::Info),
    sc("Chainsaw", Category::Machine, Severity::Info),
    sc("Drill", Category::Machine, Severity::Info),
    sc("Power tool", Category::Machine, Severity::Info),
    sc("Sawing", Category::Machine, Severity::Info),
    sc("Sanding", Category::Machine, Severity::Info),
    sc("Hammer", Category::Machine, Severity::Info),
    sc("Filing (rasp)", Category::Machine, Severity::Info),
    sc("Mechanisms", Category::Machine, Severity::Info),
    sc("Gears", Category::Machine, Severity::Info),
    sc("Lawn mower", Category::Machine, Severity::Info),
    sc("Engine", Category::Machine, Severity::Info),
    // — İnsan varlığı (birlikte oluş kuralı için) —
    sc("Speech", Category::Human, Severity::Info),
    sc("Male speech, man speaking", Category::Human, Severity::Info),
    sc("Female speech, woman speaking", Category::Human, Severity::Info),
    sc("Conversation", Category::Human, Severity::Info),
    sc("Walk, footsteps", Category::Human, Severity::Info),
    sc("Laughter", Category::Human, Severity::Info),
];

pub fn lookup(en: &str) -> Option<&'static SafetyClass> {
    SAFETY_CLASSES.iter().find(|c| c.en == en)
}

/// Güvenlik sınıfına düşen bir olay, kategorisi ve önemiyle birlikte.
#[derive(Debug, Clone, Serialize)]
pub struct SafetyEvent {
    pub label: String,
    pub label_tr: String,
    pub category: Category,
    pub severity: Severity,
    pub start_sec: f32,
    pub end_sec: f32,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    /// Kural kimliği: "post_alarm_activity" | "vehicle_near_person" | "critical_sound"
    pub rule: &'static str,
    pub severity: Severity,
    pub start_sec: f32,
    pub end_sec: f32,
    pub title: String,
    pub detail: String,
    /// Kuralı tetikleyen olaylar — bulgunun kanıtı.
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SafetyReport {
    pub events: Vec<SafetyEvent>,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone)]
pub struct SafetyParams {
    /// Alarmdan sonra faaliyet aranacak süre.
    pub post_alarm_window_sec: f32,
    /// Araç–insan birlikteliğinde kabul edilen zaman örtüşmesi.
    pub cooccurrence_tolerance_sec: f32,
}

impl Default for SafetyParams {
    fn default() -> Self {
        Self { post_alarm_window_sec: 120.0, cooccurrence_tolerance_sec: 0.0 }
    }
}

/// Bulgu metinlerindeki zaman damgası.
///
/// Bir saati aşınca saat alanı açılır: güvenlik kamerası kayıtları saatler
/// sürüyor ve `mm:ss` biçimi orada `75:30` gibi okunmaz değerler veriyordu.
fn ts(sec: f32) -> String {
    let total = sec.max(0.0) as u32;
    let (hours, minutes, seconds) = (total / 3600, (total % 3600) / 60, total % 60);

    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

fn overlaps(a: &AudioEvent, b: &AudioEvent, tol: f32) -> bool {
    a.start_sec - tol < b.end_sec && b.start_sec - tol < a.end_sec
}

/// Olay listesinden güvenlik olaylarını ve kural bulgularını çıkarır.
///
/// `events` **başlangıç zamanına göre sıralı** gelmeli (`events::segment`
/// öyle döndürüyor); `vehicle_near_person` bu sıraya güvenerek erken çıkıyor.
/// Ayrıca liste `max_events` kırpmasından **önceki** tam liste olmalı, yoksa
/// düşük güvenli bir alarm bulgusuyla birlikte kaybolur.
pub fn analyze(events: &[AudioEvent], params: &SafetyParams) -> SafetyReport {
    let mut safety_events: Vec<SafetyEvent> = events
        .iter()
        .filter_map(|e| {
            lookup(&e.label).map(|c| SafetyEvent {
                label: e.label.clone(),
                label_tr: e.label_tr.clone(),
                category: c.category,
                severity: c.severity,
                start_sec: e.start_sec,
                end_sec: e.end_sec,
                confidence: e.confidence,
            })
        })
        .collect();
    safety_events.sort_by(|a, b| a.start_sec.total_cmp(&b.start_sec));

    let mut findings = Vec::new();
    findings.extend(critical_sounds(events));
    findings.extend(post_alarm_activity(events, params));
    findings.extend(vehicle_near_person(events, params));
    findings.sort_by(|a, b| a.start_sec.total_cmp(&b.start_sec));

    SafetyReport { events: safety_events, findings }
}

/// Kural 1 — kritik önemdeki sesler.
///
/// Aynı fiziksel olay birden çok sınıfa düşebilir: bir duman alarmı hem
/// "Duman dedektörü" hem "Yangın alarmı" olarak algılanır. Bunları ayrı ayrı
/// raporlamak aynı olayı iki kez saymak olur; bu yüzden **zamanda örtüşen ve
/// aynı kategoriye düşen** kritik sesler tek bulguda birleştirilir.
fn critical_sounds(events: &[AudioEvent]) -> Vec<Finding> {
    let mut critical: Vec<(&AudioEvent, &'static SafetyClass)> = events
        .iter()
        .filter_map(|e| lookup(&e.label).map(|c| (e, c)))
        .filter(|(_, c)| c.severity == Severity::Critical)
        .collect();
    critical.sort_by(|a, b| a.0.start_sec.total_cmp(&b.0.start_sec));

    // Örtüşen ve aynı kategorideki olayları kümele
    let mut groups: Vec<Vec<(&AudioEvent, &'static SafetyClass)>> = Vec::new();
    for item in critical {
        let joined = groups.iter_mut().find(|g| {
            g[0].1.category == item.1.category
                && g.iter().any(|(e, _)| overlaps(e, item.0, 0.0))
        });
        match joined {
            Some(g) => g.push(item),
            None => groups.push(vec![item]),
        }
    }

    groups
        .into_iter()
        .map(|g| {
            let start = g.iter().map(|(e, _)| e.start_sec).fold(f32::MAX, f32::min);
            let end = g.iter().map(|(e, _)| e.end_sec).fold(0.0f32, f32::max);
            // En güvenli etiket başlığı belirlesin
            let best = g
                .iter()
                .max_by(|a, b| a.0.confidence.total_cmp(&b.0.confidence))
                .expect("küme boş olamaz")
                .0;
            let others: Vec<String> = g
                .iter()
                .filter(|(e, _)| e.label != best.label)
                .map(|(e, _)| format!("{} %{}", e.label_tr, (e.confidence * 100.0).round()))
                .collect();

            let detail = if others.is_empty() {
                format!(
                    "{}–{} arasında {} sesi %{} güvenle algılandı.",
                    ts(start),
                    ts(end),
                    best.label_tr,
                    (best.confidence * 100.0).round()
                )
            } else {
                format!(
                    "{}–{} arasında {} sesi %{} güvenle algılandı. Aynı ses şu sınıflara da düştü: {}.",
                    ts(start),
                    ts(end),
                    best.label_tr,
                    (best.confidence * 100.0).round(),
                    others.join(", ")
                )
            };

            Finding {
                rule: "critical_sound",
                severity: Severity::Critical,
                start_sec: start,
                end_sec: end,
                title: format!("{} tespit edildi", best.label_tr),
                detail,
                evidence: g
                    .iter()
                    .map(|(e, _)| format!("{} {}–{}", e.label_tr, ts(e.start_sec), ts(e.end_sec)))
                    .collect(),
            }
        })
        .collect()
}

/// Kural 2 — alarm çaldıktan sonra makine/el aleti sesi sürüyor mu?
///
/// Alarm sırasında iş durmalıdır. Sürmesi ya duyulmadığını (kulak koruyucu +
/// gürültülü makine) ya da yok sayıldığını gösterir; ikisi de bulgudur.
fn post_alarm_activity(events: &[AudioEvent], params: &SafetyParams) -> Vec<Finding> {
    let alarms: Vec<&AudioEvent> = events
        .iter()
        .filter(|e| {
            lookup(&e.label)
                .is_some_and(|c| c.category == Category::Alarm && c.severity == Severity::Critical)
        })
        .collect();
    if alarms.is_empty() {
        return Vec::new();
    }

    // Ardışık alarm parçalarını tek olaya indir; yoksa her parça için bulgu üretir.
    let mut start = alarms[0].start_sec;
    let mut end = alarms[0].end_sec;
    let mut spans: Vec<(f32, f32)> = Vec::new();
    for a in alarms.iter().skip(1) {
        if a.start_sec <= end + 5.0 {
            end = end.max(a.end_sec);
        } else {
            spans.push((start, end));
            start = a.start_sec;
            end = a.end_sec;
        }
    }
    spans.push((start, end));

    spans
        .into_iter()
        .filter_map(|(alarm_start, alarm_end)| {
            let window_end = alarm_start + params.post_alarm_window_sec;
            let mut active: Vec<&AudioEvent> = events
                .iter()
                .filter(|e| {
                    lookup(&e.label).is_some_and(|c| c.category == Category::Machine)
                        && e.end_sec > alarm_start
                        && e.start_sec < window_end
                })
                .collect();
            if active.is_empty() {
                return None;
            }
            // Faaliyet aralıklarının BİRLEŞİMİ alınmalı: aynı anda üç sınıf
            // (Dişliler + Mekanizmalar + Cırcır) tetiklenince süre üçe
            // katlanmamalı. Ayrıca faaliyet genelde kesintili olduğu için
            // "şu kadar saniye boyunca sürdü" demek olguyu yanlış aktarır —
            // toplam süre ile son görülme anı ayrı ayrı raporlanır.
            active.sort_by(|a, b| a.start_sec.total_cmp(&b.start_sec));
            let mut spans: Vec<(f32, f32)> = Vec::new();
            for e in &active {
                let s = e.start_sec.max(alarm_start);
                let t = e.end_sec.min(window_end);
                if t <= s {
                    continue;
                }
                match spans.last_mut() {
                    Some(last) if s <= last.1 => last.1 = last.1.max(t),
                    _ => spans.push((s, t)),
                }
            }
            if spans.is_empty() {
                return None;
            }
            let total: f32 = spans.iter().map(|(s, t)| t - s).sum();
            let last_end = spans.last().expect("boş değil").1;
            let bursts = spans.len();

            let mut evidence: Vec<String> = active
                .iter()
                .rev()
                .take(4)
                .map(|e| format!("{} {}–{}", e.label_tr, ts(e.start_sec), ts(e.end_sec)))
                .collect();
            evidence.insert(0, format!("Alarm {}–{}", ts(alarm_start), ts(alarm_end)));

            Some(Finding {
                rule: "post_alarm_activity",
                severity: Severity::Critical,
                start_sec: alarm_start,
                end_sec: last_end,
                title: "Alarm sonrası faaliyet sürdü".into(),
                detail: format!(
                    "{}'de alarm başladı. Sonraki {:.0} saniyede makine/el aleti sesi {} ayrı aralıkta, \
                     toplam {:.0} saniye tespit edildi; en son {} civarında. \
                     Tahliye yapılmamış ya da alarm duyulmamış olabilir.",
                    ts(alarm_start),
                    params.post_alarm_window_sec,
                    bursts,
                    total,
                    ts(last_end)
                ),
                evidence,
            })
        })
        .collect()
}

/// Kural 3 — araç sesiyle insan sesi aynı anda mı?
///
/// Geri manevra yapan aracın yakınında insan olması, forklift–yaya
/// çarpışmalarının klasik senaryosu. Ses yön bilgisi taşımadığı için bu kural
/// "çarpma oldu" demez, "bu aralığı izle" der.
fn vehicle_near_person(events: &[AudioEvent], params: &SafetyParams) -> Vec<Finding> {
    let vehicles: Vec<&AudioEvent> = events
        .iter()
        .filter(|e| lookup(&e.label).is_some_and(|c| c.category == Category::Vehicle))
        .collect();
    let humans: Vec<&AudioEvent> = events
        .iter()
        .filter(|e| {
            lookup(&e.label).is_some_and(|c| {
                c.category == Category::Human || c.category == Category::Distress
            })
        })
        .collect();

    let mut out: Vec<Finding> = Vec::new();
    for v in &vehicles {
        for h in &humans {
            // `events` zamana göre sıralı geldiğinden `humans` da sıralı: insan
            // sesi aracın bitişinden sonra başlıyorsa sonrakilerin hepsi de
            // başlıyor demektir. Kırpma artık bu kuralın girdisini sınırlamadığı
            // için (bkz. `events::cap_events`) bu erken çıkış, uzun ve gürültülü
            // kayıtlarda araç × insan taramasını ayakta tutuyor.
            if h.start_sec - params.cooccurrence_tolerance_sec >= v.end_sec {
                break;
            }
            if !overlaps(v, h, params.cooccurrence_tolerance_sec) {
                continue;
            }
            let start = v.start_sec.max(h.start_sec);
            let end = v.end_sec.min(h.end_sec);
            // Aynı aralık için birden fazla bulgu üretme
            if out.iter().any(|f| f.rule == "vehicle_near_person" && (f.start_sec - start).abs() < 1.0) {
                continue;
            }
            let distress = lookup(&h.label).is_some_and(|c| c.category == Category::Distress);
            out.push(Finding {
                rule: "vehicle_near_person",
                severity: if distress { Severity::Critical } else { Severity::Warning },
                start_sec: start,
                end_sec: end,
                title: "Araç sesiyle birlikte insan sesi".into(),
                detail: format!(
                    "{}–{} arasında {} ile {} aynı anda algılandı. Araç manevrası sırasında \
                     yakında insan bulunuyor olabilir; aralığın izlenmesi önerilir.",
                    ts(start),
                    ts(end),
                    v.label_tr,
                    h.label_tr
                ),
                evidence: vec![
                    format!("{} {}–{}", v.label_tr, ts(v.start_sec), ts(v.end_sec)),
                    format!("{} {}–{}", h.label_tr, ts(h.start_sec), ts(h.end_sec)),
                ],
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(label: &str, label_tr: &str, start: f32, end: f32) -> AudioEvent {
        AudioEvent {
            class_index: 0,
            label: label.into(),
            label_tr: label_tr.into(),
            mid: String::new(),
            start_sec: start,
            end_sec: end,
            peak_sec: start,
            confidence: 0.8,
            mean_confidence: 0.7,
        }
    }

    #[test]
    fn timestamps_gain_an_hour_field_past_one_hour() {
        assert_eq!(ts(0.0), "00:00");
        assert_eq!(ts(75.4), "01:15");
        assert_eq!(ts(3599.0), "59:59");
        assert_eq!(ts(3600.0), "1:00:00");
        // Eskiden "75:30" yazıyordu.
        assert_eq!(ts(4530.0), "1:15:30");
        assert_eq!(ts(-5.0), "00:00");
    }

    #[test]
    fn safety_class_names_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for c in SAFETY_CLASSES {
            assert!(seen.insert(c.en), "yinelenen sınıf: {}", c.en);
        }
    }

    #[test]
    fn only_safety_classes_are_extracted() {
        let events = vec![
            ev("Fire alarm", "Yangın alarmı", 10.0, 14.0),
            ev("Music", "Müzik", 0.0, 60.0), // güvenlikle ilgisiz
            ev("Jackhammer", "Kırıcı", 12.0, 30.0),
        ];
        let r = analyze(&events, &SafetyParams::default());
        let labels: Vec<&str> = r.events.iter().map(|e| e.label.as_str()).collect();
        assert_eq!(labels, vec!["Fire alarm", "Jackhammer"]);
    }

    #[test]
    fn post_alarm_rule_reports_continued_work() {
        let events = vec![
            ev("Fire alarm", "Yangın alarmı", 100.0, 106.0),
            ev("Jackhammer", "Kırıcı", 90.0, 145.0), // alarmdan sonra da sürüyor
        ];
        let r = analyze(&events, &SafetyParams::default());
        let f = r.findings.iter().find(|f| f.rule == "post_alarm_activity").expect("bulgu bekleniyor");
        assert_eq!(f.severity, Severity::Critical);
        assert_eq!(f.start_sec, 100.0);
        assert_eq!(f.end_sec, 145.0); // 45 saniye devam etti
        assert!(f.detail.contains("45"), "süre metinde geçmeli: {}", f.detail);
    }

    #[test]
    fn post_alarm_rule_silent_when_work_stops() {
        let events = vec![
            ev("Fire alarm", "Yangın alarmı", 100.0, 106.0),
            ev("Jackhammer", "Kırıcı", 40.0, 99.0), // alarmdan önce bitti
        ];
        let r = analyze(&events, &SafetyParams::default());
        assert!(!r.findings.iter().any(|f| f.rule == "post_alarm_activity"));
    }

    #[test]
    fn post_alarm_rule_ignores_activity_beyond_window() {
        let events = vec![
            ev("Fire alarm", "Yangın alarmı", 100.0, 106.0),
            ev("Jackhammer", "Kırıcı", 400.0, 420.0), // pencerenin (120 sn) dışında
        ];
        let r = analyze(&events, &SafetyParams::default());
        assert!(!r.findings.iter().any(|f| f.rule == "post_alarm_activity"));
    }

    #[test]
    fn fragmented_alarm_produces_single_finding() {
        // Aynı alarm üç parça hâlinde algılanırsa tek bulgu olmalı
        let events = vec![
            ev("Fire alarm", "Yangın alarmı", 100.0, 103.0),
            ev("Fire alarm", "Yangın alarmı", 104.0, 107.0),
            ev("Fire alarm", "Yangın alarmı", 108.0, 111.0),
            ev("Drill", "Matkap", 100.0, 130.0),
        ];
        let r = analyze(&events, &SafetyParams::default());
        let n = r.findings.iter().filter(|f| f.rule == "post_alarm_activity").count();
        assert_eq!(n, 1, "parçalı alarm tek bulgu üretmeli");
    }


    #[test]
    fn dedup_merges_same_alarm_under_two_labels() {
        // Ayni duman alarmi hem 'Duman dedektoru' hem 'Yangin alarmi' olarak
        // algilanabilir; tek bulgu olmali.
        let mut a = ev("Smoke detector, smoke alarm", "Duman dedektörü", 378.5, 384.5);
        a.confidence = 0.81;
        let mut b = ev("Fire alarm", "Yangın alarmı", 379.0, 384.5);
        b.confidence = 0.66;
        let r = analyze(&[a, b], &SafetyParams::default());
        let c: Vec<&Finding> = r.findings.iter().filter(|f| f.rule == "critical_sound").collect();
        assert_eq!(c.len(), 1, "ortusen ayni kategori tek bulgu olmali");
        assert!(c[0].title.contains("Duman dedektörü"), "en guvenli etiket baslik olmali");
        assert!(c[0].detail.contains("Yangın alarmı"), "digeri detayda gecmeli");
        assert_eq!(c[0].evidence.len(), 2);
    }

    #[test]
    fn different_categories_stay_separate() {
        let a = ev("Fire alarm", "Yangın alarmı", 100.0, 105.0);
        let b = ev("Screaming", "Çığlık", 101.0, 104.0); // distress, alarm degil
        let r = analyze(&[a, b], &SafetyParams::default());
        let c = r.findings.iter().filter(|f| f.rule == "critical_sound").count();
        assert_eq!(c, 2, "farkli kategoriler birlestirilmemeli");
    }

    #[test]
    fn post_alarm_reports_total_not_span() {
        // Kesintili faaliyet: 3 sn + 3 sn, aralarinda 20 sn bosluk.
        // Yanlis: '26 saniye boyunca surdu'. Dogru: 'toplam 6 saniye, 2 aralik'.
        let events = vec![
            ev("Fire alarm", "Yangın alarmı", 100.0, 106.0),
            ev("Drill", "Matkap", 103.0, 106.0),
            ev("Drill", "Matkap", 123.0, 126.0),
        ];
        let r = analyze(&events, &SafetyParams::default());
        let f = r.findings.iter().find(|f| f.rule == "post_alarm_activity").unwrap();
        assert!(f.detail.contains("2 ayrı aralıkta"), "aralik sayisi: {}", f.detail);
        assert!(f.detail.contains("toplam 6 saniye"), "toplam sure: {}", f.detail);
        assert_eq!(f.end_sec, 126.0);
    }

    #[test]
    fn overlapping_machine_classes_counted_once() {
        // Ayni anda uc sinif tetiklenirse sure uce katlanmamali.
        let events = vec![
            ev("Fire alarm", "Yangın alarmı", 100.0, 106.0),
            ev("Gears", "Dişliler", 103.0, 110.0),
            ev("Mechanisms", "Mekanizmalar", 103.0, 110.0),
            ev("Drill", "Matkap", 104.0, 109.0),
        ];
        let r = analyze(&events, &SafetyParams::default());
        let f = r.findings.iter().find(|f| f.rule == "post_alarm_activity").unwrap();
        assert!(f.detail.contains("1 ayrı aralıkta"), "tek aralik olmali: {}", f.detail);
        assert!(f.detail.contains("toplam 7 saniye"), "birlesim 7 sn olmali: {}", f.detail);
    }

    #[test]
    fn vehicle_and_person_overlap_is_flagged() {
        let events = vec![
            ev("Reversing beeps", "Geri vites uyarı sesi", 50.0, 56.0),
            ev("Speech", "Konuşma", 54.0, 60.0),
        ];
        let r = analyze(&events, &SafetyParams::default());
        let f = r.findings.iter().find(|f| f.rule == "vehicle_near_person").expect("bulgu bekleniyor");
        assert_eq!(f.start_sec, 54.0);
        assert_eq!(f.end_sec, 56.0);
        assert_eq!(f.severity, Severity::Warning);
    }

    #[test]
    fn vehicle_with_distress_is_critical() {
        let events = vec![
            ev("Reversing beeps", "Geri vites uyarı sesi", 50.0, 56.0),
            ev("Screaming", "Çığlık", 54.0, 58.0),
        ];
        let r = analyze(&events, &SafetyParams::default());
        let f = r.findings.iter().find(|f| f.rule == "vehicle_near_person").unwrap();
        assert_eq!(f.severity, Severity::Critical);
    }

    #[test]
    fn separated_vehicle_and_person_not_flagged() {
        let events = vec![
            ev("Reversing beeps", "Geri vites uyarı sesi", 50.0, 56.0),
            ev("Speech", "Konuşma", 70.0, 80.0), // örtüşmüyor
        ];
        let r = analyze(&events, &SafetyParams::default());
        assert!(!r.findings.iter().any(|f| f.rule == "vehicle_near_person"));
    }

    #[test]
    fn critical_sound_rule_fires_for_scream_not_for_hammer() {
        let events = vec![
            ev("Screaming", "Çığlık", 10.0, 12.0),
            ev("Hammer", "Çekiç", 20.0, 25.0),
        ];
        let r = analyze(&events, &SafetyParams::default());
        let critical: Vec<&Finding> = r.findings.iter().filter(|f| f.rule == "critical_sound").collect();
        assert_eq!(critical.len(), 1);
        assert!(critical[0].title.contains("Çığlık"));
    }
}
