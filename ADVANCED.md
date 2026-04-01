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
- **formats.rs**: Simple format types and conversion functions
  - `anthropic_to_openai()` - converts Anthropic request to OpenAI
  - `openai_to_anthropic()` - converts OpenAI response to Anthropic  
  - `ollama_to_openai()` - converts Ollama request to OpenAI
  - `openai_to_ollama()` - converts OpenAI response to Ollama
- **middleware.rs**: API key authentication
- **proxy.rs**: Request forwarding using simple conversion functions
- **main.rs**: Server setup and routing

## Adding Custom Formats

Just add conversion functions in `formats.rs`:

```rust
// 1. Define your format types
#[derive(Debug, Clone, Deserialize)]
pub struct MyCustomRequest { ... }

// 2. Add conversion function
impl MyCustomRequest {
    pub fn to_canonical(self) -> CanonicalChatRequest {
        // Convert your format to canonical
    }
}

// 3. Add reverse conversion
impl From<&CanonicalChatResponse> for MyCustomResponse {
    fn from(c: &CanonicalChatResponse) -> Self {
        // Convert canonical to your format
    }
}

// 4. Update detect_input_format() to recognize your format
pub fn detect_input_format(body: &[u8]) -> InputFormat {
    // Check for your format's unique fields
    if json.get("your_unique_field").is_some() {
        return InputFormat::Custom;
    }
}

// 5. Update parse_request() and format_response() match arms
```

That's it! No traits, no registry, no complex patterns.

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
