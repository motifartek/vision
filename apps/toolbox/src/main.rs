use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use futures::StreamExt;
use motif_event_sdk::{messages::ToolExecuteRequest, subjects};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use std::time::Duration;
use tower_http::cors::CorsLayer;
use tracing::{error, info, warn};

#[derive(Clone)]
struct AppState {
    pool: PgPool,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ExternalTool {
    #[serde(default)]
    id: i32,
    name: String,
    title: String,
    description: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    motif_observer::init("toolbox");
    info!("Toolbox servisi baslatiliyor...");

    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://motif:motif@127.0.0.1:5433/motif".into());
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;
    info!("PostgreSQL'e basariyla baglanildi.");

    let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    let nats_client = async_nats::connect(&nats_url).await?;
    info!("NATS'a baglanildi!");

    // NATS Worker arka planda
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        let mut subscriber = nats_client.subscribe(subjects::TOOL_EXECUTE).await.unwrap();
        info!("Toolbox worker hazir. {} dinleniyor...", subjects::TOOL_EXECUTE);

        while let Some(message) = subscriber.next().await {
            if let Ok(req) = serde_json::from_slice::<ToolExecuteRequest>(&message.payload) {
                info!("Yeni arac calistirma istegi alindi: {:?}", req.tool_name);
                tokio::time::sleep(Duration::from_millis(1500)).await;

                let title = match sqlx::query("SELECT title FROM external_tools WHERE name = $1")
                    .bind(&req.tool_name)
                    .fetch_optional(&pool_clone)
                    .await
                {
                    Ok(Some(row)) => row.try_get::<String, _>("title").unwrap_or_else(|_| req.tool_name.clone()),
                    _ => req.tool_name.clone(),
                };

                info!("Mock fonksiyon calisti: {}", title);

                let payload = serde_json::json!({
                    "video_id": req.video_id,
                    "tool_name": req.tool_name,
                    "title": title,
                    "message": format!("🚨 Dış Sistem Uyarıldı: {}", title),
                    "payload": req.payload
                }).to_string();

                let query = format!("NOTIFY tool_alerts, '{}'", payload.replace("'", "''"));
                if let Err(e) = sqlx::query(&query).execute(&pool_clone).await {
                    error!("Tool alert bildirimi gonderilemedi: {}", e);
                } else {
                    info!("Gateway'e tool_alerts uyarisi firlatildi.");
                }
            } else {
                warn!("Gecersiz ToolExecuteRequest mesaji.");
            }
        }
    });

    // REST API Onde
    let state = AppState { pool };
    let app = Router::new()
        .route("/v1/tools", get(list_tools).post(create_tool))
        .route("/v1/tools/{name}", put(update_tool).delete(delete_tool))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let bind = std::env::var("TOOLBOX_BIND").unwrap_or_else(|_| "0.0.0.0:8115".into());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    info!("Toolbox API {} uzerinde dinliyor", bind);

    axum::serve(listener, app).await?;

    motif_observer::shutdown();
    Ok(())
}

async fn list_tools(State(state): State<AppState>) -> impl IntoResponse {
    let rows = sqlx::query("SELECT id, name, title, description FROM external_tools ORDER BY id ASC")
        .fetch_all(&state.pool)
        .await;

    match rows {
        Ok(rows) => {
            let tools: Vec<ExternalTool> = rows.into_iter().map(|row| ExternalTool {
                id: row.get("id"),
                name: row.get("name"),
                title: row.get("title"),
                description: row.get("description"),
            }).collect();
            (StatusCode::OK, Json(serde_json::json!({ "tools": tools }))).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
    }
}

async fn create_tool(State(state): State<AppState>, Json(tool): Json<ExternalTool>) -> impl IntoResponse {
    let res = sqlx::query("INSERT INTO external_tools (name, title, description) VALUES ($1, $2, $3)")
        .bind(&tool.name)
        .bind(&tool.title)
        .bind(&tool.description)
        .execute(&state.pool)
        .await;
    
    match res {
        Ok(_) => StatusCode::CREATED.into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response()
    }
}

async fn update_tool(State(state): State<AppState>, Path(name): Path<String>, Json(tool): Json<ExternalTool>) -> impl IntoResponse {
    let res = sqlx::query("UPDATE external_tools SET title = $1, description = $2 WHERE name = $3")
        .bind(&tool.title)
        .bind(&tool.description)
        .bind(&name)
        .execute(&state.pool)
        .await;
    
    match res {
        Ok(done) if done.rows_affected() > 0 => StatusCode::OK.into_response(),
        Ok(_) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
    }
}

async fn delete_tool(State(state): State<AppState>, Path(name): Path<String>) -> impl IntoResponse {
    let res = sqlx::query("DELETE FROM external_tools WHERE name = $1")
        .bind(&name)
        .execute(&state.pool)
        .await;
    
    match res {
        Ok(done) if done.rows_affected() > 0 => StatusCode::NO_CONTENT.into_response(),
        Ok(_) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
    }
}
