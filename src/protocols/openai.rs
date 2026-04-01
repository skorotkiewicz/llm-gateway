use super::{CanonicalChatRequest, CanonicalChatResponse, CanonicalMessage};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenAIRequest {
    pub model: String,
    pub messages: Vec<OpenAIMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stream: Option<bool>,
    pub top_p: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub stop: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenAIMessage {
    pub role: String,
    pub content: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OpenAIChoice>,
    pub usage: Option<OpenAIUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIChoice {
    pub index: u32,
    pub message: OpenAIMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

impl OpenAIRequest {
    pub fn to_canonical(&self) -> CanonicalChatRequest {
        CanonicalChatRequest {
            model: self.model.clone(),
            messages: self
                .messages
                .iter()
                .map(|m| CanonicalMessage {
                    role: m.role.clone(),
                    content: m.content.clone(),
                    name: m.name.clone(),
                })
                .collect(),
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            stream: self.stream,
            top_p: self.top_p,
            frequency_penalty: self.frequency_penalty,
            presence_penalty: self.presence_penalty,
            stop: self.stop.clone(),
        }
    }
}

impl From<CanonicalChatResponse> for OpenAIResponse {
    fn from(c: CanonicalChatResponse) -> Self {
        OpenAIResponse {
            id: c.id,
            object: c.object,
            created: c.created,
            model: c.model,
            choices: c
                .choices
                .into_iter()
                .map(|ch| OpenAIChoice {
                    index: ch.index,
                    message: OpenAIMessage {
                        role: ch.message.role,
                        content: ch.message.content,
                        name: ch.message.name,
                    },
                    finish_reason: ch.finish_reason,
                })
                .collect(),
            usage: c.usage.map(|u| OpenAIUsage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            }),
        }
    }
}
