# LLM Proxy API

A Rust-based HTTP proxy that normalizes requests from multiple sources (Anthropic, OpenAI, Zai, etc.) to OpenAI-compatible format and outputs in the configured format.

## Features

- **Modular format system**: Easy to add new request/response formats
- **Multi-provider support**: OpenAI, Anthropic, Ollama, Zai, and custom providers
- **Request normalization**: Automatically detect and convert input formats (OpenAI, Anthropic, Ollama)
- **Configurable output format**: Per-provider output format configuration
- **Streaming support**: SSE streaming for real-time responses
- **API key protection**: Secure your proxy with API key authentication
- **Multiple configuration methods**: TOML file or environment variables

## Quick Start

1. Configure via environment variables or create a `config.toml`:

```toml
[server]
host = "0.0.0.0"
port = 8888
api-key = "your-proxy-api-key"

[providers.openai]
base_url = "https://api.openai.com/v1"
api_key = "sk-your-openai-key"
output = "openai-compatible"

[providers.anthropic]
base_url = "https://api.anthropic.com/v1"
api_key = "sk-ant-your-key"
output = "anthropic"
```

2. Run the server:

```bash
cargo run
```

## API Endpoints

### Chat Completions (Auto-detect format)
```bash
curl http://localhost:8888/v1/chat/completions \
  -H "Authorization: Bearer your-proxy-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

The proxy automatically detects the request format based on headers and body content.

**Supported Input Formats:**
- OpenAI (`model`, `messages`)
- Anthropic (`model`, `messages`, `max_tokens`)
- Ollama (`model`, `prompt` or `messages` with images)

### Provider-specific endpoint
```bash
curl http://localhost:8888/v1/openai/chat/completions \
  -H "Authorization: Bearer your-proxy-api-key" \
  -H "Content-Type: application/json" \
  -d '{"model": "gpt-4", "messages": [{"role": "user", "content": "Hi!"}]}'
```

### List models
```bash
curl http://localhost:8888/v1/models \
  -H "Authorization: Bearer your-proxy-api-key"
```

### Health check
```bash
curl http://localhost:8888/health
```

## Environment Variables

```bash
export PROXY_HOST="0.0.0.0"
export PROXY_PORT="8888"
export PROXY_API_KEY="your-proxy-api-key"

export PROVIDER_OPENAI_BASE_URL="https://api.openai.com/v1"
export PROVIDER_OPENAI_API_KEY="sk-your-openai-key"
export PROVIDER_OPENAI_OUTPUT="openai-compatible"

export PROVIDER_ANTHROPIC_BASE_URL="https://api.anthropic.com/v1"
export PROVIDER_ANTHROPIC_API_KEY="sk-ant-your-key"
export PROVIDER_ANTHROPIC_OUTPUT="anthropic"

export PROVIDER_OLLAMA_BASE_URL="http://localhost:11434/api"
export PROVIDER_OLLAMA_API_KEY="ollama-api-key"
export PROVIDER_OLLAMA_OUTPUT="ollama"
```

## Request Flow

1. Client sends request (any supported format)
2. Proxy **auto-detects** the request format
3. Request is normalized to **canonical internal format**
4. Request is forwarded to configured upstream provider
5. Response is converted to configured **output format**
6. Response is returned to client

## Architecture

- **config.rs**: Configuration parsing (TOML/env vars)
- **formats.rs**: Modular format system with interpreters and formatters
- **middleware.rs**: API key authentication
- **proxy.rs**: Request forwarding and format conversion
- **main.rs**: Server setup and routing

## Adding Custom Formats

The proxy uses a modular system where you can easily add new format interpreters and formatters.

### Step 1: Implement `RequestInterpreter` trait

```rust
use axum::http::HeaderMap;
use llm_proxy_api::formats::{RequestInterpreter, CanonicalChatRequest};

pub struct CustomInterpreter;

