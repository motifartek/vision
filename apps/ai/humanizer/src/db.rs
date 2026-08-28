use sqlx::{PgPool, Row};
use crate::llm::ChatMessage;

pub struct ChatStore {
    pool: PgPool,
}

impl ChatStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_session(&self, session_id: &str, video_id: &str) -> anyhow::Result<()> {
        sqlx::query("INSERT INTO chat_sessions (id, video_id) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING")
            .bind(session_id)
            .bind(video_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn add_message(&self, session_id: &str, role: &str, content: &str) -> anyhow::Result<()> {
        sqlx::query("INSERT INTO chat_messages (session_id, role, content) VALUES ($1, $2, $3)")
            .bind(session_id)
            .bind(role)
            .bind(content)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_history(&self, session_id: &str) -> anyhow::Result<Vec<ChatMessage>> {
        let rows = sqlx::query("SELECT role, content FROM chat_messages WHERE session_id = $1 ORDER BY created_at ASC")
            .bind(session_id)
            .fetch_all(&self.pool)
            .await?;
        
        let mut history = Vec::new();
        for row in rows {
            history.push(ChatMessage {
                role: row.try_get("role")?,
                content: row.try_get("content")?,
            });
        }
        Ok(history)
    }
}