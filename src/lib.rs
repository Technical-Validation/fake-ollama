
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use clap::Parser;
use crypto_hash::{hex_digest, Algorithm};
use futures_util::{stream, StreamExt, TryStreamExt};
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io;
use std::net::SocketAddr;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Remote server URL
    #[arg(short, long)]
    url: String,

    /// API key for the remote server
    #[arg(short, long)]
    api_key: String,

    /// Enabled models, separated by commas
    #[arg(long, value_delimiter = ',', num_args = 1..)]
    enabled_models: Vec<String>,
}

// Structs for /api/chat
#[derive(Serialize, Deserialize, Debug)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    temperature: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct OllamaChatResponse {
    model: String,
    created_at: String,
    message: Message,
    done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    load_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_eval_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_eval_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eval_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eval_duration: Option<u64>,
}

// Structs for /api/generate
#[derive(Serialize, Deserialize, Debug)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Serialize, Deserialize, Debug)]
struct OllamaGenerateResponse {
    model: String,
    created_at: String,
    response: String,
    done: bool,
}

// Structs for /api/tags
#[derive(Serialize, Deserialize, Debug)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Serialize, Deserialize, Debug)]
struct OllamaModel {
    name: String,
    model: String,
    modified_at: String,
    size: u64,
    digest: String,
    details: OllamaModelDetails,
}

#[derive(Serialize, Deserialize, Debug)]
struct OllamaModelDetails {
    parent_model: String,
    format: String,
    family: String,
    families: Vec<String>,
    parameter_size: String,
    quantization_level: String,
}

async fn root_handler() -> &'static str {
    "Ollama is running"
}

async fn chat_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<OllamaChatRequest>,
) -> impl IntoResponse {
    println!("---\nRequest Headers:\n{:#?}", headers);
    println!(
        "Request Body:\n{}\n---",
        serde_json::to_string_pretty(&payload).unwrap()
    );
    forward_to_api(state, payload.messages, payload.model, payload.stream).await
}

async fn v1_chat_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<OllamaChatRequest>,
) -> impl IntoResponse {
    println!("---\nRequest Headers:\n{:#?}", headers);
    println!(
        "Request Body:\n{}\n---",
        serde_json::to_string_pretty(&payload).unwrap()
    );
    forward_to_api(state, payload.messages, payload.model, payload.stream).await
}

async fn generate_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<OllamaGenerateRequest>,
) -> impl IntoResponse {
    println!("---\nRequest Headers:\n{:#?}", headers);
    println!(
        "Request Body:\n{}\n---",
        serde_json::to_string_pretty(&payload).unwrap()
    );
    let messages = vec![Message {
        role: "user".to_string(),
        content: payload.prompt.clone(),
    }];
    forward_to_api(state, messages, payload.model, payload.stream).await
}

async fn tags_handler(State(state): State<AppState>) -> impl IntoResponse {
    println!("Received /api/tags request");
    let models = state.args.enabled_models.iter().map(|model_name| {
        let re_family = Regex::new(r"^([a-zA-Z0-9]+)").unwrap();
        let family = re_family
            .captures(model_name)
            .and_then(|cap| cap.get(1))
            .map_or("unknown", |m| m.as_str());

        let (format, size, parameter_size, quantization_level) = if model_name.contains("llama") {
            ("gguf", 1234567890, "405B", "Q4_0")
        } else if model_name.contains("mistral") {
            ("gguf", 1234567890, "unknown", "unknown")
        } else {
            ("unknown", 9876543210, "unknown", "unknown")
        };

        OllamaModel {
            name: model_name.clone(),
            model: model_name.clone(),
            modified_at: Utc::now().to_rfc3339(),
            size,
            digest: hex_digest(Algorithm::SHA256, model_name.as_bytes()),
            details: OllamaModelDetails {
                parent_model: "".to_string(),
                format: format.to_string(),
                family: family.to_string(),
                families: vec![family.to_string()],
                parameter_size: parameter_size.to_string(),
                quantization_level: quantization_level.to_string(),
            },
        }
    })
    .collect();

    Json(OllamaTagsResponse { models }).into_response()
}

