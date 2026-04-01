use super::{CanonicalChatRequest, CanonicalChatResponse, CanonicalMessage};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AnthropicRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessage>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub stream: Option<bool>,
    pub system: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub response_type: String,
    pub role: String,
    pub content: Vec<AnthropicContent>,
    pub model: String,
    pub stop_reason: Option<String>,
    pub usage: Option<AnthropicUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicContent {
    #[serde(rename = "text")]
    Text { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

impl AnthropicRequest {
    pub fn to_canonical(&self) -> CanonicalChatRequest {
        let mut messages: Vec<CanonicalMessage> = self
            .messages
            .iter()
            .map(|m| CanonicalMessage {
                role: m.role.clone(),
                content: m.content.clone(),
                name: None,
            })
            .collect();

        if let Some(system) = &self.system {
            messages.insert(
                0,
                CanonicalMessage {
                    role: "system".to_string(),
                    content: system.clone(),
                    name: None,
                },
            );
        }

        CanonicalChatRequest {
            model: self.model.clone(),
            messages,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            stream: self.stream,
            top_p: self.top_p,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
        }
    }
}

impl From<CanonicalChatResponse> for AnthropicResponse {
    fn from(c: CanonicalChatResponse) -> Self {
        let content: Vec<AnthropicContent> = c
            .choices
            .iter()
            .map(|ch| AnthropicContent::Text {
                text: ch.message.content.clone(),
            })
            .collect();

        AnthropicResponse {
            id: c.id,
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            content,
            model: c.model,
            stop_reason: c.choices.first().and_then(|ch| ch.finish_reason.clone()),
            usage: c.usage.map(|u| AnthropicUsage {
                input_tokens: u.prompt_tokens,
                output_tokens: u.completion_tokens,
            }),
        }
    }
}
