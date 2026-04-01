// Real integration test: Mock Ollama upstream → Proxy → OpenAI output

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::{sleep, timeout};

/// Mock Ollama provider
struct MockOllama {
    addr: SocketAddr,
    #[allow(dead_code)]
    server: tokio::task::JoinHandle<()>,
}

impl MockOllama {
    async fn new() -> Self {
        use axum::{extract::Json, response::Json as JsonResponse, routing::post, Router};
        use serde_json::json;

        let app = Router::new().route(
            "/v1/chat/completions",
            post(|Json(body): Json<serde_json::Value>| async move {
                // Ollama accepts OpenAI format from proxy
                let content = format!(
                    "Ollama received: {}",
                    body["messages"][0]["content"]
                        .as_str()
                        .unwrap_or("no content")
                );

                // Return OpenAI format (Ollama actually returns OpenAI-compatible)
                JsonResponse(json!({
                    "id": "chatcmpl-ollama-mock",
                    "object": "chat.completion",
                    "created": 1234567890,
                    "model": "llama2",
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

/// Start real proxy with config pointing to mock Ollama
async fn start_test_proxy_with_ollama(mock_url: &str) -> (SocketAddr, tokio::task::JoinHandle<()>) {
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
input = "ollama"
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
async fn test_ollama_input_to_openai_output() {
    // 1. Start mock Ollama upstream
    let mock = MockOllama::new().await;
    println!("Mock Ollama running on {}", mock.addr);

    // 2. Start REAL proxy pointed at mock
    let (proxy_addr, _proxy_server) = start_test_proxy_with_ollama(&mock.url()).await;
    println!("Real proxy running on {}", proxy_addr);

    // 3. Send Ollama format request to proxy
    let client = reqwest::Client::new();
    let response = timeout(
        Duration::from_secs(5),
        client
            .post(format!("http://{}/v1/chat/completions", proxy_addr))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "model": "llama2",
                "prompt": "Hello from test!",
                "system": "You are helpful."
            }))
            .send(),
    )
    .await
    .expect("Request timed out")
    .expect("Request failed");

    // 4. Verify response
    assert_eq!(response.status(), 200, "Expected 200 OK");

    let body: serde_json::Value = response.json().await.unwrap();
    println!(
        "Response body: {}",
        serde_json::to_string_pretty(&body).unwrap()
    );

    // Verify it's OpenAI-compatible format (not Ollama format)
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

    // Verify content contains expected message
    let content = body["choices"][0]["message"]["content"].as_str().unwrap();
    assert!(content.contains("Ollama received:") || content.contains("Hello from test!"));

    println!("✅ TEST PASSED: Ollama input → OpenAI output conversion works!");
}

#[tokio::test]
async fn test_proxy_with_ollama_chat_messages() {
    // Test Ollama chat format (messages array instead of prompt)
    let mock = MockOllama::new().await;
    let (proxy_addr, _proxy_server) = start_test_proxy_with_ollama(&mock.url()).await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{}/v1/chat/completions", proxy_addr))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": "llama2",
            "messages": [
                {"role": "system", "content": "You are helpful."},
                {"role": "user", "content": "Test message"}
            ]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["choices"].as_array().is_some());

    println!("✅ TEST PASSED: Ollama chat messages format works!");
}

#[tokio::test]
async fn test_system_prompt_forwarding_through_proxy() {
    use std::sync::atomic::{AtomicBool, Ordering};

    // Track if system prompt was received by mock
    let system_received = Arc::new(AtomicBool::new(false));
    let system_received_clone = system_received.clone();

    // Create custom mock that checks for system
    let app = axum::Router::new().route(
        "/v1/chat/completions",
        axum::routing::post(
            move |axum::extract::Json(body): axum::extract::Json<serde_json::Value>| async move {
                let messages = body["messages"].as_array().unwrap();

                // Check if first message is system
                if messages
                    .first()
                    .map(|m| m["role"] == "system")
                    .unwrap_or(false)
                {
                    system_received_clone.store(true, Ordering::SeqCst);
                }

                axum::response::Json(serde_json::json!({
                    "id": "test",
                    "choices": [{
                        "message": {"role": "assistant", "content": "OK"}
                    }]
                }))
            },
        ),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mock_addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    sleep(Duration::from_millis(100)).await;

    let mock_url = format!("http://{}/v1", mock_addr);
    let (proxy_addr, _proxy_server) = start_test_proxy_with_ollama(&mock_url).await;

    // Send request with system prompt
    let client = reqwest::Client::new();
    let _response = client
        .post(format!("http://{}/v1/chat/completions", proxy_addr))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": "llama2",
            "prompt": "Hello",
            "system": "Important system prompt"
        }))
        .send()
        .await
        .unwrap();

    // Verify system prompt was forwarded
    assert!(
        system_received.load(Ordering::SeqCst),
        "System prompt should be forwarded to upstream"
    );

    println!("✅ TEST PASSED: System prompt forwarded correctly!");
}

#[tokio::test]
async fn test_proxy_health_with_running_server() {
    let mock = MockOllama::new().await;
    let (proxy_addr, _proxy_server) = start_test_proxy_with_ollama(&mock.url()).await;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{}/health", proxy_addr))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["status"], "healthy");

    println!("✅ TEST PASSED: Health endpoint works!");
}
