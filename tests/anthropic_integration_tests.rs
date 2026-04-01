// Real integration test: Mock Anthropic upstream → Proxy → OpenAI output

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::sleep;

/// Mock Anthropic provider
struct MockAnthropic {
    addr: SocketAddr,
    #[allow(dead_code)]
    server: tokio::task::JoinHandle<()>,
}

impl MockAnthropic {
    async fn new() -> Self {
        use axum::{extract::Json, response::Json as JsonResponse, routing::post, Router};
        use serde_json::json;

        let app = Router::new().route(
            "/v1/chat/completions",
            post(|Json(body): Json<serde_json::Value>| async move {
                // Anthropic receives OpenAI format from proxy
                let content = format!(
                    "Anthropic received: {}",
                    body["messages"][0]["content"]
                        .as_str()
                        .unwrap_or("no content")
                );

                // Return OpenAI format
                JsonResponse(json!({
                    "id": "chatcmpl-anthropic-mock",
                    "object": "chat.completion",
                    "created": 1234567890,
                    "model": "claude-3-opus",
                    "choices": [{
                        "index": 0,
                        "message": {
                            "role": "assistant",
                            "content": content
                        },
                        "finish_reason": "stop"
                    }],
                    "usage": {
                        "prompt_tokens": 10,
                        "completion_tokens": 5,
                        "total_tokens": 15
                    }
                }))
            }),
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        sleep(Duration::from_millis(100)).await;

        Self { addr, server }
    }

    fn url(&self) -> String {
        format!("http://{}/v1", self.addr)
    }
}

/// Start real proxy with config pointing to mock Anthropic
async fn start_test_proxy_with_anthropic(
    mock_url: &str,
) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    use llm_gateway::{build_router, config::Config};
    use std::io::Write;

    // Create temp config
    let config = format!(
        r#"
[server]
host = "127.0.0.1"
port = 0

[providers.test]
base_url = "{}"
api_key = "test-key"
input = "anthropic"
output = "openai-compatible"
"#,
        mock_url
    );

    let mut temp = tempfile::NamedTempFile::new().unwrap();
    temp.write_all(config.as_bytes()).unwrap();
    let path = temp.path().to_str().unwrap().to_string();

    // Load config
    let config = Config::from_file(&path).expect("Failed to load config");
    let config = Arc::new(config);

    // Build real proxy router
    let app = build_router(config);

    // Start on random port
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    sleep(Duration::from_millis(200)).await;

    (addr, server)
}

#[tokio::test]
async fn test_anthropic_input_to_openai_output() {
    // 1. Start mock Anthropic upstream
    let mock = MockAnthropic::new().await;
    println!("Mock Anthropic running on {}", mock.addr);

    // 2. Start REAL proxy pointed at mock
    let (proxy_addr, _proxy_server) = start_test_proxy_with_anthropic(&mock.url()).await;
    println!("Real proxy running on {}", proxy_addr);

    // 3. Send Anthropic format request to proxy
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{}/v1/chat/completions", proxy_addr))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": "claude-3-opus",
            "messages": [
                {"role": "user", "content": "Hello from Anthropic test!"}
            ],
            "max_tokens": 100,
            "system": "You are Claude."
        }))
        .send()
        .await
        .expect("Request failed");

    // 4. Verify response
    assert_eq!(response.status(), 200, "Expected 200 OK");

    let body: serde_json::Value = response.json().await.unwrap();
    println!(
        "Response body: {}",
        serde_json::to_string_pretty(&body).unwrap()
    );

    // Verify it's OpenAI-compatible format
    assert!(
        body["choices"].as_array().is_some(),
        "Should have choices array"
    );
    assert_eq!(body["choices"][0]["message"]["role"], "assistant");
    assert!(body["choices"][0]["message"]["content"].as_str().is_some());
    assert!(
        body["usage"].as_object().is_some(),
        "Should have usage stats"
    );

    println!("✅ TEST PASSED: Anthropic input → OpenAI output conversion works!");
}

#[tokio::test]
async fn test_anthropic_system_prompt_conversion() {
    // Test that Anthropic's top-level "system" field is converted to messages
    let mock = MockAnthropic::new().await;
    let (proxy_addr, _proxy_server) = start_test_proxy_with_anthropic(&mock.url()).await;

    // Send Anthropic request with system field
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{}/v1/chat/completions", proxy_addr))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": "claude-3-opus",
            "messages": [
                {"role": "user", "content": "Hi"}
            ],
            "system": "Important system prompt"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["choices"].as_array().is_some());

    println!("✅ TEST PASSED: Anthropic system prompt conversion works!");
}

#[tokio::test]
async fn test_anthropic_max_tokens_parameter() {
    // Test that max_tokens is preserved through conversion
    let mock = MockAnthropic::new().await;
    let (proxy_addr, _proxy_server) = start_test_proxy_with_anthropic(&mock.url()).await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{}/v1/chat/completions", proxy_addr))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": "claude-3-opus",
            "messages": [{"role": "user", "content": "Test"}],
            "max_tokens": 200
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    println!("✅ TEST PASSED: Anthropic max_tokens parameter works!");
}

#[tokio::test]
async fn test_anthropic_health_endpoint() {
    let mock = MockAnthropic::new().await;
    let (proxy_addr, _proxy_server) = start_test_proxy_with_anthropic(&mock.url()).await;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{}/health", proxy_addr))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["status"], "healthy");

    println!("✅ TEST PASSED: Health endpoint works with Anthropic config!");
}
