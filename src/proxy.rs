use axum::{
    body::{Body, Bytes},
    extract::{Path, State},
    http::{header, HeaderMap, Response, StatusCode},
    response::{IntoResponse, Json},
};
use futures::stream::StreamExt;
use reqwest::Client;
use serde_json::Value;
use std::sync::Arc;
use tracing::{error, info};

use crate::config::{Config, InputFormat as ConfigInputFormat, OutputFormat as ConfigOutputFormat};
use crate::protocols::{format_response, parse_request, CanonicalChatRequest, InputFormat, OutputFormat};

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

    fn get_provider(
        &self,
        provider_name: Option<&str>,
    ) -> Option<(String, crate::config::ProviderConfig)> {
        match provider_name {
            Some(name) => self
                .config
                .providers
                .get(name)
                .cloned()
                .map(|p| (name.to_string(), p)),
            None => {
                // Return first provider if no name specified
                self.config
                    .providers
                    .iter()
                    .next()
                    .map(|(k, v)| (k.clone(), v.clone()))
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
    // Get provider configuration first
    let (provider_name, provider_config) = match state.get_provider(provider.as_deref()) {
        Some(p) => p,
        None => {
            return error_response(StatusCode::BAD_REQUEST, "No provider configured");
        }
    };

    // Use configured input format
    let input_format = match provider_config.input {
        ConfigInputFormat::Anthropic => InputFormat::Anthropic,
        ConfigInputFormat::Ollama => InputFormat::Ollama,
        ConfigInputFormat::OpenAi => InputFormat::OpenAI,
    };
    info!("Using input format: {:?} for provider: {}", input_format, provider_name);

    // Parse into canonical format
    let canonical_request: CanonicalChatRequest = match parse_request(input_format, &body) {
        Ok(req) => req,
        Err(e) => {
            error!("Failed to parse request: {}", e);
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("Invalid request body: {}", e),
            );
        }
    };

    // Check if streaming is requested
    let is_streaming = canonical_request.stream.unwrap_or(false);

    // Forward request to upstream provider (always as OpenAI format)
    let upstream_url = format!(
        "{}/chat/completions",
        provider_config.base_url.trim_end_matches('/')
    );

    info!(
        "Forwarding request to provider: {} at {}",
        provider_name, upstream_url
    );

    let mut upstream_request = state
        .client
        .post(&upstream_url)
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", provider_config.api_key),
        )
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(&canonical_request);

    // Copy relevant headers
    for (key, value) in headers.iter() {
        let key_str = key.as_str().to_lowercase();
        if key_str == "x-request-id" || key_str.starts_with("x-") {
            if let Ok(reqwest_header) = value.to_str() {
                upstream_request = upstream_request.header(key.as_str(), reqwest_header);
            }
        }
    }

    let upstream_response = match upstream_request.send().await {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to forward request: {}", e);
            return error_response(
                StatusCode::BAD_GATEWAY,
                &format!("Failed to connect to upstream: {}", e),
            );
        }
    };

    let status = upstream_response.status();

    // Handle streaming responses
    if is_streaming && status.is_success() {
        let stream = upstream_response
            .bytes_stream()
            .map(move |chunk| match chunk {
                Ok(bytes) => Ok::<_, std::convert::Infallible>(bytes),
                Err(e) => {
                    error!("Stream error: {}", e);
                    Ok(bytes::Bytes::new())
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
        let axum_status = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
        return Response::builder()
            .status(axum_status)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(response_body))
            .unwrap();
    }

    // Determine output format from config
    let output_format = match provider_config.output {
        ConfigOutputFormat::Anthropic => OutputFormat::Anthropic,
        ConfigOutputFormat::Ollama => OutputFormat::Ollama,
        ConfigOutputFormat::OpenAiCompatible => OutputFormat::OpenAI,
    };

    // Parse upstream response (always OpenAI format)
    let canonical_response: crate::protocols::CanonicalChatResponse =
        match serde_json::from_slice(&response_body) {
            Ok(resp) => resp,
            Err(e) => {
                error!("Failed to parse upstream response: {}", e);
                // Return original response if parsing fails
                return Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(response_body))
                    .unwrap();
            }
        };

    // Format the response to desired output format
    match format_response(output_format, &canonical_response) {
        Ok(formatted) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(formatted))
            .unwrap(),
        Err(e) => {
            error!("Failed to format response: {}", e);
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to format response",
            )
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
pub async fn list_models(State(state): State<Arc<ProxyState>>) -> impl IntoResponse {
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
