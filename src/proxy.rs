use axum::{
    body::{Body, Bytes},
    extract::{Path, Request, State},
    http::{header, HeaderMap, Response, StatusCode},
    response::{IntoResponse, Json},
};
use futures::stream::StreamExt;
use reqwest::Client;
use serde_json::Value;
use std::sync::Arc;
use tracing::{error, info};

use crate::config::{Config, OutputFormat};
use crate::models::{
    AnthropicRequest, OpenAIChatRequest, OpenAIChatResponse, RequestSource,
};

#[derive(Clone)]
pub struct ProxyState {
    pub config: Arc<Config>,
    pub client: Client,
}

impl ProxyState {
    pub fn new(config: Arc<Config>) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("Failed to create HTTP client");

        Self { config, client }
    }

    fn get_provider(&self, provider_name: Option<&str>) -> Option<(String, crate::config::ProviderConfig)> {
        match provider_name {
            Some(name) => self.config.providers.get(name).cloned().map(|p| (name.to_string(), p)),
            None => {
                // Return first provider if no name specified
                self.config.providers.iter().next().map(|(k, v)| (k.clone(), v.clone()))
            }
        }
    }
}

pub async fn chat_completions(
    State(state): State<Arc<ProxyState>>,
    Path(provider): Path<Option<String>>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    // Detect request source
    let source = RequestSource::from_headers(&headers);
    info!("Received request from source: {:?}", source);

    // Get provider configuration
    let (provider_name, provider_config) = match state.get_provider(provider.as_deref()) {
        Some(p) => p,
        None => {
            return error_response(StatusCode::BAD_REQUEST, "No provider configured");
        }
    };

    // Parse request based on source
    let openai_request: OpenAIChatRequest = match source {
        RequestSource::Anthropic => {
            match serde_json::from_slice::<AnthropicRequest>(&body) {
                Ok(req) => req.to_openai(),
                Err(e) => {
                    error!("Failed to parse Anthropic request: {}", e);
                    return error_response(StatusCode::BAD_REQUEST, &format!("Invalid request body: {}", e));
                }
            }
        }
        _ => {
            // OpenAI or Zai format - already OpenAI-compatible
            match serde_json::from_slice::<OpenAIChatRequest>(&body) {
                Ok(req) => req,
                Err(e) => {
                    error!("Failed to parse OpenAI request: {}", e);
                    return error_response(StatusCode::BAD_REQUEST, &format!("Invalid request body: {}", e));
                }
            }
        }
    };

    // Check if streaming is requested
    let is_streaming = openai_request.stream.unwrap_or(false);

    // Forward request to upstream provider
    let upstream_url = format!("{}/chat/completions", provider_config.base_url.trim_end_matches('/'));
    
    info!("Forwarding request to provider: {} at {}", provider_name, upstream_url);

    let mut upstream_request = state
        .client
        .post(&upstream_url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", provider_config.api_key))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(&openai_request);

    // Copy relevant headers
    for (key, value) in headers.iter() {
        let key_str = key.as_str().to_lowercase();
        if key_str == "x-request-id" || key_str.starts_with("x-") {
            // Convert axum header to reqwest header
            if let Ok(reqwest_header) = value.to_str() {
                upstream_request = upstream_request.header(key.as_str(), reqwest_header);
            }
        }
    }

    let upstream_response = match upstream_request.send().await {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to forward request: {}", e);
            return error_response(StatusCode::BAD_GATEWAY, &format!("Failed to connect to upstream: {}", e));
        }
    };

    let status = upstream_response.status();

    // Handle streaming responses
    if is_streaming && status.is_success() {
        let stream = upstream_response.bytes_stream().map(move |chunk| {
            match chunk {
                Ok(bytes) => {
                    // Pass through SSE chunks
                    Ok::<_, std::convert::Infallible>(bytes)
                }
                Err(e) => {
                    error!("Stream error: {}", e);
                    Ok(bytes::Bytes::new())
                }
            }
        });

        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/event-stream")
            .header("Cache-Control", "no-cache")
            .body(Body::from_stream(stream))
            .unwrap();
    }

    // Handle non-streaming responses
    let response_body = match upstream_response.bytes().await {
        Ok(body) => body,
        Err(e) => {
            error!("Failed to read response body: {}", e);
            return error_response(StatusCode::BAD_GATEWAY, "Failed to read upstream response");
        }
    };

    // Parse response and convert if needed
    if !status.is_success() {
        // Convert reqwest status to axum status
        let axum_status = StatusCode::from_u16(status.as_u16())
            .unwrap_or(StatusCode::BAD_GATEWAY);
        return Response::builder()
            .status(axum_status)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(response_body))
            .unwrap();
    }

    match provider_config.output {
        OutputFormat::OpenAiCompatible => {
            // Pass through OpenAI format
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(response_body))
                .unwrap()
        }
        OutputFormat::Anthropic => {
            // Convert OpenAI response to Anthropic format
            match serde_json::from_slice::<OpenAIChatResponse>(&response_body) {
                Ok(openai_resp) => {
                    let anthropic_resp = openai_resp.to_anthropic();
                    match serde_json::to_string(&anthropic_resp) {
                        Ok(json) => Response::builder()
                            .status(StatusCode::OK)
                            .header(header::CONTENT_TYPE, "application/json")
                            .body(Body::from(json))
                            .unwrap(),
                        Err(e) => {
                            error!("Failed to serialize Anthropic response: {}", e);
                            error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to format response")
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to parse OpenAI response: {}", e);
                    // Return original response if parsing fails
                    Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(response_body))
                        .unwrap()
                }
            }
        }
    }
}

