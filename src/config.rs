use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub server: ServerConfig,
    pub providers: HashMap<String, ProviderConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    #[serde(rename = "api-key")]
    pub api_key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderConfig {
    pub base_url: String,
    pub api_key: String,
    pub output: OutputFormat,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    OpenAiCompatible,
    Anthropic,
}

impl Config {
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn from_env() -> anyhow::Result<Self> {
        let host = std::env::var("PROXY_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = std::env::var("PROXY_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8888);
        let api_key = std::env::var("PROXY_API_KEY")
            .map_err(|_| anyhow::anyhow!("PROXY_API_KEY environment variable must be set"))?;

        let mut providers = HashMap::new();

        // Parse providers from env
        // Format: PROVIDER_<NAME>_BASE_URL, PROVIDER_<NAME>_API_KEY, PROVIDER_<NAME>_OUTPUT
        for (key, value) in std::env::vars() {
            if key.starts_with("PROVIDER_") && key.ends_with("_BASE_URL") {
                let name = key
                    .trim_start_matches("PROVIDER_")
                    .trim_end_matches("_BASE_URL")
                    .to_lowercase();

                let base_url = value;
                let api_key = std::env::var(format!("PROVIDER_{}_API_KEY", name.to_uppercase()))?;
                let output = std::env::var(format!("PROVIDER_{}_OUTPUT", name.to_uppercase()))
                    .unwrap_or_else(|_| "openai-compatible".to_string());

                let output_format = match output.as_str() {
                    "anthropic" => OutputFormat::Anthropic,
                    _ => OutputFormat::OpenAiCompatible,
                };

                providers.insert(
                    name,
                    ProviderConfig {
                        base_url,
                        api_key,
                        output: output_format,
                    },
                );
            }
        }

        Ok(Config {
            server: ServerConfig {
                host,
                port,
                api_key,
            },
            providers,
        })
    }
}
