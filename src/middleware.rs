use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, Response, StatusCode},
    middleware::Next,
    response::IntoResponse,
};
use std::sync::Arc;

use crate::config::Config;

pub async fn auth_middleware(
    State(config): State<Arc<Config>>,
    request: Request,
    next: Next,
) -> impl IntoResponse {
    // Check for authorization header
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());

    let valid = auth_header.map_or(false, |auth| {
        // Support "Bearer <token>" or just "<token>" format
        let token = auth
            .strip_prefix("Bearer ")
            .unwrap_or(auth)
            .trim();
        token == config.server.api_key
    });

    if !valid {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({
                    "error": {
                        "message": "Invalid or missing API key",
                        "type": "authentication_error"
                    }
                })
                .to_string(),
            ))
            .unwrap();
    }

    next.run(request).await
}