fn error_response(status: StatusCode, message: &str) -> Response<Body> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "error": {
                    "message": message,
                    "type": "api_error"
                }
            })
            .to_string(),
        ))
        .unwrap()
}

// Health check endpoint
pub async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

// List models endpoint
pub async fn list_models(
    State(state): State<Arc<ProxyState>>,
) -> impl IntoResponse {
    let models: Vec<Value> = state
        .config
        .providers
        .keys()
        .map(|name| {
            serde_json::json!({
                "id": format!("{}/default", name),
                "object": "model",
                "owned_by": name
            })
        })
        .collect();

    Json(serde_json::json!({
        "object": "list",
        "data": models
    }))
}

// Proxy any path (for other endpoints)
pub async fn proxy_request(
    State(state): State<Arc<ProxyState>>,
    Path((provider, path)): Path<(String, String)>,
    request: Request,
) -> impl IntoResponse {
    let provider_config = match state.config.providers.get(&provider) {
        Some(p) => p.clone(),
        None => {
            return error_response(StatusCode::NOT_FOUND, &format!("Provider '{}' not found", provider));
        }
    };

    let upstream_url = format!(
        "{}/{}",
        provider_config.base_url.trim_end_matches('/'),
        path
    );

    info!("Proxying request to: {}", upstream_url);

    let method = request.method().clone();
    let headers = request.headers().clone();
    let body = request.into_body();

    // Convert axum method to reqwest method
    let reqwest_method = match method {
        axum::http::Method::GET => reqwest::Method::GET,
        axum::http::Method::POST => reqwest::Method::POST,
        axum::http::Method::PUT => reqwest::Method::PUT,
        axum::http::Method::DELETE => reqwest::Method::DELETE,
        axum::http::Method::PATCH => reqwest::Method::PATCH,
        axum::http::Method::HEAD => reqwest::Method::HEAD,
        axum::http::Method::OPTIONS => reqwest::Method::OPTIONS,
        _ => reqwest::Method::from_bytes(method.as_str().as_bytes())
            .unwrap_or(reqwest::Method::GET),
    };

    let mut upstream_request = state
        .client
        .request(reqwest_method, &upstream_url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", provider_config.api_key));

    // Copy relevant headers (converting from axum to reqwest types)
    for (key, value) in headers.iter() {
        if key != header::AUTHORIZATION && key != header::HOST {
            if let Ok(value_str) = value.to_str() {
                upstream_request = upstream_request.header(key.as_str(), value_str);
            }
        }
    }

    // Read body
    let body_bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Failed to read request body: {}", e);
            return error_response(StatusCode::BAD_REQUEST, "Failed to read request body");
        }
    };

    if !body_bytes.is_empty() {
        upstream_request = upstream_request.body(body_bytes);
    }

    let upstream_response = match upstream_request.send().await {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to forward request: {}", e);
            return error_response(StatusCode::BAD_GATEWAY, &format!("Failed to connect to upstream: {}", e));
        }
    };

    let status = upstream_response.status();
    let response_body = match upstream_response.bytes().await {
        Ok(body) => body,
        Err(e) => {
            error!("Failed to read response body: {}", e);
            return error_response(StatusCode::BAD_GATEWAY, "Failed to read response body");
        }
    };

    let axum_status = StatusCode::from_u16(status.as_u16())
        .unwrap_or(StatusCode::BAD_GATEWAY);

    Response::builder()
        .status(axum_status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(response_body))
        .unwrap()
}
