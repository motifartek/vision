pub mod qdrant;
pub mod surreal;

use std::sync::Arc;

#[derive(Clone)]
pub struct DatabaseClients {
    pub surreal: Arc<surrealdb::Surreal<surrealdb::engine::remote::ws::Client>>,
    pub qdrant: Arc<qdrant_client::Qdrant>,
}

impl DatabaseClients {
    /// Her iki veritabanina baglanir ve istemcileri dondurur.
    pub async fn connect(
        surreal_url: &str,
        surreal_ns: &str,
        surreal_db: &str,
        qdrant_url: &str,
        qdrant_api_key: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let surreal = surreal::connect(surreal_url, surreal_ns, surreal_db).await?;
        let qdrant = qdrant::connect(qdrant_url, qdrant_api_key).await?;

        Ok(Self {
            surreal: Arc::new(surreal),
            qdrant: Arc::new(qdrant),
        })
    }
}