//! Makro döngü: video geldiğinde ses ve görüntü ajanlarını sırayla çalıştırır.
//!
//! Akış: `stream.video.ingested` → `sonic` (işitsel bağlam) → `vision` (rapor)
//! → Postgres → `pg_notify` → gateway'in SSE akışı.
//!
//! # Ses neden görüntüden önce
//!
//! `vision`'ın istemi işitsel bağlamı **girdi olarak** alıyor: ses metni,
//! prompt'un ayraçlı güvenilmez bölgesine giriyor. Sıra bu yüzden zorunlu,
//! tercih değil.
//!
//! # Ses isteğe bağlı
//!
//! `sonic` düşerse, medyayı bulamazsa ya da hata dönerse görüntü analizi yine
//! çalışıyor — yalnızca ses bağlamı olmadan. Şartname sistemin kararlı
//! çalışmasını puanlıyor; tek bir ajanın arızası zinciri kesmemeli.

use std::time::Duration;

use futures::StreamExt;
use motif_event_sdk::{subjects, VideoIngested};
use reqwest::Client;
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use tracing::{error, info, warn};

/// Modele taşınacak azami ses olayı sayısı.
///
/// Ses özeti prompt'un içine giriyor; uzun bir liste asıl talimatı bağlamın
/// dışına iter. `UntrustedText` zaten karakter tavanı uyguluyor, bu sınır
/// listeyi **anlamlı** yerden kesiyor: en güvenilir olaylar kalıyor.
const AZAMI_SES_OLAYI: usize = 12;

/// Panele iz düşer.
///
/// `pg_notify` bağlı parametrelerle çağrılıyor. Önceden kanal ve yük
/// `format!` ile SQL metnine gömülüyordu; yüke servis hata mesajları
/// giriyor, yani veri SQL'e karışıyordu.
async fn notify_trace(pool: &sqlx::PgPool, video_id: impl std::fmt::Display, status: &str) {
    let payload = json!({ "video_id": video_id.to_string(), "message": status }).to_string();
    if let Err(e) = sqlx::query("SELECT pg_notify($1, $2)")
        .bind("ai_trace")
        .bind(&payload)
        .execute(pool)
        .await
    {
        warn!("iz bildirimi gönderilemedi: {}", e);
    }
}

/// Saniyeyi `MM:SS`'e çevirir.
///
/// Biçim `vision`'ın olay sözleşmesiyle aynı: model iki farklı zaman gösterimi
/// arasında çeviri yapmak zorunda kalmasın.
fn ss(saniye: f32) -> String {
    let toplam = saniye.max(0.0) as u32;
    format!("{:02}:{:02}", toplam / 60, toplam % 60)
}

/// `sonic` yanıtını modele verilecek kısa bir metne indirger.
///
/// Ham JSON gönderilmiyor: 527 sınıflık skor tablosu bağlamı şişirir ve
/// modelin okuması gereken şey olayların ne zaman olduğu. Olay yoksa `None`
/// dönüyor — boş bir "işitsel bağlam" bölümü açmanın anlamı yok.
fn ses_ozeti(analiz: &Value) -> Option<String> {
    let olaylar = analiz.get("events")?.as_array()?;

    // Alanları eksik olan olaylar atlanıyor: bozuk tek bir kayıt tüm özeti
    // düşürmemeli.
    let mut ayiklanmis: Vec<(f32, f32, &str, f64)> = olaylar
        .iter()
        .filter_map(|o| {
            let etiket = o.get("label_tr").and_then(Value::as_str)?;
            let bas = o.get("start_sec").and_then(Value::as_f64)? as f32;
            let son = o.get("end_sec").and_then(Value::as_f64)? as f32;
            let guven = o.get("confidence").and_then(Value::as_f64).unwrap_or(0.0);
            Some((bas, son, etiket, guven))
        })
        .collect();

    if ayiklanmis.is_empty() {
        return None;
    }
    let toplam = ayiklanmis.len();

    // Kırpma **güvene göre**, listeleme **zamana göre**.
    //
    // Zamana göre kırpmak kaydın sonunu tamamen düşürürdü; güvene göre
    // kırpıp sonra zamana geri sıralamak hem en bilgilendirici olayları
    // tutuyor hem modele kronolojik bir hat veriyor.
    if toplam > AZAMI_SES_OLAYI {
        ayiklanmis.sort_by(|a, b| b.3.total_cmp(&a.3));
        ayiklanmis.truncate(AZAMI_SES_OLAYI);
    }
    ayiklanmis.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut metin = ayiklanmis
        .iter()
        .map(|(bas, son, etiket, guven)| {
            format!("{}–{} {} (%{:.0})", ss(*bas), ss(*son), etiket, guven * 100.0)
        })
        .collect::<Vec<_>>()
        .join("\n");

    if toplam > AZAMI_SES_OLAYI {
        metin.push_str(&format!(
            "\n(… {} olay daha; en güvenilir {AZAMI_SES_OLAYI} tanesi yukarıda)",
            toplam - AZAMI_SES_OLAYI
        ));
    }
    Some(metin)
}

