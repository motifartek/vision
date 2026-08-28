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

        if let Some(start) = clean_text.find("[TOOL_CALL]") {
            let json_str = clean_text[start + 11..].trim();
            
            // LLM'ler JSON'ı bazen markdown code block içine alabiliyor (```json ... ```).
            // Bu yüzden ilk '{' ile son '}' arasını almak en güvenlisidir.
            if let (Some(first_brace), Some(last_brace)) = (json_str.find('{'), json_str.rfind('}')) {
                if first_brace <= last_brace {
                    let valid_json = &json_str[first_brace..=last_brace];
                    match serde_json::from_str::<serde_json::Value>(valid_json) {
                        Ok(parsed) => {
                            tracing::info!("Araç çağrısı yakalandı: {}", parsed);
                            tools.push(parsed);
                        }
                        Err(e) => {
                            tracing::error!("Araç çağrısı JSON parse hatası: {}. JSON str: '{}'", e, valid_json);
                        }
                    }
                }
            } else {
                tracing::warn!("Araç çağrısı JSON objesi sınırları ({{, }}) bulunamadı: {}", json_str);
            }
            clean_text.truncate(start);
        }
        (clean_text.trim().to_string(), tools)
    }

    pub async fn enhance_report(&self, report_json: &str, tools: Option<String>) -> anyhow::Result<(String, Vec<serde_json::Value>, String)> {
        let mut ctx = PromptContext::new(0).with_audio(motif_prompt::UntrustedText::new(report_json));
        let has_tools = tools.is_some();
        if let Some(t) = tools {
            ctx = ctx.with_tools(Some(t));
        }
        let p = self.preview(PromptKind::HumanizerEnhance, &ctx);
        let prompt_text = p.joined();
        
        let user_prompt = if has_tools {
            "İşte analiz raporu. Durumu incele ve profesyonelce açıkla. Eğer müdahale için bir araç kullanman GEREKİYORSA, cevabının EN SON SATIRINA KESİNLİKLE şu formatta ilgili aracı ekle:\n[TOOL_CALL]\n{\"action\": \"arac_ismi\", \"params\": {}}"
        } else {
            "İşte analiz raporu:"
        };

        let text = self.llm.generate(&prompt_text, user_prompt, &[]).await?;
        let (clean, extracted_tools) = self.extract_tools(&text).await;
        Ok((clean, extracted_tools, prompt_text))
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