impl RequestInterpreter for CustomInterpreter {
    fn name(&self) -> &'static str {
        "custom"
    }
    
    fn can_interpret(&self, headers: &HeaderMap, body: &[u8]) -> bool {
        // Check if this interpreter can handle the request
        // e.g., check for specific headers or body structure
        headers.get("x-api-type").is_some()
    }
    
    fn interpret(&self, body: &[u8]) -> anyhow::Result<CanonicalChatRequest> {
        // Parse your custom format and convert to canonical format
        let custom: CustomRequest = serde_json::from_slice(body)?;
        Ok(custom.to_canonical())
    }
}
```

### Step 2: Implement `ResponseFormatter` trait

```rust
use llm_proxy_api::formats::{ResponseFormatter, CanonicalChatResponse};

pub struct CustomFormatter;

impl ResponseFormatter for CustomFormatter {
    fn name(&self) -> &'static str {
        "custom"
    }
    
    fn format(&self, response: &CanonicalChatResponse) -> anyhow::Result<Vec<u8>> {
        // Convert canonical response to your custom format
        let custom_resp: CustomResponse = response.clone().into();
        Ok(serde_json::to_vec(&custom_resp)?)
    }
}
```

### Step 3: Register in `FormatRegistry`

```rust
// In src/formats.rs, add to FormatRegistry::new():
registry.register_interpreter(Arc::new(CustomInterpreter));
registry.register_formatter(Arc::new(CustomFormatter));
```

Or create your own registry initialization:

```rust
use llm_proxy_api::formats::FormatRegistry;

let mut registry = FormatRegistry::new();
registry.register_interpreter(Arc::new(CustomInterpreter));
registry.register_formatter(Arc::new(CustomFormatter));
```

### Complete Example: Adding Google Gemini Format

```rust
// src/formats/gemini.rs

use axum::http::HeaderMap;
use crate::formats::{RequestInterpreter, ResponseFormatter, CanonicalChatRequest, CanonicalChatResponse};

pub struct GeminiInterpreter;

impl RequestInterpreter for GeminiInterpreter {
    fn name(&self) -> &'static str {
        "gemini"
    }
    
    fn can_interpret(&self, headers: &HeaderMap, body: &[u8]) -> bool {
        // Check for Gemini-specific fields
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(body) {
            json.get("contents").is_some()
        } else {
            false
        }
    }
    
    fn interpret(&self, body: &[u8]) -> anyhow::Result<CanonicalChatRequest> {
        let gemini: GeminiRequest = serde_json::from_slice(body)?;
        
        Ok(CanonicalChatRequest {
            model: gemini.model,
            messages: gemini.contents.into_iter().flat_map(|c| {
                c.parts.into_iter().map(|p| CanonicalMessage {
                    role: c.role.clone(),
                    content: p.text,
                    name: None,
                })
            }).collect(),
            temperature: gemini.generation_config.temperature,
            max_tokens: gemini.generation_config.max_output_tokens,
            stream: None,
            top_p: gemini.generation_config.top_p,
            frequency_penalty: None,
            presence_penalty: None,
            stop: gemini.generation_config.stop_sequences,
            system: None,
            extra: HashMap::new(),
        })
    }
}

pub struct GeminiFormatter;

impl ResponseFormatter for GeminiFormatter {
    fn name(&self) -> &'static str {
        "gemini"
    }
    
    fn format(&self, response: &CanonicalChatResponse) -> anyhow::Result<Vec<u8>> {
        let gemini_resp = GeminiResponse {
            candidates: response.choices.iter().map(|c| Candidate {
                content: Content {
                    role: c.message.role.clone(),
                    parts: vec![Part { text: c.message.content.clone() }],
                },
            }).collect(),
        };
        Ok(serde_json::to_vec(&gemini_resp)?)
    }
}
```

Then register in main.rs or formats.rs:

```rust
registry.register_interpreter(Arc::new(GeminiInterpreter));
registry.register_formatter(Arc::new(GeminiFormatter));
```

## Development

```bash
# Run in development mode
cargo run

# Run tests
cargo test

# Build release
cargo build --release
```

## License

MIT
