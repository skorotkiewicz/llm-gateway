use super::{CanonicalChatRequest, CanonicalChatResponse, CanonicalMessage};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OllamaRequest {
    pub model: String,
    pub prompt: Option<String>,
    pub messages: Option<Vec<OllamaMessage>>,
    pub system: Option<String>,
    pub stream: Option<bool>,
    pub options: Option<OllamaOptions>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OllamaMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OllamaOptions {
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub num_predict: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaResponse {
    pub model: String,
    pub response: Option<String>,
    pub done: bool,
}

impl OllamaRequest {
    pub fn to_canonical(&self) -> CanonicalChatRequest {
        let mut messages = vec![];

        if let Some(system) = &self.system {
            messages.push(CanonicalMessage {
                role: "system".to_string(),
                content: system.clone(),
                name: None,
            });
        }

        if let Some(ollama_messages) = &self.messages {
            for msg in ollama_messages {
                messages.push(CanonicalMessage {
                    role: msg.role.clone(),
                    content: msg.content.clone(),
                    name: None,
                });
            }
        } else if let Some(prompt) = &self.prompt {
            messages.push(CanonicalMessage {
                role: "user".to_string(),
                content: prompt.clone(),
                name: None,
            });
        }

        let (temperature, top_p, max_tokens) = self
            .options
            .as_ref()
            .map(|o| (o.temperature, o.top_p, o.num_predict.map(|n| n as u32)))
            .unwrap_or((None, None, None));

        CanonicalChatRequest {
            model: self.model.clone(),
            messages,
            temperature,
            max_tokens,
            stream: self.stream,
            top_p,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
        }
    }
}

impl From<&CanonicalChatResponse> for OllamaResponse {
    fn from(c: &CanonicalChatResponse) -> Self {
        let content = c
            .choices
            .first()
            .map(|ch| ch.message.content.clone())
            .unwrap_or_default();

        OllamaResponse {
            model: c.model.clone(),
            response: Some(content),
            done: true,
        }
    }
}
