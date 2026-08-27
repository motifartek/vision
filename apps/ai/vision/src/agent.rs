//! Video analiz ajanı.
//!
//! Döngü kısa ve kasıtlı olarak öyle: **bak → emin değilsen yakınlaştır →
//! raporla**. Şartnamenin istediği çıktıyı üreten yer burası.
//!
//! # Zaman çevirisi
//!
//! Ajanın en kolay gözden kaçan işi bu. Yakınlaştırma klipleri ağır çekime
//! alınıyor ve model **klibin kendi saatini** raporluyor. Ölçüldü: 12–15 sn
//! aralığı 8 kat yavaşlatılınca model olayları `00:20–00:22` diye verdi ve
//! isteme dönüşüm formülü açıkça yazılmasına rağmen düzelmedi.
//!
//! Bu yüzden her rapor, o raporu üreten klibin `ClipRef`'i üzerinden
//! [`ClipRef::to_source_ms`] ile kaynak zamana taşınıyor.

use std::sync::Arc;

use motif_event_sdk::{AnalysisReport, ClipRef, DetectedEvent, RiskLevel, SCHEMA_VERSION};

use crate::stream_client::ClipSource;
use crate::vlm::{Decision, RawReport, VlmProvider};

/// Ajanın kaç kez yakınlaştırabileceği.
///
/// Her tur bir video isteği demek ve servis tüm takımlarca paylaşılıyor.
/// Ölçümde tek turluk analiz zaten 12/13 olayı yakalıyordu; sınır bu yüzden dar.
pub const MAX_ZOOM: usize = 2;

/// Yakınlaştırmada istenen kare sayısı.
///
/// Servis 2 fps örneklediği için bu sayı aynı zamanda ağır çekim oranını
/// belirliyor: 3 saniyelik aralıktan 48 kare istemek 8× yavaşlatma demek.
pub const ZOOM_BUDGET: usize = 48;

/// Uzun kenar sınırı. Token maliyeti kare alanıyla doğru orantılı.
pub const MAX_DIM: u32 = 768;

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error(transparent)]
    Stream(#[from] crate::stream_client::StreamError),
    #[error(transparent)]
    Vlm(#[from] crate::vlm::VlmError),
    #[error("model {MAX_ZOOM} yakınlaştırmadan sonra da rapor vermedi")]
    NoReport,
}

/// Ajanın attığı her adım. Panelde ve günlükte açıklanabilirlik için tutuluyor.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentStep {
    pub step: usize,
    pub action: String,
    pub t0_ms: u64,
    pub t1_ms: u64,
    pub time_scale: f32,
    pub service_frames: u32,
    pub elapsed_ms: u64,
}

/// Rapor + ajanın oraya nasıl vardığı.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentOutcome {
    pub report: AnalysisReport,
    pub steps: Vec<AgentStep>,
}

pub struct VisionAgent {
    stream: Arc<dyn ClipSource>,
    vlm: Arc<dyn VlmProvider>,
}

impl VisionAgent {
    pub fn new(stream: Arc<dyn ClipSource>, vlm: Arc<dyn VlmProvider>) -> Self {
        Self { stream, vlm }
    }

