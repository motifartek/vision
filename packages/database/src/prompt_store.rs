//! Prompt override'larının Postgres uygulaması.
//!
//! Doğrulama burada **yapılmıyor**: kural `packages/prompt` içinde, tek yerde.
//! Bu katman yalnızca kalıcılık.

use chrono::{DateTime, Utc};
use motif_prompt::{PromptOverride, PromptStore, StoreError};
use sqlx::PgPool;

pub struct PostgresPromptStore {
    pool: PgPool,
}

impl PostgresPromptStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn hata(e: sqlx::Error) -> StoreError {
    StoreError::Backend(e.to_string())
}

#[async_trait::async_trait]
impl PromptStore for PostgresPromptStore {
    async fn list(&self) -> Result<Vec<PromptOverride>, StoreError> {
        let satirlar: Vec<(String, String, String, String, DateTime<Utc>)> = sqlx::query_as(
            "SELECT agent, fragment, text, author, updated_at FROM prompt_override",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(hata)?;

        Ok(satirlar
            .into_iter()
            .map(|(agent, fragment, text, author, updated_at)| PromptOverride {
                // Birincil anahtar (agent, fragment); kimlik ondan türetiliyor.
                id: format!("{agent}/{fragment}"),
                agent,
                fragment,
                text,
                author,
                updated_at: updated_at.to_rfc3339(),
            })
            .collect())
    }

    async fn put(&self, o: PromptOverride) -> Result<(), StoreError> {
        sqlx::query(
            r#"
            INSERT INTO prompt_override (agent, fragment, text, author, updated_at)
            VALUES ($1, $2, $3, $4, now())
            ON CONFLICT (agent, fragment)
            DO UPDATE SET text = EXCLUDED.text,
                          author = EXCLUDED.author,
                          updated_at = now()
            "#,
        )
        .bind(&o.agent)
        .bind(&o.fragment)
        .bind(&o.text)
        .bind(&o.author)
        .execute(&self.pool)
        .await
        .map_err(hata)?;
        Ok(())
    }

    async fn delete(&self, agent: &str, fragment: &str) -> Result<(), StoreError> {
        sqlx::query("DELETE FROM prompt_override WHERE agent = $1 AND fragment = $2")
            .bind(agent)
            .bind(fragment)
            .execute(&self.pool)
            .await
            .map_err(hata)?;
        Ok(())
    }
}
