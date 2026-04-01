use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Internal canonical representation of a chat request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalChatRequest {
    pub model: String,
    pub messages: Vec<CanonicalMessage>,
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
    /// Additional provider-specific fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Internal canonical representation of a chat response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalChatResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<CanonicalChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<CanonicalUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalChoice {
    pub index: u32,
    pub message: CanonicalMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Trait for interpreting incoming requests from different formats
pub trait RequestInterpreter: Send + Sync {
    /// Name of this interpreter (e.g., "anthropic", "openai")
    fn name(&self) -> &'static str;

    /// Check if this interpreter can handle the given request
    fn can_interpret(&self, headers: &HeaderMap, body: &[u8]) -> bool;

    /// Parse the request body into canonical format
    fn interpret(&self, body: &[u8]) -> anyhow::Result<CanonicalChatRequest>;
}

/// Trait for formatting responses to different output formats
pub trait ResponseFormatter: Send + Sync {
    /// Name of this formatter (e.g., "anthropic", "openai")
    fn name(&self) -> &'static str;

    /// Format a canonical response to the output format
    fn format(&self, response: &CanonicalChatResponse) -> anyhow::Result<Vec<u8>>;
}

/// Registry of format interpreters and formatters
pub struct FormatRegistry {
    interpreters: HashMap<String, Arc<dyn RequestInterpreter>>,
    formatters: HashMap<String, Arc<dyn ResponseFormatter>>,
}

impl FormatRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            interpreters: HashMap::new(),
            formatters: HashMap::new(),
        };

        // Register built-in formats
        registry.register_interpreter(Arc::new(OpenAIInterpreter));
        registry.register_interpreter(Arc::new(AnthropicInterpreter));
        registry.register_interpreter(Arc::new(OllamaInterpreter));
        registry.register_formatter(Arc::new(OpenAIFormatter));
        registry.register_formatter(Arc::new(AnthropicFormatter));
        registry.register_formatter(Arc::new(OllamaFormatter));

        registry
    }

    pub fn register_interpreter(&mut self, interpreter: Arc<dyn RequestInterpreter>) {
        self.interpreters
            .insert(interpreter.name().to_string(), interpreter);
    }

    pub fn register_formatter(&mut self, formatter: Arc<dyn ResponseFormatter>) {
        self.formatters
            .insert(formatter.name().to_string(), formatter);
    }

    /// Get the number of registered interpreters
    pub fn interpreter_count(&self) -> usize {
        self.interpreters.len()
    }

    /// Get the number of registered formatters
    pub fn formatter_count(&self) -> usize {
        self.formatters.len()
    }

    /// Get list of registered interpreter names
    pub fn interpreter_names(&self) -> Vec<&str> {
        self.interpreters.keys().map(|k| k.as_str()).collect()
    }

    /// Get list of registered formatter names
    pub fn formatter_names(&self) -> Vec<&str> {
        self.formatters.keys().map(|k| k.as_str()).collect()
    }

    /// Detect which interpreter should handle this request
    pub fn detect_interpreter(
        &self,
        headers: &HeaderMap,
        body: &[u8],
    ) -> Option<Arc<dyn RequestInterpreter>> {
        // First check explicit headers
        if let Some(source) = headers.get("x-source") {
            if let Ok(source_str) = source.to_str() {
                let source_lower = source_str.to_lowercase();
                if let Some(interp) = self.interpreters.get(&source_lower) {
                    if interp.can_interpret(headers, body) {
                        return Some(interp.clone());
                    }
                }
            }
        }

        // Check User-Agent
        if let Some(ua) = headers.get("user-agent") {
            if let Ok(ua_str) = ua.to_str() {
                let ua_lower = ua_str.to_lowercase();
                for (name, interp) in &self.interpreters {
                    if ua_lower.contains(name) && interp.can_interpret(headers, body) {
                        return Some(interp.clone());
                    }
                }
            }
        }

        // Try each interpreter
        for (_, interp) in &self.interpreters {
            if interp.can_interpret(headers, body) {
                return Some(interp.clone());
            }
        }

        // Default to OpenAI
        self.interpreters.get("openai").cloned()
    }

    /// Get formatter by name
    pub fn get_formatter(&self, name: &str) -> Option<Arc<dyn ResponseFormatter>> {
        self.formatters.get(name).cloned()
    }
}

impl Default for FormatRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============== OpenAI Format ==============

pub struct OpenAIInterpreter;

