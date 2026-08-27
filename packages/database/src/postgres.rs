//! Postgres havuzu ve şema kurulumu.

use std::time::Duration;

pub use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("veritabanına bağlanılamadı: {0}")]
    Connect(#[source] sqlx::Error),
    #[error("şema kurulamadı: {0}")]
    Migrate(#[source] sqlx::Error),
}

/// Bağlanır ve gereken tabloları kurar.
///
/// Ayrı bir göç aracı yok: tek tablo ve `IF NOT EXISTS` yeterli. Şema
/// büyürse `sqlx::migrate!` devreye alınmalı.
pub async fn connect(url: &str) -> Result<PgPool, DbError> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        // Kısa: veritabanı yoksa servis beklemeden gömülü katalogla açılmalı.
        .acquire_timeout(Duration::from_secs(3))
        .connect(url)
        .await
        .map_err(DbError::Connect)?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS prompt_override (
            agent       TEXT NOT NULL,
            fragment    TEXT NOT NULL,
            text        TEXT NOT NULL,
            author      TEXT NOT NULL,
            updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
            PRIMARY KEY (agent, fragment)
        );
        "#,
    )
    .execute(&pool)
    .await
    .map_err(DbError::Migrate)?;

    tracing::info!("Postgres hazır, prompt_override tablosu kuruldu");
    Ok(pool)
}
