use futures::StreamExt;
use motif_event_sdk::{subjects, VideoIngested};
use std::time::Duration;
use reqwest::Client;
use serde_json::{json, Value};
use tracing::{info, warn, error};
use sqlx::postgres::PgPoolOptions;

async fn notify_trace(pool: &sqlx::PgPool, video_id: impl std::fmt::Display, status: &str) {
    let payload = json!({ "video_id": video_id.to_string(), "message": status }).to_string();
    let query = format!("NOTIFY ai_trace, '{}'", payload.replace("'", "''"));
    let _ = sqlx::query(&query).execute(pool).await;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    motif_observer::init("orchestrator");
    info!("Orchestrator (Macro Loop) baslatiliyor...");

    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://motif:motif@127.0.0.1:5433/motif".into());
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
    let sonic_url = std::env::var("SONIC_URL").unwrap_or_else(|_| "http://127.0.0.1:8120".into());
    let vision_url = std::env::var("VISION_URL").unwrap_or_else(|_| "http://127.0.0.1:8110".into());

    let mut subscriber = nats_client.subscribe(subjects::VIDEO_INGESTED).await?;
    info!("Macro Loop hazir. {} dinleniyor...", subjects::VIDEO_INGESTED);

    while let Some(message) = subscriber.next().await {
        if let Ok(event) = serde_json::from_slice::<VideoIngested>(&message.payload) {
            let video_id = event.video_id;
            info!("YENI TETIKLEYICI ALINDI: Video {} isleme aliniyor.", video_id);
            notify_trace(&pool, &video_id, "[Orchestrator] Yeni video yüklendi. NATS mesajı yakalandı.").await;

            // ADIM 1: Sonic'ten isitsel baglam iste
            info!("Adim 1: Sonic (Ses) analizine basvuruluyor...");
            notify_trace(&pool, &video_id, "[Orchestrator -> Sonic] Adım 1: İşitsel (Ses) analiz başlatılıyor...").await;
            
            let _sonic_resp = http_client.post(format!("{}/v1/analyze", sonic_url))
                .json(&json!({ "video_id": video_id }))
                .send()
                .await;

            notify_trace(&pool, &video_id, "[Sonic -> Orchestrator] İşitsel analiz tamamlandı.").await;

            // ADIM 2: Vision ajanindan analiz iste
            info!("Adim 2: Vision (VLM) ajanindan nihai analiz isteniyor...");
            notify_trace(&pool, &video_id, "[Orchestrator -> Vision] Adım 2: Görsel (VLM) ajanı tetiklendi. Video izleniyor...").await;

            let vision_req = http_client.post(format!("{}/v1/analyze", vision_url))
                .json(&json!({ "video_id": video_id }))
                .send()
                .await;

            match vision_req {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(report) = resp.json::<Value>().await {
                        info!("Vision Ajanindan rapor basariyla alindi!");
                        notify_trace(&pool, &video_id, "[Vision -> Orchestrator] Görsel analiz başarıyla alındı ve birleştirildi.").await;
                        
                        // ADIM 3: Veritabanina kaydet ve SSE (pg_notify) firlat
                        let summary = report["summary"].as_str().unwrap_or("");
                        let risk = report["risk"].as_str().unwrap_or("Dusuk");

                        let query = "
                            INSERT INTO ai_events (video_id, summary, events, risk, actions)
                            VALUES ($1, $2, $3, $4, $5)
                            ON CONFLICT (video_id) DO UPDATE SET
                                summary = EXCLUDED.summary,
                                events = EXCLUDED.events,
                                risk = EXCLUDED.risk,
                                actions = EXCLUDED.actions;
                        ";

                        let events_json = report["events"].clone();
                        let actions_json = report["actions"].clone();

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
                                let notify_query = format!("NOTIFY ai_events, '{}'", video_id);
                                let _ = sqlx::query(&notify_query).execute(&pool).await;
                                info!("Adim 4: Gateway (SSE) icin pg_notify gonderildi! Zincir tamamlandi.");
                            }
                            Err(e) => {
                                error!("Veritabanina yazma hatasi: {}", e);
                                notify_trace(&pool, &video_id, &format!("[Hata] Postgres yazılamadı: {}", e)).await;
                            }
                        }
                    }
                }
                Ok(resp) => {
                    error!("Vision servisi hata kodu dondu: {}", resp.status());
                    notify_trace(&pool, &video_id, &format!("[Hata] Vision servisi {} döndü.", resp.status())).await;
                }
                Err(e) => {
                    error!("Vision servisine ulasilamadi: {}", e);
                    notify_trace(&pool, &video_id, &format!("[Hata] Vision servisine ulaşılamadı: {}", e)).await;
                }
            }
        }
    }

    motif_observer::shutdown();
    Ok(())
}
