pub mod anthropic;
pub mod ollama;
pub mod openai;

use serde::{Deserialize, Serialize};

// Canonical Types (Internal Format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalChatRequest {
    pub model: String,
    pub messages: Vec<CanonicalMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stream: Option<bool>,
    pub top_p: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub stop: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalMessage {
    pub role: String,
    pub content: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalChatResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<CanonicalChoice>,
    pub usage: Option<CanonicalUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalChoice {
    pub index: u32,
    pub message: CanonicalMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// Format Enums
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputFormat {
    OpenAI,
    Anthropic,
    Ollama,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    OpenAI,
    Anthropic,
    Ollama,
}

// Conversion Functions
pub fn parse_request(format: InputFormat, body: &[u8]) -> anyhow::Result<CanonicalChatRequest> {
    match format {
        InputFormat::OpenAI => {
            let req: openai::OpenAIRequest = serde_json::from_slice(body)?;
            Ok(req.to_canonical())
        }
        InputFormat::Anthropic => {
            let req: anthropic::AnthropicRequest = serde_json::from_slice(body)?;
            Ok(req.to_canonical())
        }
        InputFormat::Ollama => {
            let req: ollama::OllamaRequest = serde_json::from_slice(body)?;
            Ok(req.to_canonical())
        }
    }
}

pub fn format_response(
    format: OutputFormat,
    response: &CanonicalChatResponse,
) -> anyhow::Result<Vec<u8>> {
    match format {
        OutputFormat::OpenAI => {
            let resp: openai::OpenAIResponse = response.clone().into();
            Ok(serde_json::to_vec(&resp)?)
        }
        OutputFormat::Anthropic => {
            let resp: anthropic::AnthropicResponse = response.clone().into();
            Ok(serde_json::to_vec(&resp)?)
        }
        OutputFormat::Ollama => {
            let resp: ollama::OllamaResponse = response.into();
            Ok(serde_json::to_vec(&resp)?)
        }
    }
}