impl RequestInterpreter for OpenAIInterpreter {
    fn name(&self) -> &'static str {
        "openai"
    }

    fn can_interpret(&self, _headers: &HeaderMap, body: &[u8]) -> bool {
        // OpenAI format is the default - it can parse most valid chat requests
        // Check if it has required fields
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(body) {
            json.get("model").is_some() && json.get("messages").is_some()
        } else {
            false
        }
    }

    fn interpret(&self, body: &[u8]) -> anyhow::Result<CanonicalChatRequest> {
        let openai: OpenAIChatRequest = serde_json::from_slice(body)?;
        Ok(openai.to_canonical())
    }
}

pub struct OpenAIFormatter;

impl ResponseFormatter for OpenAIFormatter {
    fn name(&self) -> &'static str {
        "openai"
    }

    fn format(&self, response: &CanonicalChatResponse) -> anyhow::Result<Vec<u8>> {
        let openai_resp: OpenAIChatResponse = response.clone().into();
        Ok(serde_json::to_vec(&openai_resp)?)
    }
}

// ============== Anthropic Format ==============

pub struct AnthropicInterpreter;

impl RequestInterpreter for AnthropicInterpreter {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn can_interpret(&self, headers: &HeaderMap, body: &[u8]) -> bool {
        // Check for anthropic-specific headers
        if let Some(auth) = headers.get("authorization") {
            if let Ok(auth_str) = auth.to_str() {
                if auth_str.to_lowercase().contains("anthropic") {
                    return true;
                }
            }
        }

        // Check for anthropic-specific fields in body
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(body) {
            // Anthropic uses "max_tokens" as required field
            if json.get("max_tokens").is_some() && json.get("messages").is_some() {
                // Check if messages use anthropic format (role-based with no "name" field pattern)
                return true;
            }
        }

        false
    }

    fn interpret(&self, body: &[u8]) -> anyhow::Result<CanonicalChatRequest> {
        let anthropic: AnthropicRequest = serde_json::from_slice(body)?;
        Ok(anthropic.to_canonical())
    }
}

pub struct AnthropicFormatter;

impl ResponseFormatter for AnthropicFormatter {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn format(&self, response: &CanonicalChatResponse) -> anyhow::Result<Vec<u8>> {
        let anthropic_resp: AnthropicResponse = response.clone().into();
        Ok(serde_json::to_vec(&anthropic_resp)?)
    }
}

// ============== OpenAI Types ==============

