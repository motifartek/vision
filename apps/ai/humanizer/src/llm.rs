use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("Ağ hatası: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Servis hatası: {0}")]
    Service(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    async fn generate(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        history: &[ChatMessage],
    ) -> Result<String, LlmError>;
}

pub struct EvrenProvider {
    client: reqwest::Client,
    base_url: String,
    model: String,
    api_key: String,
}

impl EvrenProvider {
    pub fn from_env() -> anyhow::Result<Self> {
        let base_url = std::env::var("EVREN_BASE_URL")
            .unwrap_or_else(|_| "https://evren-llmapi.ssyz.org.tr/v1".into());
        let model = std::env::var("EVREN_MODEL").unwrap_or_else(|_| "llm-fast".into());
        let api_key = std::env::var("EVREN_KEY").map_err(|_| {
            anyhow::anyhow!("EVREN_KEY ortam değişkeni tanımlı değil.")
        })?;

        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()?,
            base_url,
            model,
            api_key,
        })
    }
}

#[async_trait::async_trait]
impl LlmProvider for EvrenProvider {
    async fn generate(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        history: &[ChatMessage],
    ) -> Result<String, LlmError> {
        let mut messages = vec![
            json!({"role": "system", "content": system_prompt}),
        ];
        
        for msg in history {
            messages.push(json!({"role": &msg.role, "content": &msg.content}));
        }

        messages.push(json!({"role": "user", "content": user_prompt}));

        let payload = json!({
            "model": self.model,
            "messages": messages,
            "temperature": 0.7,
        });

        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(LlmError::Service(format!("{status}: {text}")));
        }

        #[derive(Deserialize)]
        struct Completion {
            choices: Vec<Choice>,
        }
        #[derive(Deserialize)]
        struct Choice {
            message: Message,
        }
        #[derive(Deserialize)]
        struct Message {
            content: Option<String>,
        }

        let parsed: Completion = resp.json().await?;
        let text = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .unwrap_or_default();

        Ok(text)
    }
}