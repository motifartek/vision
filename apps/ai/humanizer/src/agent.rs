use std::sync::Arc;
use motif_prompt::{PromptContext, PromptKind, PromptRegistry, RenderedPrompt};
use crate::llm::LlmProvider;
use async_nats::Client;
use crate::db::ChatStore;

pub struct HumanizerAgent {
    llm: Arc<dyn LlmProvider>,
    prompts: Arc<PromptRegistry>,
    nats: Client,
    chat_store: Arc<ChatStore>,
}

impl HumanizerAgent {
    pub fn new(llm: Arc<dyn LlmProvider>, prompts: Arc<PromptRegistry>, nats: Client, chat_store: Arc<ChatStore>) -> Self {
        Self { llm, prompts, nats, chat_store }
    }

    pub fn prompts(&self) -> &Arc<PromptRegistry> {
        &self.prompts
    }

    pub fn preview(&self, kind: PromptKind, ctx: &PromptContext) -> RenderedPrompt {
        self.prompts.render(kind, ctx)
    }
    
    async fn extract_tools(&self, text: &str) -> (String, Vec<serde_json::Value>) {
        let mut clean_text = text.to_string();
        let mut tools = Vec::new();

        while let Some(start) = clean_text.find("<tool_call>") {
            if let Some(end_offset) = clean_text[start..].find("</tool_call>") {
                let end = start + end_offset;
                let json_str = &clean_text[start + 11..end];
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                    tracing::info!("Araç çağrısı yakalandı: {}", parsed);
                    tools.push(parsed);
                }
                let block_end = end + 12;
                clean_text.replace_range(start..block_end, "");
            } else {
                break;
            }
        }
        (clean_text.trim().to_string(), tools)
    }

    pub async fn enhance_report(&self, report_json: &str, tools: Option<String>) -> anyhow::Result<(String, Vec<serde_json::Value>)> {
        let mut ctx = PromptContext::new(0).with_audio(motif_prompt::UntrustedText::new(report_json));
        if let Some(t) = tools {
            ctx = ctx.with_tools(Some(t));
        }
        let p = self.preview(PromptKind::HumanizerEnhance, &ctx);
        
        let text = self.llm.generate(&p.joined(), "İşte analiz raporu:", &[]).await?;
        Ok(self.extract_tools(&text).await)
    }

    pub async fn generate_document(&self, report_json: &str, kind: PromptKind) -> anyhow::Result<(String, Vec<serde_json::Value>)> {
        let ctx = PromptContext::new(0).with_audio(motif_prompt::UntrustedText::new(report_json));
        let p = self.preview(kind, &ctx);
        let text = self.llm.generate(&p.joined(), "İşte analiz raporu:", &[]).await?;
        Ok(self.extract_tools(&text).await)
    }

    pub async fn chat(&self, session_id: &str, video_id: &str, user_message: &str, tools: Option<String>) -> anyhow::Result<String> {
        // Oturum yoksa oluştur
        self.chat_store.create_session(session_id, video_id).await?;
        
        // Kullanıcı mesajını kaydet
        self.chat_store.add_message(session_id, "user", user_message).await?;
        
        // Geçmişi getir
        let history = self.chat_store.get_history(session_id).await?;

        let mut ctx = PromptContext::new(0);
        if let Some(t) = tools {
            ctx = ctx.with_tools(Some(t));
        }
        let p = self.preview(PromptKind::HumanizerChat, &ctx);
        
        // LLM'e gönder
        let text = self.llm.generate(&p.joined(), user_message, &history).await?;
        
        let (clean, tools_parsed) = self.extract_tools(&text).await;
        
        // Eğer araç onaylanırsa, arayüzden istek gönderilip tetiklenecek.
        // Mesajı kaydet
        self.chat_store.add_message(session_id, "assistant", &clean).await?;
        Ok(clean)
    }
}
