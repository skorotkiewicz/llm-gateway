//! Example: Adding a custom format to the LLM Proxy
//!
//! This example demonstrates how to add a custom request/response format
//! to the proxy using the modular format system.

use axum::http::HeaderMap;
use std::collections::HashMap;
use std::sync::Arc;

// Import the format traits and types from the main crate
use llm_proxy_api::formats::{
    CanonicalChatRequest, CanonicalChatResponse, CanonicalMessage, FormatRegistry,
    RequestInterpreter, ResponseFormatter,
};

// ============================================
// Example: Simple Custom Format (Ollama-style)
// ============================================

/// Ollama-style request format
#[derive(Debug, Clone, serde::Deserialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>,
}

/// Ollama-style response format
#[derive(Debug, Clone, serde::Serialize)]
struct OllamaResponse {
    model: String,
    response: String,
    done: bool,
}

/// Request interpreter for Ollama format
pub struct OllamaInterpreter;

impl RequestInterpreter for OllamaInterpreter {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn can_interpret(&self, headers: &HeaderMap, body: &[u8]) -> bool {
        // Check if request has "prompt" field instead of "messages"
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(body) {
            json.get("prompt").is_some() && json.get("messages").is_none()
        } else {
            false
        }
    }

    fn interpret(&self, body: &[u8]) -> anyhow::Result<CanonicalChatRequest> {
        let ollama: OllamaRequest = serde_json::from_slice(body)?;

        // Convert Ollama format to canonical format
        let mut messages = vec![];

        // Add system message if present
        if let Some(system) = &ollama.system {
            messages.push(CanonicalMessage {
                role: "system".to_string(),
                content: system.clone(),
                name: None,
            });
        }

        // Add user prompt
        messages.push(CanonicalMessage {
            role: "user".to_string(),
            content: ollama.prompt.clone(),
            name: None,
        });

        Ok(CanonicalChatRequest {
            model: ollama.model,
            messages,
            temperature: ollama.options.as_ref().and_then(|o| o.temperature),
            max_tokens: ollama.options.as_ref().and_then(|o| o.num_predict),
            stream: ollama.stream,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            system: ollama.system,
            extra: HashMap::new(),
        })
    }
}

/// Response formatter for Ollama format
pub struct OllamaFormatter;

impl ResponseFormatter for OllamaFormatter {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn format(&self, response: &CanonicalChatResponse) -> anyhow::Result<Vec<u8>> {
        // Convert canonical response to Ollama format
        // Take the first choice's content as the response
        let content = response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        let ollama_resp = OllamaResponse {
            model: response.model.clone(),
            response: content,
            done: true,
        };

        Ok(serde_json::to_vec(&ollama_resp)?)
    }
}

// ============================================
// Example Usage
// ============================================

fn main() {
    // Create a format registry with built-in formats
    let mut registry = FormatRegistry::new();

    // Register our custom Ollama format
    registry.register_interpreter(Arc::new(OllamaInterpreter));
    registry.register_formatter(Arc::new(OllamaFormatter));

    println!(
        "Registered interpreters: {:?}",
        registry.interpreter_names()
    );
    println!("Registered formatters: {:?}", registry.formatter_names());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_interpreter() {
        let interpreter = OllamaInterpreter;

        // Test can_interpret
        let ollama_body = br#"{"model": "llama2", "prompt": "Hello!"}"#;
        assert!(interpreter.can_interpret(&HeaderMap::new(), ollama_body));

        // Test interpret
        let canonical = interpreter.interpret(ollama_body).unwrap();
        assert_eq!(canonical.model, "llama2");
        assert_eq!(canonical.messages.len(), 1);
        assert_eq!(canonical.messages[0].content, "Hello!");
    }

    #[test]
    fn test_ollama_formatter() {
        let formatter = OllamaFormatter;

        let canonical = CanonicalChatResponse {
            id: "test-123".to_string(),
            object: "chat.completion".to_string(),
            created: 1234567890,
            model: "llama2".to_string(),
            choices: vec![CanonicalChoice {
                index: 0,
                message: CanonicalMessage {
                    role: "assistant".to_string(),
                    content: "Hi there!".to_string(),
                    name: None,
                },
                finish_reason: Some("stop".to_string()),
            }],
            usage: None,
        };

        let formatted = formatter.format(&canonical).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&formatted).unwrap();

        assert_eq!(json["model"], "llama2");
        assert_eq!(json["response"], "Hi there!");
        assert_eq!(json["done"], true);
    }
}
