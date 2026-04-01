// Library exports for testing

pub mod config;
pub mod middleware;
pub mod protocols;
pub mod proxy;

use std::net::SocketAddr;
use std::path::Path;
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

use config::Config;
use proxy::{
    chat_completions, chat_completions_default, fallback_handler, health_check, list_models,
    ProxyState,
};

/// Build the router from a config
pub fn build_router(config: Arc<Config>) -> Router {
    // Create proxy state
    let proxy_state = Arc::new(ProxyState::new(config.clone()));

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build router
    Router::new()
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
        .fallback(fallback_handler)
}

/// Start a test server with given config file
pub async fn start_test_server<P: AsRef<Path>>(config_path: P) -> TestServer {
    // Load config from temp file
    let path_str = config_path.as_ref().to_str().expect("Invalid path");
    let config = Config::from_file(path_str).expect("Failed to load test config");
    let config = Arc::new(config);
    let app = build_router(config);

    // Bind to random port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind");
    let addr = listener.local_addr().expect("Failed to get local addr");

    // Start server in background
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("Server failed");
    });

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    TestServer {
        addr,
        _server: server,
    }
}

/// Test server handle
pub struct TestServer {
    pub addr: SocketAddr,
    _server: tokio::task::JoinHandle<()>,
}
