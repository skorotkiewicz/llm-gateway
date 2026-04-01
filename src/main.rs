mod config;
mod protocols;
mod middleware;
mod proxy;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    middleware::from_fn_with_state,
    routing::{get, post},
    Router,
};
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use config::Config;
use proxy::{chat_completions, chat_completions_default, fallback_handler, health_check, list_models, ProxyState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing with simplified logging
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            "llm_proxy_api=info,tower_http=warn".into()
        }))
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_thread_ids(false)
                .with_ansi(false)
                .without_time()
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

    // Create proxy state
    let proxy_state = Arc::new(ProxyState::new(config.clone()));

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build router
    let app = Router::new()
        // Health check (no auth required)
        .route("/health", get(health_check))
        // OpenAI-compatible endpoints (no provider)
        .route("/v1/chat/completions", post(chat_completions_default))
        .route("/v1/models", get(list_models))
        // Provider-specific endpoints
        .route("/v1/:provider/chat/completions", post(chat_completions))
        .route("/v1/:provider/models", get(list_models))
        // Anthropic-compatible endpoints (no provider)
        .route("/v1/messages", post(chat_completions_default))
        .route("/v1/:provider/messages", post(chat_completions))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(cors)
                .layer(from_fn_with_state(
                    config.clone(),
                    middleware::auth_middleware,
                )),
        )
        .with_state(proxy_state)
        .fallback(fallback_handler);

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
    info!("To add a new format, just implement conversion functions in formats.rs!");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
