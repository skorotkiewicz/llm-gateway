# LLM Proxy API

A Rust-based HTTP proxy that normalizes requests from multiple sources (Anthropic, OpenAI, Zai, etc.) to OpenAI-compatible format and outputs in the configured format.

## Features

- Multi-provider support (OpenAI, Anthropic, Zai, and custom providers)
- Request normalization from Anthropic/OpenAI/Zai formats to OpenAI-compatible
- Configurable output format per provider (OpenAI-compatible or Anthropic)
- Streaming support (SSE)
- API key protection
- Environment variable or TOML configuration
- Health check endpoint
- OpenAI-compatible and Anthropic-compatible endpoints

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

### Chat Completions (OpenAI format)
```bash
curl http://localhost:8888/v1/chat/completions \
  -H "Authorization: Bearer your-proxy-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

### Chat Completions (Anthropic format)
```bash
curl http://localhost:8888/v1/messages \
  -H "Authorization: Bearer your-proxy-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-3-opus-20240229",
    "messages": [{"role": "user", "content": "Hello!"}],
    "max_tokens": 1024
  }'
```

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

Instead of using `config.toml`, you can configure via environment:

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
```

## Request Flow

1. Client sends request (Anthropic/OpenAI/Zai format)
2. Proxy validates API key
3. Request is normalized to OpenAI-compatible format
4. Request is forwarded to configured upstream provider
5. Response is converted to configured output format
6. Response is returned to client

## Architecture

- **config.rs**: Configuration parsing (TOML/env vars)
- **models.rs**: Request/response types for OpenAI and Anthropic
- **middleware.rs**: API key authentication
- **proxy.rs**: Request forwarding and format conversion
- **main.rs**: Server setup and routing

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
