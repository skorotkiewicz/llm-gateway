# LLM Proxy

Simple API format converter for LLM providers.

## Usage

```bash
cargo run
```

## Configuration

Create `config.toml`:

```toml
[server]
host = "0.0.0.0"
port = 8888
api-key = "your-proxy-key"

[providers.ollama]
base_url = "http://localhost:11434/api"
api_key = "key"
input = "ollama"
output = "openai-compatible"
```

**Input formats:** openai, anthropic, ollama  
**Output formats:** openai-compatible, anthropic, ollama

## Example

```bash
curl http://localhost:8888/v1/chat/completions \
  -H "Authorization: Bearer your-proxy-key" \
  -d '{"model": "llama2", "prompt": "Hello!"}'
```

## Architecture

- `protocols/` - Format implementations
- `config.rs` - Configuration
- `proxy.rs` - Request handling
- `middleware.rs` - Authentication

See [ADVANCED.md](ADVANCED.md) for detailed documentation.

## License

MIT