    pub async fn analyze(&self, video_id: &str) -> Result<AgentOutcome, AgentError> {
        let basladi = std::time::Instant::now();
        let mut steps = Vec::new();

        let info = self.stream.video_info(video_id).await?;
        let mut clip = self
            .stream
            .full_clip(video_id, info.duration_ms, Some(MAX_DIM))
            .await?;
        let mut prompt = ilk_istem(info.duration_ms);

        for tur in 0..=MAX_ZOOM {
            let adim_basladi = std::time::Instant::now();
            let baytlar = self.stream.fetch_blob(&clip.object_key).await?;

            tracing::info!(
                tur,
                t0_ms = clip.t0_ms,
                t1_ms = clip.t1_ms,
                time_scale = clip.time_scale,
                kare = clip.service_frames,
                mb = baytlar.len() as f64 / 1e6,
                "klip modele gönderiliyor"
            );

            let karar = self.vlm.analyze(&prompt, &baytlar).await?;

            match karar {
                Decision::Report(ham) => {
                    steps.push(AgentStep {
                        step: tur,
                        action: "report".into(),
                        t0_ms: clip.t0_ms,
                        t1_ms: clip.t1_ms,
                        time_scale: clip.time_scale,
                        service_frames: clip.service_frames,
                        elapsed_ms: adim_basladi.elapsed().as_millis() as u64,
                    });

                    let report = rapora_cevir(
                        video_id,
                        ham,
                        &clip,
                        basladi.elapsed().as_millis() as u64,
                    );
                    return Ok(AgentOutcome { report, steps });
                }
                Decision::ZoomRange { t0_ms, t1_ms } => {
                    steps.push(AgentStep {
                        step: tur,
                        action: format!("zoom_range({t0_ms},{t1_ms})"),
                        t0_ms: clip.t0_ms,
                        t1_ms: clip.t1_ms,
                        time_scale: clip.time_scale,
                        service_frames: clip.service_frames,
                        elapsed_ms: adim_basladi.elapsed().as_millis() as u64,
                    });

                    if tur == MAX_ZOOM {
                        break;
                    }

                    // Modelin istediği aralık kaynağın dışına taşabiliyor.
                    let t0 = t0_ms.min(info.duration_ms.saturating_sub(1));
                    let t1 = t1_ms.min(info.duration_ms).max(t0 + 500);

                    clip = self.stream.zoom_clip(video_id, t0, t1, ZOOM_BUDGET).await?;
                    prompt = yakinlastirma_istemi(&clip);
                }
            }
        }

        Err(AgentError::NoReport)
    }
}

/// Modelin ham raporunu şartname raporuna çevirir.
///
/// İki iş yapıyor: zamanları kaynak videoya taşıyor ve boş kalan alanları
/// güvenli varsayılanlara bağlıyor. `actions` boş dönerse şartnamenin açık bir
/// maddesi karşılanmamış olur, o yüzden en azından kayıt önerisi bırakılıyor.
fn rapora_cevir(
    video_id: &str,
    ham: RawReport,
    clip: &ClipRef,
    processing_ms: u64,
) -> AnalysisReport {
    let mut events: Vec<DetectedEvent> = ham
        .events
        .into_iter()
        .map(|e| {
            let kaynak_ms = clip.to_source_ms(e.klip_ms());
            DetectedEvent::new(kaynak_ms, e.event, risk_cevir(&e.severity))
        })
        .collect();
    events.sort_by_key(|e| e.t_ms);

    // Genel risk modelden geliyor; vermezse olayların en yükseğine düşülüyor.
    let risk = if ham.risk.is_empty() {
        events
            .iter()
            .map(|e| e.severity)
            .max_by_key(|r| r.severity_rank())
            .unwrap_or(RiskLevel::Dusuk)
    } else {
        risk_cevir(&ham.risk)
    };

    let actions = if ham.actions.is_empty() {
        vec!["Kaydı incelemeye al ve olayı raporla".to_string()]
    } else {
        ham.actions
    };

    let summary = if ham.summary.trim().is_empty() {
        "Kayıt analiz edildi; özet üretilemedi.".to_string()
    } else {
        ham.summary
    };

    AnalysisReport {
        schema_version: SCHEMA_VERSION,
        video_id: video_id.to_string().into(),
        summary,
        events,
        risk,
        actions,
        processing_ms: Some(processing_ms),
    }
}

/// Modelin yazdığı risk metnini şartnamenin üç seviyesine indirger.
///
/// Şema `enum` ile kısıtlı ama model yine de "yüksek", "HIGH" gibi varyantlar
/// üretebiliyor; eşleşmeyen her şey `Orta` sayılıyor — sessizce `Düşük`e
/// düşürmek riski gizlerdi.
fn risk_cevir(s: &str) -> RiskLevel {
    let d = s.trim().to_lowercase();
    if d.starts_with("yüks") || d.starts_with("yuks") || d.starts_with("high") {
        RiskLevel::Yuksek
    } else if d.starts_with("düş") || d.starts_with("dus") || d.starts_with("low") {
        RiskLevel::Dusuk
    } else {
        RiskLevel::Orta
    }
}

/// Modelden istenen çıktı sözleşmesi.
///
/// Şema araç tanımı yerine isteme yazılıyor: servis araç çağrısını
/// desteklemiyor, gerekçesi [`crate::vlm`] modülünde.
const SOZLESME: &str = r#"

Yalnızca JSON döndür, başka hiçbir şey yazma.

Emin değilsen ve bir aralığa yakından bakman gerekiyorsa:
{"zoom": {"t0_ms": <başlangıç>, "t1_ms": <bitiş>}}