async fn forward_to_api(
    state: AppState,
    messages: Vec<Message>,
    model: String,
    stream: bool,
) -> Response {
    let client = &state.client;
    let args = state.args.clone();
    let remote_url = format!("{}{}", args.url, "/v1/chat/completions");

    let model_for_stream = model.clone();

    let chat_request = OllamaChatRequest {
        model,
        messages,
        stream,
        temperature: Some(0.7),
    };

    let res = client
        .post(&remote_url)
        .bearer_auth(&args.api_key)
        .json(&chat_request)
        .send()
        .await;

    let res = match res {
        Ok(res) => res,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!("Error forwarding request: {}", e)))
                .unwrap();
        }
    };

    if !res.status().is_success() {
        let status = res.status();
        let body_bytes = res.bytes().await.unwrap_or_default();
        return Response::builder()
            .status(status)
            .body(Body::from(body_bytes))
            .unwrap();
    }

    if stream {
        let stream = res
            .bytes_stream()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e));
        let transformed_stream = stream.flat_map(|chunk| {
            let chunk = chunk.unwrap_or_default();
            let lines = chunk.split(|&b| b == b'\n');
            stream::iter(lines.map(|line| Ok::<_, io::Error>(line.to_vec())).collect::<Vec<_>>())
        });

        let body = Body::from_stream(transformed_stream.and_then(move |chunk| {
            let model = model_for_stream.clone();
            async move {
                let mut line = String::from_utf8(chunk.to_vec()).unwrap_or_default();
                if line.starts_with("data: ") {
                    line = line[6..].to_string();
                }
                if line.trim() == "[DONE]" {
                    let final_response = OllamaChatResponse {
                        model: model.clone(),
                        created_at: Utc::now().to_rfc3339(),
                        message: Message { role: "assistant".to_string(), content: "".to_string() },
                        done: true,
                        total_duration: Some(0),
                        load_duration: Some(0),
                        prompt_eval_count: Some(0),
                        prompt_eval_duration: Some(0),
                        eval_count: Some(0),
                        eval_duration: Some(0),
                    };
                    let response_body = serde_json::to_string(&final_response).unwrap() + "\n";
                    // println!("Response Body:\n{}", response_body);
                    return Ok(response_body);
                }
                if let Ok(val) = serde_json::from_str::<Value>(&line) {
                    if let Some(choices) = val.get("choices").and_then(|c| c.as_array()) {
                        if let Some(delta) = choices[0].get("delta") {
                            if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                if !content.is_empty() {
                                    let response = OllamaChatResponse {
                                        model: model.clone(),
                                        created_at: Utc::now().to_rfc3339(),
                                        message: Message { role: "assistant".to_string(), content: content.to_string() },
                                        done: false,
                                        total_duration: None,
                                        load_duration: None,
                                        prompt_eval_count: None,
                                        prompt_eval_duration: None,
                                        eval_count: None,
                                        eval_duration: None,
                                    };
                                    let response_body = serde_json::to_string(&response).unwrap() + "\n";
                                    // println!("Response Body:\n{}", response_body);
                                    return Ok(response_body);
                                }
                            }
                        }
                    }
                }
                Ok("".to_string())
            }
        }));
        Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/x-ndjson")
            .body(body)
            .unwrap()
    } else {
        let body_bytes = res.bytes().await.unwrap_or_default();
        // println!("Response Body:\n{}", String::from_utf8_lossy(&body_bytes));
        let val: Value = serde_json::from_slice(&body_bytes).unwrap();
        let content = val["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("");
        let prompt_tokens = val["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32;
        let completion_tokens = val["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32;
        let total_duration = val["usage"]["total_tokens"].as_u64().unwrap_or(0) * 100000;

        let response = OllamaChatResponse {
            model: chat_request.model,
            created_at: Utc::now().to_rfc3339(),
            message: Message {
                role: "assistant".to_string(),
                content: content.to_string(),
            },
            done: true,
            total_duration: Some(total_duration),
            load_duration: Some(1234567),
            prompt_eval_count: Some(prompt_tokens),
            prompt_eval_duration: Some(prompt_tokens as u64 * 100000),
            eval_count: Some(completion_tokens),
            eval_duration: Some(completion_tokens as u64 * 100000),
        };
        Json(response).into_response()
    }
}

#[derive(Clone)]
struct AppState {
    client: Client,
    args: Args,
}

pub async fn main() {
    let args = Args::parse();

    let state = AppState {
        client: Client::new(),
        args: args.clone(),
    };

    let app = Router::new()
        .route("/", get(root_handler))
        .route("/api/chat", post(chat_handler))
        .route("/v1/chat/completions", post(v1_chat_handler))
        .route("/api/generate", post(generate_handler))
        .route("/api/tags", get(tags_handler))
        .with_state(state.clone());

    let addr = SocketAddr::from(([127, 0, 0, 1], 11434));
    println!("Fake Ollama is listening on http://{}", addr);
    println!("----------------------------------------");
    println!("Configuration:");
    println!("  Remote server URL: {}", &args.url);
    println!("  Enabled models: {:?}", &args.enabled_models);
    println!("----------------------------------------");
    println!("Available API Endpoints:");
    println!("  GET    /");
    println!("  POST   /api/chat");
    println!("  POST   /v1/chat/completions");
    println!("  POST   /api/generate");
    println!("  GET    /api/tags");
    println!("----------------------------------------");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}


