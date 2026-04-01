mod config;
mod middleware;
mod models;
mod proxy;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    middleware::from_fn_with_state,
    routing::{any, get, post},
    Router,
};
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::{info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use config::Config;
use proxy::{ProxyState, chat_completions, health_check, list_models, proxy_request};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            format!("{}=debug,tower_http=debug", env!("CARGO_PKG_NAME")).into()
        }))
        .with(tracing_subscriber::fmt::layer())
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
        // OpenAI-compatible endpoints
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/models", get(list_models))
        // Provider-specific endpoints
        .route("/v1/:provider/chat/completions", post(chat_completions))
        .route("/v1/:provider/models", get(list_models))
        .route("/v1/:provider/*path", any(proxy_request))
        // Anthropic-compatible endpoints
        .route("/v1/messages", post(chat_completions))
        .route("/v1/:provider/messages", post(chat_completions))
        // Generic proxy for any other paths
        .route("/*path", any(proxy_request))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(cors)
                .layer(from_fn_with_state(
                    config.clone(),
                    middleware::auth_middleware,
                )),
        )
        .with_state(proxy_state);

    // Start server
    let addr = SocketAddr::from((
        config.server.host.parse::<std::net::IpAddr>()?,
        config.server.port,
    ));

    info!("Starting server on {}", addr);
    info!("Available endpoints:");
    info!("  POST /v1/chat/completions - Chat completions (OpenAI format)");
    info!("  POST /v1/messages - Chat completions (Anthropic format)");
    info!("  GET  /v1/models - List available models");
    info!("  GET  /health - Health check");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
