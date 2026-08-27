use futures::StreamExt;
use motif_event_sdk::{subjects, messages::ToolExecuteRequest};
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    motif_observer::init("toolbox");
    info!("Toolbox servisi baslatiliyor...");

    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://root:root@localhost:5432/motif".into());
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

    let mut subscriber = nats_client.subscribe(subjects::TOOL_EXECUTE).await?;
    info!("Toolbox worker hazir. {} dinleniyor...", subjects::TOOL_EXECUTE);

    while let Some(message) = subscriber.next().await {
        if let Ok(req) = serde_json::from_slice::<ToolExecuteRequest>(&message.payload) {
            info!("Yeni arac calistirma istegi alindi: {:?}", req.tool_name);
            
            // Mock islem: 1-2 saniye bekliyor gibi yap
            tokio::time::sleep(Duration::from_millis(1500)).await;
            
            // Veritabanindan basligi (title) cekip gercekci bir mesaj uretelim
            let title = match sqlx::query("SELECT title FROM external_tools WHERE name = $1")
                .bind(&req.tool_name)
                .fetch_optional(&pool)
                .await
            {
                Ok(Some(row)) => {
                    use sqlx::Row;
                    row.try_get::<String, _>("title").unwrap_or_else(|_| req.tool_name.clone())
                }
                _ => req.tool_name.clone(), // bulunamazsa ham adini kullan
            };

            info!("Mock fonksiyon calisti: {}", title);

            // Frontend'e firlatmak icin postgres'e pg_notify(tool_alerts) yolla
            let payload = serde_json::json!({
                "video_id": req.video_id,
                "tool_name": req.tool_name,
                "title": title,
                "message": format!("🚨 Dış Sistem Uyarıldı: {}", title),
                "payload": req.payload
            }).to_string();

            let query = format!("NOTIFY tool_alerts, '{}'", payload.replace("'", "''"));
            if let Err(e) = sqlx::query(&query).execute(&pool).await {
                error!("Tool alert bildirimi gonderilemedi: {}", e);
            } else {
                info!("Gateway'e tool_alerts uyarisi firlatildi.");
            }
        } else {
            warn!("Geçersiz ToolExecuteRequest mesaji.");
        }
    }

    motif_observer::shutdown();
    Ok(())
}
