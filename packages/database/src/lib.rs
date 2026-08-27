pub mod qdrant;
pub mod postgres;

use std::sync::Arc;

#[derive(Clone)]
pub struct DatabaseClients {
    pub postgres: postgres::PostgresDb,
    pub qdrant: Arc<qdrant_client::Qdrant>,
}

impl DatabaseClients {
    /// Her iki veritabanina baglanir ve istemcileri dondurur.
    pub async fn connect(
        postgres_url: &str,
        qdrant_url: &str,
        qdrant_api_key: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let pg = postgres::PostgresDb::connect(postgres_url).await?;
        let qd = qdrant::connect(qdrant_url, qdrant_api_key).await?;

        Ok(Self {
            postgres: pg,
            qdrant: Arc::new(qd),
        })
    }
}
