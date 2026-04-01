// Main entry point - uses lib exports

use std::net::SocketAddr;
use std::sync::Arc;

use llm_gateway::config::Config;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing with simplified logging
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "llm_gateway=info,tower_http=warn".into()),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_thread_ids(false)
                .with_ansi(false)
                .without_time(),
        )
        .init();

    // Load configuration
    let config = if std::path::Path::new("config.toml").exists() {
        info!("Loading configuration from config.toml");
        Config::from_file("config.toml")?
    } else {
        info!("Loading configuration from environment variables");
        Config::from_env()?
    };

    let config = Arc::new(config);
    info!("Loaded {} providers", config.providers.len());

    // Build router
    let app = llm_gateway::build_router(config.clone());

    // Start server
    let addr = SocketAddr::from((
        config.server.host.parse::<std::net::IpAddr>()?,
        config.server.port,
    ));

    info!("Starting server on {}", addr);
    info!("Available endpoints:");
    info!("  POST /v1/chat/completions - Chat completions (auto-detect format)");
    info!("  POST /v1/messages - Chat completions (auto-detect format)");
    info!("  POST /v1/{{provider}}/chat/completions - Provider-specific endpoint");
    info!("  GET  /v1/models - List available models");
    info!("  GET  /health - Health check");
    info!("");
    info!("Request formats supported: openai, anthropic, ollama");
    info!("Output formats supported: openai-compatible, anthropic, ollama");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
