# LLM Gateway

A Rust-based HTTP proxy that bridges different LLM API formats. Configure providers with specific input/output formats and route requests seamlessly between incompatible APIs.

## How It Works

LLM Gateway acts as a translation layer between API formats:

1. **Client sends request** in configured input format (OpenAI, Anthropic, or Ollama)
2. **Gateway converts** the request to internal canonical format
3. **Request is forwarded** to the upstream LLM provider
4. **Response is converted** from provider's format to configured output format
5. **Client receives response** in the expected output format

## Features

- **Format Translation**: Convert between OpenAI, Anthropic, and Ollama API formats
- **Multi-Provider Support**: Configure multiple providers simultaneously
- **Provider Selection**: Use default provider or specify by name in URL
- **Flexible I/O**: Each provider has independent input and output format configuration
- **API Key Protection**: Optional gateway-level authentication
- **Environment Configuration**: Full configuration via environment variables
- **Pure Rust**: No OpenSSL dependencies, uses rustls for TLS

## Quick Start

### 1. Create Configuration

Create `config.toml`:

```toml
[server]
host = "0.0.0.0"
port = 8888
api-key = "your-gateway-key"  # Optional: protects the gateway

[providers.local_ml]
base_url = "http://localhost:11434/v1"
api_key = "ollama-key"
input = "openai"           # What format clients send
output = "openai-compatible" # What format clients receive

[providers.claude]
base_url = "https://api.anthropic.com/v1"
api_key = "sk-ant-api03-..."
input = "anthropic"        # Accept Anthropic format
output = "openai-compatible" # Return OpenAI format
```

### 2. Run the Server

```bash
cargo run --release
# Or use the binary directly
./target/release/llm-gateway
```

## API Endpoints

### Default Provider Endpoints

These use the **first configured provider** from your config:

```bash
# Chat completions (OpenAI-compatible endpoint)
curl http://localhost:8888/v1/chat/completions \
  -H "Authorization: Bearer your-gateway-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama3.2",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'

# Chat completions (Anthropic-compatible endpoint)
curl http://localhost:8888/v1/messages \
  -H "Authorization: Bearer your-gateway-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-3-opus",
    "messages": [{"role": "user", "content": "Hello!"}],
    "max_tokens": 1024
  }'

# List models
curl http://localhost:8888/v1/models \
  -H "Authorization: Bearer your-gateway-key"
```

### Provider-Specific Endpoints

Route to a **specific provider** by name:

```bash
# Route to 'claude' provider
curl http://localhost:8888/v1/claude/chat/completions \
  -H "Authorization: Bearer your-gateway-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-3-opus",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'

# Route to 'local_ml' provider
curl http://localhost:8888/v1/local_ml/chat/completions \
  -H "Authorization: Bearer your-gateway-key" \
  -d '{"model": "llama3.2", "messages": [{"role": "user", "content": "Hi"}]}'
```

### Health Check

```bash
# No authentication required
curl http://localhost:8888/health
```

Response:
```json
{"status": "healthy", "version": "0.1.0"}
```

## Configuration Reference

### TOML Configuration

```toml
[server]
host = "0.0.0.0"          # Bind address
port = 8888               # Bind port
api-key = "secret-key"    # Optional gateway authentication

[providers.provider_name]
base_url = "https://api.example.com/v1"  # Upstream API URL
api_key = "upstream-api-key"              # Upstream authentication
input = "openai"          # Expected client format: openai, anthropic, ollama
output = "openai-compatible"  # Response format: openai-compatible, anthropic, ollama
```

### Environment Variables

Configure entirely via environment variables:

```bash
# Server settings
export PROXY_HOST="0.0.0.0"
export PROXY_PORT="8888"
export PROXY_API_KEY="your-gateway-key"

# Provider 1: Local Ollama
export PROVIDER_LOCAL_BASE_URL="http://localhost:11434/v1"
export PROVIDER_LOCAL_API_KEY="ollama-key"
export PROVIDER_LOCAL_INPUT="openai"
export PROVIDER_LOCAL_OUTPUT="openai-compatible"

# Provider 2: Anthropic
export PROVIDER_CLAUDE_BASE_URL="https://api.anthropic.com/v1"
export PROVIDER_CLAUDE_API_KEY="sk-ant-..."
export PROVIDER_CLAUDE_INPUT="anthropic"
export PROVIDER_CLAUDE_OUTPUT="openai-compatible"

# Run server
./llm-gateway
```

**Note**: When using environment variables, the gateway reads config directly from env. No config.toml needed.

## Format Reference

### Input Formats

What format clients **must send** to the gateway:

| Format | Description | Example Use Case |
|--------|-------------|------------------|
| `openai` | OpenAI Chat Completions format | Clients expecting OpenAI-compatible API |
| `anthropic` | Anthropic Messages format | Clients using Claude SDK |
| `ollama` | Ollama generate/chat format | Ollama-native clients |

### Output Formats

What format clients **will receive** from the gateway:

| Format | Description | Example Use Case |
|--------|-------------|------------------|
| `openai-compatible` | OpenAI Chat Completions response | Most clients expect this |
| `anthropic` | Anthropic Messages response | Native Anthropic clients |
| `ollama` | Ollama generate response | Ollama-native clients |