#[derive(Debug, Clone, Deserialize, Serialize)]
struct OpenAIChatRequest {
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
struct OpenAIMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIChatResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OpenAIChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<OpenAIUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIChoice {
    pub index: u32,
    pub message: OpenAIMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

impl OpenAIChatRequest {
    fn to_canonical(self) -> CanonicalChatRequest {
        CanonicalChatRequest {
            model: self.model,
            messages: self
                .messages
                .into_iter()
                .map(|m| CanonicalMessage {
                    role: m.role,
                    content: m.content,
                    name: m.name,
                })
                .collect(),
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            stream: self.stream,
            top_p: self.top_p,
            frequency_penalty: self.frequency_penalty,
            presence_penalty: self.presence_penalty,
            stop: self.stop,
            system: None,
            extra: HashMap::new(),
        }
    }
}

impl From<CanonicalChatResponse> for OpenAIChatResponse {
    fn from(c: CanonicalChatResponse) -> Self {
        OpenAIChatResponse {
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

// ============== Anthropic Types ==============

#[derive(Debug, Clone, Deserialize, Serialize)]
struct AnthropicRequest {
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
struct AnthropicMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicResponse {
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
enum AnthropicContent {
    #[serde(rename = "text")]
    Text { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

impl AnthropicRequest {
    fn to_canonical(self) -> CanonicalChatRequest {
        let mut messages: Vec<CanonicalMessage> = self
            .messages
            .into_iter()
            .map(|m| CanonicalMessage {
                role: m.role,
                content: m.content,
                name: None,
            })
            .collect();

        // Convert system message if present
        if let Some(ref system) = self.system {
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
            model: self.model,
            messages,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            stream: self.stream,
            top_p: self.top_p,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            system: self.system.clone(),
            extra: HashMap::new(),
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

        let usage = c.usage.map(|u| AnthropicUsage {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
        });

        AnthropicResponse {
            id: c.id,
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            content,
            model: c.model,
            stop_reason: c.choices.first().and_then(|ch| ch.finish_reason.clone()),
            usage,
        }
    }
}

// ============== Ollama Format ==============

pub struct OllamaInterpreter;

impl RequestInterpreter for OllamaInterpreter {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn can_interpret(&self, _headers: &HeaderMap, body: &[u8]) -> bool {
        // Ollama uses "prompt" instead of "messages" or has messages with different structure
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(body) {
            // Has prompt field (non-chat mode) or has messages with images/ollama-specific fields
            if json.get("prompt").is_some() {
                return true;
            }
            // Check for ollama-specific fields like "images" in messages
            if let Some(messages) = json.get("messages").and_then(|m| m.as_array()) {
                for msg in messages {
                    if msg.get("images").is_some() {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn interpret(&self, body: &[u8]) -> anyhow::Result<CanonicalChatRequest> {
        let ollama: OllamaRequest = serde_json::from_slice(body)?;
        Ok(ollama.to_canonical())
    }
}

pub struct OllamaFormatter;

impl ResponseFormatter for OllamaFormatter {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn format(&self, response: &CanonicalChatResponse) -> anyhow::Result<Vec<u8>> {
        let ollama_resp: OllamaResponse = response.into();
        Ok(serde_json::to_vec(&ollama_resp)?)
    }
}

// ============== Ollama Types ==============

#[derive(Debug, Clone, Deserialize, Serialize)]
struct OllamaRequest {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<Vec<OllamaMessage>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<OllamaOptions>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct OllamaMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_predict: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i32>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OllamaResponse {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    pub message: Option<OllamaMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<String>,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_duration: Option<u64>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl OllamaRequest {
    fn to_canonical(self) -> CanonicalChatRequest {
        let mut messages = vec![];
        let mut extra = HashMap::new();

        // Copy ollama-specific fields to extra
        if let Some(template) = self.template {
            extra.insert("template".to_string(), serde_json::Value::String(template));
        }
        if let Some(context) = self.context {
            extra.insert("context".to_string(), context);
        }
        if let Some(raw) = self.raw {
            extra.insert("raw".to_string(), serde_json::Value::Bool(raw));
        }
        if let Some(fmt) = self.format {
            extra.insert("format".to_string(), serde_json::Value::String(fmt));
        }
        // Copy any extra fields
        for (key, value) in self.extra {
            extra.insert(key, value);
        }

        // Handle system message
        if let Some(ref system) = self.system {
            messages.push(CanonicalMessage {
                role: "system".to_string(),
                content: system.clone(),
                name: None,
            });
        }

        // Handle messages or prompt
        if let Some(ollama_messages) = self.messages {
            // Chat mode with messages
            for msg in ollama_messages {
                let canonical_msg = CanonicalMessage {
                    role: msg.role,
                    content: msg.content,
                    name: None,
                };
                messages.push(canonical_msg);

                // Store images in extra if present
                if let Some(images) = msg.images {
                    extra.insert(
                        "images".to_string(),
                        serde_json::Value::Array(
                            images
                                .into_iter()
                                .map(|i| serde_json::Value::String(i))
                                .collect(),
                        ),
                    );
                }
            }
        } else if let Some(prompt) = self.prompt {
            // Non-chat mode with prompt
            messages.push(CanonicalMessage {
                role: "user".to_string(),
                content: prompt,
                name: None,
            });
        }

        // Extract options
        let (temperature, top_p, max_tokens) = self
            .options
            .map(|o| (o.temperature, o.top_p, o.num_predict.map(|n| n as u32)))
            .unwrap_or((None, None, None));

        CanonicalChatRequest {
            model: self.model,
            messages,
            temperature,
            max_tokens,
            stream: self.stream,
            top_p,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            system: self.system.clone(),
            extra,
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

        let message = Some(OllamaMessage {
            role: "assistant".to_string(),
            content: content.clone(),
            images: None,
        });

        let usage_tokens = c.usage.as_ref().map(|u| u.completion_tokens);

        OllamaResponse {
            model: c.model.clone(),
            created_at: Some(chrono::Utc::now().to_rfc3339()),
            message,
            response: Some(content),
            done: true,
            context: None,
            total_duration: None,
            load_duration: None,
            prompt_eval_count: c.usage.as_ref().map(|u| u.prompt_tokens),
            prompt_eval_duration: None,
            eval_count: usage_tokens,
            eval_duration: None,
            extra: HashMap::new(),
        }
    }
}
