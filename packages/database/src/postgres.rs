use sqlx::postgres::{PgPool, PgPoolOptions};
use std::sync::Arc;
use tracing::info;

#[derive(Clone)]
pub struct PostgresDb {
    pub pool: Arc<PgPool>,
}

impl PostgresDb {
    pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
        info!("PostgreSQL'e baglaniliyor: {}", url);
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(url)
            .await?;
        
        info!("PostgreSQL baglantisi basarili.");
        Ok(Self {
            pool: Arc::new(pool),
        })
    }
}