### Common Configurations

**OpenAI-to-OpenAI** (transparent proxy):
```toml
input = "openai"
output = "openai-compatible"
```

**Anthropic-to-OpenAI** (use Claude with OpenAI clients):
```toml
input = "anthropic"
output = "openai-compatible"
```

**Ollama-to-OpenAI** (use Ollama with OpenAI clients):
```toml
input = "ollama"
output = "openai-compatible"
```

## Architecture

```
┌─────────────┐     Request      ┌─────────────────┐
│   Client    │ ────────────────> │  LLM Gateway    │
│  (any format)│    (input fmt)   │                 │
└─────────────┘                   │  ┌───────────┐  │
                                  │  │  Parse    │  │
                                  │  │  Request  │  │
                                  │  │  (input)  │  │
                                  │  └─────┬─────┘  │
                                  │        │         │
                                  │  ┌─────▼─────┐  │
                                  │  │ Canonical │  │
                                  │  │   Format  │  │
                                  │  └─────┬─────┘  │
                                  │        │         │
                                  │  ┌─────▼─────┐  │
                                  │  │  Forward  │  │
                                  │  │  Request  │  │
                                  │  └─────┬─────┘  │
                                  └────────┼────────┘
                                         │
                                  ┌──────▼──────┐
                                  │   Upstream  │
                                  │   Provider  │
                                  └──────┬──────┘
                                         │
                                  ┌──────▼──────┐
                                  │  Response   │
                                  └──────┬──────┘
                                         │
                                  ┌──────▼──────┐     Response     ┌─────────────┐
                                  │  LLM Gateway│ ───────────────> │   Client    │
                                  │  (convert   │   (output fmt)   │  (expected  │
                                  │   to output)│                  │   format)   │
                                  └─────────────┘                  └─────────────┘
```

### Module Structure

- **`protocols/`** - API format implementations and conversions
  - `mod.rs` - Canonical types and conversion dispatch
  - `openai.rs` - OpenAI request/response types
  - `anthropic.rs` - Anthropic request/response types
  - `ollama.rs` - Ollama request/response types

- **`config.rs`** - Configuration parsing (TOML and environment variables)

- **`proxy.rs`** - Request routing, provider selection, forwarding logic

- **`middleware.rs`** - Authentication middleware

- **`lib.rs`** - Router builder and test utilities

- **`main.rs`** - Server startup and logging

## Request Flow Details

### 1. Request Parsing

The gateway parses client requests based on the provider's configured `input` format:

- **OpenAI**: Expects `model`, `messages` array, optional parameters
- **Anthropic**: Expects `model`, `messages`, `max_tokens`, optional `system`
- **Ollama**: Accepts either `prompt` string or `messages` array

### 2. Canonical Format

All requests are converted to an internal canonical format:

```rust
CanonicalChatRequest {
    model: String,
    messages: Vec<CanonicalMessage>,  // role, content, name
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    stream: Option<bool>,
    // ... other fields
}
```

### 3. Upstream Forwarding

The canonical request is serialized to the upstream provider's expected format (always OpenAI-compatible for most providers) and forwarded with authentication.

### 4. Response Conversion

The upstream response is parsed and converted to the configured `output` format:

- **OpenAI-compatible**: Standard chat completions response
- **Anthropic**: Messages API response format
- **Ollama**: Generate/chat response format

## Development

### Building

```bash
# Development build
cargo build

# Release build
cargo build --release

# Run tests
cargo test
```

### Testing with Just

```bash
# Run all checks
just check

# Format code
just fmt

# Run integration test
just test-api
```

### Adding New Formats

To add a new API format:

1. Create a new file in `src/protocols/` (e.g., `custom.rs`)
2. Define request and response structs
3. Implement `to_canonical()` for the request type
4. Implement `From<CanonicalChatResponse>` for the response type
5. Add format to `InputFormat` and `OutputFormat` enums in `protocols/mod.rs`
6. Add conversion cases in `parse_request()` and `format_response()`

Example structure:

```rust
// src/protocols/custom.rs
use super::{CanonicalChatRequest, CanonicalChatResponse, CanonicalMessage};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct CustomRequest {
    pub model: String,
    pub prompt: String,
    // ... other fields
}

impl CustomRequest {
    pub fn to_canonical(&self) -> CanonicalChatRequest {
        CanonicalChatRequest {
            model: self.model.clone(),
            messages: vec![CanonicalMessage {
                role: "user".to_string(),
                content: self.prompt.clone(),
                name: None,
            }],
            // ... other fields
        }
    }
}
```

## Troubleshooting

### "Endpoint not found" Error

- Check that you're using the correct URL pattern: `/v1/{provider}/chat/completions`
- Verify the provider name matches your config exactly

### "No provider configured" Error

- Ensure at least one provider is configured in `config.toml` or environment variables
- Check that the config file is in the current working directory

### Format Conversion Errors

- Verify the client's request matches the provider's configured `input` format
- Check provider documentation for required fields (e.g., Anthropic requires `max_tokens`)

### Authentication Failures

- If using `api-key` in config, ensure clients send `Authorization: Bearer <key>` header
- Verify the upstream provider's `api_key` is correctly configured

## License

MIT
