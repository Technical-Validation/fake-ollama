
use fakeOllama::{run, Args};
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};
use std::thread;
use reqwest;
use futures_util::StreamExt;
use serde_json::Value;

#[tokio::test]
async fn test_chat_streaming_integration() {
    // 1. Set up a mock server
    let mock_server = MockServer::start().await;

    let response_body = "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}

data: {\"choices\":[{\"delta\":{\"content\":\" World\"}}]}

data: [DONE]
";

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_string(response_body))
        .mount(&mock_server)
        .await;

    // 2. Run the fakeOllama server in the background
    let server_uri = mock_server.uri();
    let args = Args {
        url: server_uri,
        api_key: "test_api_key".to_string(),
        enabled_models: vec!["llama2".to_string(), "mistral".to_string()],
    };
    
    thread::spawn(move || {
        let _ = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                run(args).await;
            });
    });
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // 3. Make a request to the fakeOllama server
    let client = reqwest::Client::new();
    let res = client.post("http://127.0.0.1:11434/api/chat")
        .json(&serde_json::json!({
            "model": "llama2",
            "messages": [{"role": "user", "content": "Hey there!"}],
            "stream": true
        }))
        .send()
        .await
        .expect("Failed to send request");

    // 4. Assert the response
    assert_eq!(res.status(), reqwest::StatusCode::OK);

    let mut stream = res.bytes_stream();
    let mut responses = Vec::new();
    while let Some(item) = stream.next().await {
        let chunk = item.expect("Error while streaming");
        let s = String::from_utf8(chunk.to_vec()).unwrap();
        for line in s.lines() {
            if !line.is_empty() {
                responses.push(serde_json::from_str::<Value>(line).unwrap());
            }
        }
    }

    assert_eq!(responses.len(), 3);
    assert_eq!(responses[0]["message"]["content"], "Hello");
    assert_eq!(responses[1]["message"]["content"], " World");
    assert_eq!(responses[2]["done"], true);
}