Emin olduğunda raporu ver:
{"summary": "kısa Türkçe özet",
 "events": [{"time": "MM:SS", "event": "olay açıklaması", "severity": "Düşük|Orta|Yüksek"}],
 "risk": "Düşük|Orta|Yüksek",
 "actions": ["operatörün hemen uygulayabileceği somut öneri"]}

actions boş bırakılamaz ve genel geçer olmamalı; sahnede gördüğüne dayanmalı."#;

fn ilk_istem(duration_ms: u64) -> String {
    format!(
        "Sen bir iş sağlığı ve güvenliği analistisin. Sana {} uzunluğunda bir \
         güvenlik kamerası kaydı verildi.\n\n\
         Sahnede ne olduğunu, riskli ya da olağandışı bir durum bulunup \
         bulunmadığını değerlendir. Olayın başlangıç, gelişim ve sonuç \
         aşamalarını ayrı olaylar olarak işaretle.\n\n\
         Zamanları bu kaydın başından itibaren geçen süre olarak MM:SS \
         biçiminde ver. Kameranın görüntü üzerine bastığı saati kullanma.{SOZLESME}",
        motif_event_sdk::format_timestamp(duration_ms)
    )
}

fn yakinlastirma_istemi(clip: &ClipRef) -> String {
    let baslik = format!(
        "İstediğin {} – {} aralığının klibi.",
        motif_event_sdk::format_timestamp(clip.t0_ms),
        motif_event_sdk::format_timestamp(clip.t1_ms)
    );

    let hiz = if clip.time_scale > 1.01 {
        format!(
            " Klip {:.0} kat ağır çekimde: olaylar gerçekte burada göründüğünden \
             {:.0} kat hızlı gelişiyor.",
            clip.time_scale, clip.time_scale
        )
    } else {
        String::new()
    };

    format!(
        "{baslik}{hiz}\n\n\
         Bu aralıkta tam olarak ne olduğunu belirle ve raporu ver. Artık \
         yakınlaştırma isteme.\n\n\
         Zamanları BU KLİBİN başından itibaren MM:SS biçiminde ver; kaynak \
         kayda çevirmeye çalışma, o hesabı biz yapıyoruz.{SOZLESME}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream_client::StreamError;
    use crate::vlm::{Decision, RawEvent, VlmError};
    use motif_event_sdk::VideoInfoResponse;
    use std::sync::Mutex;

    /// Klip üretmeyen, yalnızca ne istendiğini kaydeden sahte kaynak.
    struct SahteKaynak {
        istekler: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl ClipSource for SahteKaynak {
        async fn video_info(&self, _v: &str) -> Result<VideoInfoResponse, StreamError> {
            Ok(VideoInfoResponse {
                duration_ms: 35_000,
                fps: 30.0,
                width: 854,
                height: 480,
                size_bytes: 1_000_000,
                codec: "h264".into(),
            })
        }
        async fn full_clip(
            &self,
            _v: &str,
            duration_ms: u64,
            _m: Option<u32>,
        ) -> Result<ClipRef, StreamError> {
            self.istekler.lock().unwrap().push("full".into());
            Ok(ClipRef {
                t0_ms: 0,
                t1_ms: duration_ms,
                object_key: "clips/full.mp4".into(),
                duration_ms,
                time_scale: 1.0,
                service_frames: 70,
                effective_fps: 2.0,
            })
        }
        async fn zoom_clip(
            &self,
            _v: &str,
            t0_ms: u64,
            t1_ms: u64,
            _b: usize,
        ) -> Result<ClipRef, StreamError> {
            self.istekler
                .lock()
                .unwrap()
                .push(format!("zoom({t0_ms},{t1_ms})"));
            // 8x ağır çekim: gerçek servisin 3 sn / 48 kare için yaptığı şey.
            Ok(ClipRef {
                t0_ms,
                t1_ms,
                object_key: "clips/zoom.mp4".into(),
                duration_ms: (t1_ms - t0_ms) * 8,
                time_scale: 8.0,
                service_frames: 48,
                effective_fps: 16.0,
            })
        }
        async fn fetch_blob(&self, _k: &str) -> Result<Vec<u8>, StreamError> {
            Ok(vec![0u8; 16])
        }
    }

    /// Sırayla önceden yazılmış kararları döndüren sahte model.
    struct SahteModel {
        kararlar: Mutex<Vec<Decision>>,
    }

    #[async_trait::async_trait]
    impl VlmProvider for SahteModel {
        async fn analyze(&self, _p: &str, _c: &[u8]) -> Result<Decision, VlmError> {
            let mut k = self.kararlar.lock().unwrap();
            if k.is_empty() {
                return Err(VlmError::NoDecision("senaryo bitti".into()));
            }
            Ok(k.remove(0))
        }
    }

    fn rapor(t_ms_metin: &str) -> Decision {
        Decision::Report(RawReport {
            summary: "özet".into(),
            events: vec![RawEvent {
                time: Some(t_ms_metin.into()),
                t_ms: None,
                event: "olay".into(),
                severity: "Yüksek".into(),
            }],
            risk: "Yüksek".into(),
            actions: vec!["aksiyon".into()],
        })
    }

    /// Ajan yakınlaştırma isteğini gerçekten uyguluyor mu?
    ///
    /// Canlı koşuda model 10 videonun onunda da ilk turda rapor verdi, yani bu
    /// yol sahada hiç çalışmadı. Ölü kod olmadığı burada kanıtlanıyor.
    #[tokio::test]
    async fn yakinlastirma_istegi_uygulanir_ve_zaman_kaynaga_tasinir() {
        let kaynak = Arc::new(SahteKaynak {
            istekler: Mutex::new(Vec::new()),
        });
        let model = Arc::new(SahteModel {
            kararlar: Mutex::new(vec![
                Decision::ZoomRange {
                    t0_ms: 12_000,
                    t1_ms: 15_000,
                },
                // Yakınlaştırılmış klipte model 20. saniyeyi gösteriyor.
                // 8x ağır çekimde kaynak karşılığı 12 + 20/8 = 14.5 sn.
                rapor("00:20"),
            ]),
        });

        let ajan = VisionAgent::new(kaynak.clone(), model);
        let sonuc = ajan.analyze("v1").await.unwrap();

        let istekler = kaynak.istekler.lock().unwrap().clone();
        assert_eq!(istekler, vec!["full", "zoom(12000,15000)"]);

        assert_eq!(sonuc.steps.len(), 2);
        assert!(sonuc.steps[0].action.starts_with("zoom_range"));
        assert_eq!(sonuc.steps[1].action, "report");

        assert_eq!(sonuc.report.events[0].t_ms, 14_500);
        assert_eq!(sonuc.report.events[0].time, "00:14");
    }

    /// Model sürekli yakınlaştırma isterse döngü sonsuza gitmemeli.
    #[tokio::test]
    async fn surekli_yakinlastirma_sinirda_durur() {
        let kaynak = Arc::new(SahteKaynak {
            istekler: Mutex::new(Vec::new()),
        });
        let model = Arc::new(SahteModel {
            kararlar: Mutex::new(
                (0..10)
                    .map(|_| Decision::ZoomRange {
                        t0_ms: 1_000,
                        t1_ms: 2_000,
                    })
                    .collect(),
            ),
        });

        let ajan = VisionAgent::new(kaynak.clone(), model);
        let hata = ajan.analyze("v1").await.unwrap_err();
        assert!(matches!(hata, AgentError::NoReport));
        // MAX_ZOOM + 1 tur; kaynak isteği bir tam + MAX_ZOOM yakınlaştırma.
        assert_eq!(kaynak.istekler.lock().unwrap().len(), 1 + MAX_ZOOM);
    }

    /// Model istediği aralık kaydın dışına taşarsa kırpılmalı.
    #[tokio::test]
    async fn aralik_kaynak_sinirlarina_kirpilir() {
        let kaynak = Arc::new(SahteKaynak {
            istekler: Mutex::new(Vec::new()),
        });
        let model = Arc::new(SahteModel {
            kararlar: Mutex::new(vec![
                Decision::ZoomRange {
                    t0_ms: 30_000,
                    t1_ms: 900_000, // 35 sn'lik kayıtta 15 dakika istiyor
                },
                rapor("00:00"),
            ]),
        });

        let ajan = VisionAgent::new(kaynak.clone(), model);
        ajan.analyze("v1").await.unwrap();

        let istekler = kaynak.istekler.lock().unwrap().clone();
        assert_eq!(istekler[1], "zoom(30000,35000)");
    }

    use crate::vlm::RawReport;


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
    fn agir_cekim_raporu_kaynak_zamana_tasinir() {
        // Ölçülen gerçek durum: 12-15 sn aralığı 8x yavaşlatıldı, model olayı
        // klibin 20. saniyesinde gördü. Kaynakta 12 + 20/8 = 14.5 sn.
        let ham = RawReport {
            summary: "Raf devrildi".into(),
            events: vec![RawEvent {
                time: Some("00:20".into()),
                t_ms: None,
                event: "Raf çöktü".into(),
                severity: "Yüksek".into(),
            }],
            risk: "Yüksek".into(),
            actions: vec!["Alanı boşalt".into()],
        };

        let r = rapora_cevir("v1", ham, &clip(12_000, 15_000, 8.0), 4200);
        assert_eq!(r.events[0].t_ms, 14_500);
        assert_eq!(r.events[0].time, "00:14");
        assert_eq!(r.risk, RiskLevel::Yuksek);
    }

    #[test]
    fn bos_aksiyon_listesi_doldurulur() {
        // Şartname §3 aksiyon önerisini zorunlu tutuyor; boş liste maddeyi
        // karşılamıyor demektir.
        let ham = RawReport {
            summary: "Olağan çalışma".into(),
            events: vec![],
            risk: "Düşük".into(),
            actions: vec![],
        };
        let r = rapora_cevir("v1", ham, &clip(0, 30_000, 1.0), 100);
        assert!(!r.actions.is_empty());
    }

    #[test]
    fn risk_verilmezse_en_yuksek_olaydan_turetilir() {
        let ham = RawReport {
            summary: "İki olay".into(),
            events: vec![
                RawEvent { time: None, t_ms: Some(1_000), event: "a".into(), severity: "Düşük".into() },
                RawEvent { time: None, t_ms: Some(2_000), event: "b".into(), severity: "Yüksek".into() },
            ],
            risk: String::new(),
            actions: vec!["x".into()],
        };
        let r = rapora_cevir("v1", ham, &clip(0, 30_000, 1.0), 100);
        assert_eq!(r.risk, RiskLevel::Yuksek);
    }

    #[test]
    fn olaylar_zaman_sirasina_dizilir() {
        let ham = RawReport {
            summary: "s".into(),
            events: vec![
                RawEvent { time: None, t_ms: Some(9_000), event: "sonra".into(), severity: "Orta".into() },
                RawEvent { time: None, t_ms: Some(2_000), event: "önce".into(), severity: "Orta".into() },
            ],
            risk: "Orta".into(),
            actions: vec!["x".into()],
        };
        let r = rapora_cevir("v1", ham, &clip(0, 30_000, 1.0), 100);
        assert_eq!(r.events[0].event, "önce");
        assert_eq!(r.events[1].event, "sonra");
    }

    #[test]
    fn taninmayan_risk_metni_ortaya_dusurulur() {
        // Sessizce "Düşük" saymak riski gizlerdi.
        assert_eq!(risk_cevir("kritik"), RiskLevel::Orta);
        assert_eq!(risk_cevir("HIGH"), RiskLevel::Yuksek);
        assert_eq!(risk_cevir("düşük"), RiskLevel::Dusuk);
        assert_eq!(risk_cevir("Yuksek"), RiskLevel::Yuksek);
    }

    #[test]
    fn sartname_bicimi_uretilir() {
        let ham = RawReport {
            summary: "Forklift kazası".into(),
            events: vec![RawEvent {
                time: Some("00:15".into()),
                t_ms: None,
                event: "Forklift devrildi".into(),
                severity: "Yüksek".into(),
            }],
            risk: "Yüksek".into(),
            actions: vec!["Sağlık ekibini çağır".into()],
        };
        let r = rapora_cevir("v1", ham, &clip(0, 30_000, 1.0), 100);
        let j = r.to_sartname_json();

        assert_eq!(j["summary"], "Forklift kazası");
        assert_eq!(j["events"][0]["time"], "00:15");
        assert_eq!(j["events"][0]["event"], "Forklift devrildi");
        assert_eq!(j["risk"], "Yüksek");
        assert_eq!(j["actions"][0], "Sağlık ekibini çağır");
        // Dahili alanlar teslim biçimine sızmamalı.
        assert!(j["events"][0].get("t_ms").is_none());
        assert!(j.get("video_id").is_none());
    }
}