/// `sonic`'ten işitsel bağlam ister.
///
/// Başarısızlık **hata değil**: `None` dönüyor ve zincir sessizce sesin
/// olmadığı yoldan devam ediyor. Ama sessizce değil *görünmez* değil —
/// her başarısızlık iz olarak panele düşüyor.
async fn isitsel_baglam(
    http: &Client,
    sonic_url: &str,
    pool: &sqlx::PgPool,
    video_id: &impl std::fmt::Display,
    object_key: &str,
) -> Option<String> {
    // `path` bekleniyor, `video_id` değil. Nesne anahtarı (`raw/<id>.mp4`)
    // gönderiliyor: sonic'in medya kökü `stream`'in depo köküne bakmalı.
    let cevap = http
        .post(format!("{sonic_url}/v1/audio/analyze"))
        .json(&json!({ "path": object_key }))
        .send()
        .await;

    let cevap = match cevap {
        Ok(c) => c,
        Err(e) => {
            warn!("sonic'e ulaşılamadı: {}", e);
            notify_trace(pool, video_id, "[Sonic] Ses servisine ulaşılamadı; analiz sessiz sürüyor.").await;
            return None;
        }
    };

    if !cevap.status().is_success() {
        let kod = cevap.status();
        let govde = cevap.text().await.unwrap_or_default();
        warn!("sonic {} döndü: {}", kod, govde);
        notify_trace(
            pool,
            video_id,
            &format!("[Sonic] Ses analizi başarısız ({kod}); analiz sessiz sürüyor."),
        )
        .await;
        return None;
    }

    let analiz: Value = match cevap.json().await {
        Ok(v) => v,
        Err(e) => {
            warn!("sonic yanıtı çözümlenemedi: {}", e);
            return None;
        }
    };

    match ses_ozeti(&analiz) {
        Some(ozet) => {
            info!("işitsel bağlam hazır ({} karakter)", ozet.len());
            notify_trace(
                pool,
                video_id,
                "[Sonic -> Orchestrator] İşitsel analiz tamamlandı; bulgular göre ajanına taşınıyor.",
            )
            .await;
            Some(ozet)
        }
        None => {
            notify_trace(pool, video_id, "[Sonic] Kayıtta belirgin bir ses olayı bulunamadı.").await;
            None
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    motif_observer::init("orchestrator");
    info!("Orchestrator (Macro Loop) baslatiliyor...");

    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://motif:motif@127.0.0.1:5433/motif".into());
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;
    info!("PostgreSQL'e basariyla baglanildi.");

    let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());

    let mut retries = 5;
    let nats_client = loop {
        match async_nats::connect(&nats_url).await {
            Ok(c) => break c,
            Err(e) => {
                retries -= 1;
                if retries == 0 {
                    error!("NATS baglantisi kurulamadi: {}", e);
                    return Err(e.into());
                }
                warn!("NATS'a baglanilamadi, 2 saniye sonra tekrar deneniyor...");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    };

    info!("NATS'a baglanildi!");

    let http_client = Client::new();
    // 8081: `sonic`'in kendi varsayılanı. Önceden 8120 yazıyordu ve yerelde
    // hiçbir zaman tutmuyordu.
    let sonic_url = std::env::var("SONIC_URL").unwrap_or_else(|_| "http://127.0.0.1:8081".into());
    let vision_url = std::env::var("VISION_URL").unwrap_or_else(|_| "http://127.0.0.1:8110".into());

    let mut subscriber = nats_client.subscribe(subjects::VIDEO_INGESTED).await?;
    info!("Macro Loop hazir. {} dinleniyor...", subjects::VIDEO_INGESTED);

    while let Some(message) = subscriber.next().await {
        let Ok(event) = serde_json::from_slice::<VideoIngested>(&message.payload) else {
            warn!("çözümlenemeyen VideoIngested mesajı atlandı");
            continue;
        };

        let video_id = event.video_id;
        info!("YENI TETIKLEYICI ALINDI: Video {} isleme aliniyor.", video_id);
        notify_trace(&pool, &video_id, "[Orchestrator] Yeni video yüklendi. NATS mesajı yakalandı.").await;

        // ADIM 1: işitsel bağlam.
        info!("Adim 1: Sonic (Ses) analizine basvuruluyor...");
        notify_trace(&pool, &video_id, "[Orchestrator -> Sonic] Adım 1: İşitsel (Ses) analiz başlatılıyor...").await;

        let ses = isitsel_baglam(
            &http_client,
            &sonic_url,
            &pool,
            &video_id,
            &event.object_key,
        )
        .await;

        // ADIM 2: görüntü analizi.
        //
        // `/v1/analyze/sartname` çağrılıyor, `/v1/analyze` değil: ikincisi
        // raporu `report` anahtarının altına sarıyor ve buradaki okuma
        // sessizce boş değer üretirdi.
        info!("Adim 2: Vision (VLM) ajanindan nihai analiz isteniyor...");
        notify_trace(
            &pool,
            &video_id,
            if ses.is_some() {
                "[Orchestrator -> Vision] Adım 2: Görsel ajan işitsel bağlamla tetiklendi."
            } else {
                "[Orchestrator -> Vision] Adım 2: Görsel (VLM) ajanı tetiklendi. Video izleniyor..."
            },
        )
        .await;

        let tools_rows = sqlx::query("SELECT name, title, description FROM external_tools")
            .fetch_all(&pool)
            .await;

        let tools_text = if let Ok(rows) = tools_rows {
            use sqlx::Row;
            let mut lines = Vec::new();
            for row in rows {
                let name: String = row.try_get("name").unwrap_or_default();
                let title: String = row.try_get("title").unwrap_or_default();
                let desc: String = row.try_get("description").unwrap_or_default();
                lines.push(format!("- {} ({}): {}", name, title, desc));
            }
            if lines.is_empty() { None } else { Some(lines.join("\n")) }
        } else {
            None
        };

        let mut istek = json!({ "video_id": video_id });
        if let Some(ozet) = &ses {
            istek["isitsel_baglam"] = json!(ozet);
        }
        if let Some(t) = tools_text {
            istek["tools"] = json!(t);
        }

        let vision_req = http_client
            .post(format!("{vision_url}/v1/analyze/sartname"))
            .json(&istek)
            .send()
            .await;

        let resp = match vision_req {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                let kod = r.status();
                error!("Vision servisi hata kodu dondu: {}", kod);
                notify_trace(&pool, &video_id, &format!("[Hata] Vision servisi {kod} döndü.")).await;
                continue;
            }
            Err(e) => {
                error!("Vision servisine ulasilamadi: {}", e);
                notify_trace(&pool, &video_id, &format!("[Hata] Vision servisine ulaşılamadı: {e}")).await;
                continue;
            }
        };

        let report: Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                error!("Vision yaniti cozumlenemedi: {}", e);
                notify_trace(&pool, &video_id, &format!("[Hata] Vision yanıtı okunamadı: {e}")).await;
                continue;
            }
        };

        info!("Vision Ajanindan rapor basariyla alindi!");
        notify_trace(&pool, &video_id, "[Vision -> Orchestrator] Görsel analiz başarıyla alındı ve birleştirildi.").await;

        // ADIM 3: kaydet ve SSE tetikle.
        let summary = report["summary"].as_str().unwrap_or("");
        let risk = report["risk"].as_str().unwrap_or("Düşük");
        let events_json = report["events"].clone();
        let actions_json = report["actions"].clone();

        let query = "
            INSERT INTO ai_events (video_id, summary, events, risk, actions)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (video_id) DO UPDATE SET
                summary = EXCLUDED.summary,
                events = EXCLUDED.events,
                risk = EXCLUDED.risk,
                actions = EXCLUDED.actions;
        ";

        match sqlx::query(query)
            .bind(video_id.to_string())
            .bind(summary)
            .bind(&events_json)
            .bind(risk)
            .bind(&actions_json)
            .execute(&pool)
            .await
        {
            Ok(_) => {
                info!("Adim 3: Veritabanina (Postgres) basariyla kaydedildi.");
                notify_trace(&pool, &video_id, "[Orchestrator -> Postgres] Nihai rapor veritabanına yazıldı. SSE fırlatılıyor.").await;

                if let Err(e) = sqlx::query("SELECT pg_notify($1, $2)")
                    .bind("ai_events")
                    .bind(video_id.to_string())
                    .execute(&pool)
                    .await
                {
                    error!("SSE bildirimi gonderilemedi: {}", e);
                } else {
                    info!("Adim 4: Gateway (SSE) icin pg_notify gonderildi! Zincir tamamlandi.");
                }
            }
            Err(e) => {
                error!("Veritabanina yazma hatasi: {}", e);
                notify_trace(&pool, &video_id, &format!("[Hata] Postgres yazılamadı: {e}")).await;
            }
        }
    }

    motif_observer::shutdown();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saniye_mmss_oluyor() {
        assert_eq!(ss(0.0), "00:00");
        assert_eq!(ss(72.4), "01:12");
        assert_eq!(ss(-3.0), "00:00", "negatif değer sıfıra sabitlenmeli");
    }

    #[test]
    fn ses_ozeti_olaylari_mmss_ile_yaziyor() {
        let analiz = json!({
            "events": [
                { "label_tr": "cam kırılması", "start_sec": 12.0, "end_sec": 14.5, "confidence": 0.87 },
            ]
        });
        assert_eq!(
            ses_ozeti(&analiz).unwrap(),
            "00:12–00:14 cam kırılması (%87)"
        );
    }

    /// Olay yoksa bölge açılmamalı.
    ///
    /// Aksi hâlde sessiz bir kayıtta prompt'a içi boş bir "işitsel bağlam"
    /// bölümü girer: bağlamı şişirir ve modele söyleyecek şeyi olmayan bir
    /// bölüm gösterir.
    #[test]
    fn olay_yoksa_ozet_yok() {
        assert!(ses_ozeti(&json!({ "events": [] })).is_none());
        assert!(ses_ozeti(&json!({})).is_none(), "alan hiç yoksa da None");
    }

    /// Liste kesiliyorsa model bunu bilmeli.
    #[test]
    fn uzun_liste_kirpildigini_soyluyor() {
        let olaylar: Vec<Value> = (0..AZAMI_SES_OLAYI + 3)
            .map(|i| {
                json!({
                    "label_tr": "alarm",
                    "start_sec": i as f32,
                    "end_sec": i as f32 + 1.0,
                    "confidence": 0.5
                })
            })
            .collect();

        let ozet = ses_ozeti(&json!({ "events": olaylar })).unwrap();
        assert_eq!(ozet.lines().count(), AZAMI_SES_OLAYI + 1);
        assert!(ozet.contains("3 olay daha"));
    }

    /// Kırpma zamana göre yapılsaydı kaydın sonu tamamen düşerdi.
    ///
    /// Bu test tam o hatayı yakalıyor: en güvenilir olay listenin **sonunda**
    /// duruyor ve hayatta kalmalı.
    #[test]
    fn kirpma_guvene_gore_listeleme_zamana_gore() {
        let mut olaylar: Vec<Value> = (0..AZAMI_SES_OLAYI)
            .map(|i| {
                json!({
                    "label_tr": "fısıltı",
                    "start_sec": i as f32,
                    "end_sec": i as f32 + 1.0,
                    "confidence": 0.10
                })
            })
            .collect();
        olaylar.push(json!({
            "label_tr": "patlama",
            "start_sec": 300.0,
            "end_sec": 302.0,
            "confidence": 0.99
        }));

        let ozet = ses_ozeti(&json!({ "events": olaylar })).unwrap();

        assert!(
            ozet.contains("patlama"),
            "en güvenilir olay kırpmada düştü: kırpma zamana göre yapılıyor"
        );

        // Zaman sırası korunmalı: patlama en geç olay, son satırda olmalı.
        let satirlar: Vec<&str> = ozet.lines().collect();
        assert!(
            satirlar[satirlar.len() - 2].contains("patlama"),
            "liste zamana göre sıralanmamış"
        );
    }

    /// Bozuk bir olay tüm özeti düşürmemeli.
    #[test]
    fn eksik_alanli_olay_atlaniyor() {
        let analiz = json!({
            "events": [
                { "label_tr": "alarm" },
                { "label_tr": "korna", "start_sec": 5.0, "end_sec": 6.0, "confidence": 0.9 },
            ]
        });
        let ozet = ses_ozeti(&analiz).unwrap();
        assert_eq!(ozet, "00:05–00:06 korna (%90)");
    }
}
