use serde::{Deserialize, Serialize};

// OpenAI-compatible models

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenAIChatRequest {
    pub model: String,
    pub messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenAIMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenAIChatResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OpenAIChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<OpenAIUsage>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenAIChoice {
    pub index: u32,
    pub message: OpenAIMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenAIUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenAIStreamChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OpenAIStreamChoice>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenAIStreamChoice {
    pub index: u32,
    pub delta: OpenAIDelta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct OpenAIDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

// Anthropic models

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AnthropicRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
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

// Request source detection
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RequestSource {
    OpenAI,
    Anthropic,
    Zai,
    Unknown,
}

impl RequestSource {
    pub fn from_headers(headers: &axum::http::HeaderMap) -> Self {
        // Check for specific headers to identify the source
        if let Some(auth) = headers.get("authorization") {
            if let Ok(auth_str) = auth.to_str() {
                if auth_str.contains("anthropic") {
                    return RequestSource::Anthropic;
                }
            }
        }

        // Check X-Source header or User-Agent
        if let Some(source) = headers.get("x-source") {
            if let Ok(source_str) = source.to_str() {
                match source_str.to_lowercase().as_str() {
                    "anthropic" => return RequestSource::Anthropic,
                    "openai" => return RequestSource::OpenAI,
                    "zai" => return RequestSource::Zai,
                    _ => {}
                }
            }
        }

        // Check User-Agent
        if let Some(ua) = headers.get("user-agent") {
            if let Ok(ua_str) = ua.to_str() {
                let ua_lower = ua_str.to_lowercase();
                if ua_lower.contains("anthropic") {
                    return RequestSource::Anthropic;
                }
                if ua_lower.contains("openai") {
                    return RequestSource::OpenAI;
                }
            }
        }

        // Check request path
        if let Some(path) = headers.get(":path") {
            if let Ok(path_str) = path.to_str() {
                if path_str.contains("anthropic") {
                    return RequestSource::Anthropic;
                }
                if path_str.contains("openai") || path_str.contains("chat/completions") {
                    return RequestSource::OpenAI;
                }
            }
        }

        RequestSource::Unknown
    }
}

// Conversion functions

impl AnthropicRequest {
    pub fn to_openai(self) -> OpenAIChatRequest {
        let mut messages = self.messages.clone();

        // Convert system message if present
        if let Some(system) = self.system {
            messages.insert(
                0,
                AnthropicMessage {
                    role: "system".to_string(),
                    content: system,
                },
            );
        }

        // Convert Anthropic messages to OpenAI format
        let openai_messages: Vec<OpenAIMessage> = messages
            .into_iter()
            .map(|m| OpenAIMessage {
                role: match m.role.as_str() {
                    "assistant" => "assistant".to_string(),
                    "user" => "user".to_string(),
                    _ => m.role,
                },
                content: m.content,
                name: None,
            })
            .collect();

        OpenAIChatRequest {
            model: self.model,
            messages: openai_messages,
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

impl OpenAIChatResponse {
    pub fn to_anthropic(self) -> AnthropicResponse {
        let content: Vec<AnthropicContent> = self
            .choices
            .iter()
            .map(|choice| AnthropicContent::Text {
                text: choice.message.content.clone(),
            })
            .collect();

        let usage = self.usage.map(|u| AnthropicUsage {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
        });

        AnthropicResponse {
            id: self.id,
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            content,
            model: self.model,
            stop_reason: self.choices.first().and_then(|c| c.finish_reason.clone()),
            usage,
        }
    }
}

// Zai format - uses OpenAI-compatible format with slight variations
pub type ZaiRequest = OpenAIChatRequest;
pub type ZaiResponse = OpenAIChatResponse;
