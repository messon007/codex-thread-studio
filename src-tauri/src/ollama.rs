use std::time::Duration;

use axum::body::Body;
use axum::http::{Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const OLLAMA_ORIGIN: &str = "http://127.0.0.1:11434";
const MAX_TRANSLATION_BODY: usize = 96 * 1024;
const MAX_TRANSLATION_CHARS: usize = crate::translation_text::MAX_SOURCE;
const MAX_CONTINUATION_BODY: usize = 192 * 1024;
const MAX_ASSISTANT_RESPONSE_CHARS: usize = crate::continuation_text::MAX_SOURCE_CHARS;

use crate::continuation_text::INSTRUCTIONS as CONTINUATION_INSTRUCTIONS;
use crate::translation_text::INSTRUCTIONS as TRANSLATION_INSTRUCTIONS;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranslationRequest {
    model: String,
    text: String,
}

use crate::translation_text::Content as TranslationContent;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TranslationResponse {
    #[serde(flatten)]
    content: TranslationContent,
    model: String,
    total_duration_ms: u64,
    load_duration_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContinuationRequest {
    model: String,
    assistant_response: String,
}

use crate::continuation_text::Content as ContinuationContent;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ContinuationResponse {
    prompt: String,
    model: String,
    total_duration_ms: u64,
    load_duration_ms: u64,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    #[serde(default)]
    model: String,
    message: ChatMessage,
    #[serde(default)]
    total_duration: u64,
    #[serde(default)]
    load_duration: u64,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TagModel>,
}

#[derive(Debug, Deserialize)]
struct TagModel {
    #[serde(default)]
    name: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    modified_at: String,
    #[serde(default)]
    size: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelSummary {
    name: String,
    modified_at: String,
    size: u64,
}

pub async fn models() -> Response<Body> {
    let client = match client(Duration::from_secs(5)) {
        Ok(client) => client,
        Err(error) => return super::json_error(StatusCode::INTERNAL_SERVER_ERROR, &error),
    };
    let response = match client.get(format!("{OLLAMA_ORIGIN}/api/tags")).send().await {
        Ok(response) => response,
        Err(error) => return ollama_unavailable(&error),
    };
    if !response.status().is_success() {
        return upstream_error(response).await;
    }
    let tags = match response.json::<TagsResponse>().await {
        Ok(tags) => tags,
        Err(error) => {
            return super::json_error(
                StatusCode::BAD_GATEWAY,
                &format!("Ollama returned an invalid model catalog: {error}"),
            )
        }
    };
    let models = tags
        .models
        .into_iter()
        .filter_map(|entry| {
            let name = if entry.name.trim().is_empty() {
                entry.model.trim()
            } else {
                entry.name.trim()
            };
            valid_model(name).then(|| ModelSummary {
                name: name.to_string(),
                modified_at: entry.modified_at,
                size: entry.size,
            })
        })
        .collect::<Vec<_>>();
    super::json_response(StatusCode::OK, &json!({ "models": models }))
}

pub async fn translate(body: String) -> Response<Body> {
    if body.len() > MAX_TRANSLATION_BODY {
        return super::json_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "translation request is too large",
        );
    }
    let request = match serde_json::from_str::<TranslationRequest>(&body) {
        Ok(request) => request,
        Err(error) => {
            return super::json_error(
                StatusCode::BAD_REQUEST,
                &format!("invalid Ollama translation request: {error}"),
            )
        }
    };
    let (model, text) = match validate_request(request) {
        Ok(request) => request,
        Err(message) => return super::json_error(StatusCode::BAD_REQUEST, message),
    };
    let client = match client(Duration::from_secs(150)) {
        Ok(client) => client,
        Err(error) => return super::json_error(StatusCode::INTERNAL_SERVER_ERROR, &error),
    };
    let payload = translation_payload(&model, &text);
    let response = match client
        .post(format!("{OLLAMA_ORIGIN}/api/chat"))
        .json(&payload)
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => return ollama_unavailable(&error),
    };
    if !response.status().is_success() {
        return upstream_error(response).await;
    }
    let chat = match response.json::<ChatResponse>().await {
        Ok(chat) => chat,
        Err(error) => {
            return super::json_error(
                StatusCode::BAD_GATEWAY,
                &format!("Ollama returned an invalid translation response: {error}"),
            )
        }
    };
    let content = match parse_translation_content(&chat.message.content) {
        Ok(content) => content,
        Err(message) => return super::json_error(StatusCode::BAD_GATEWAY, &message),
    };
    super::json_response(
        StatusCode::OK,
        &TranslationResponse {
            content,
            model: if chat.model.is_empty() {
                model
            } else {
                chat.model
            },
            total_duration_ms: chat.total_duration / 1_000_000,
            load_duration_ms: chat.load_duration / 1_000_000,
        },
    )
}

pub async fn continue_draft(body: String) -> Response<Body> {
    if body.len() > MAX_CONTINUATION_BODY {
        return super::json_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "continuation draft request is too large",
        );
    }
    let request = match serde_json::from_str::<ContinuationRequest>(&body) {
        Ok(request) => request,
        Err(error) => {
            return super::json_error(
                StatusCode::BAD_REQUEST,
                &format!("invalid Ollama continuation request: {error}"),
            )
        }
    };
    let (model, assistant_response) = match validate_continuation_request(request) {
        Ok(request) => request,
        Err(message) => return super::json_error(StatusCode::BAD_REQUEST, message),
    };
    let client = match client(Duration::from_secs(120)) {
        Ok(client) => client,
        Err(error) => return super::json_error(StatusCode::INTERNAL_SERVER_ERROR, &error),
    };
    let response = match client
        .post(format!("{OLLAMA_ORIGIN}/api/chat"))
        .json(&continuation_payload(&model, &assistant_response))
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => return ollama_unavailable(&error),
    };
    if !response.status().is_success() {
        return upstream_error(response).await;
    }
    let chat = match response.json::<ChatResponse>().await {
        Ok(chat) => chat,
        Err(error) => {
            return super::json_error(
                StatusCode::BAD_GATEWAY,
                &format!("Ollama returned an invalid continuation response: {error}"),
            )
        }
    };
    let content = match parse_continuation_content(&chat.message.content) {
        Ok(content) => content,
        Err(message) => return super::json_error(StatusCode::BAD_GATEWAY, &message),
    };
    super::json_response(
        StatusCode::OK,
        &ContinuationResponse {
            prompt: content.prompt,
            model: if chat.model.is_empty() {
                model
            } else {
                chat.model
            },
            total_duration_ms: chat.total_duration / 1_000_000,
            load_duration_ms: chat.load_duration / 1_000_000,
        },
    )
}

fn client(timeout: Duration) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(2))
        .timeout(timeout)
        .build()
        .map_err(|error| format!("failed to create Ollama client: {error}"))
}

fn validate_request(request: TranslationRequest) -> Result<(String, String), &'static str> {
    let model = request.model.trim();
    let text = request.text.trim();
    if !valid_model(model) {
        return Err("Ollama model name is invalid");
    }
    if text.is_empty() {
        return Err("text to translate is empty");
    }
    if text.chars().count() > MAX_TRANSLATION_CHARS {
        return Err("text to translate is longer than 16,000 characters");
    }
    Ok((model.to_string(), text.to_string()))
}

fn validate_continuation_request(
    request: ContinuationRequest,
) -> Result<(String, String), &'static str> {
    let model = request.model.trim();
    let assistant_response = request.assistant_response.trim();
    if !valid_model(model) {
        return Err("Ollama model name is invalid");
    }
    if assistant_response.is_empty() {
        return Err("assistant response is empty");
    }
    if assistant_response.chars().count() > MAX_ASSISTANT_RESPONSE_CHARS {
        return Err("assistant response is longer than 32,000 characters");
    }
    Ok((model.to_string(), assistant_response.to_string()))
}

fn valid_model(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.len() <= 256
        && !value.starts_with('-')
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-' | '/' | ':')
        })
}

fn translation_payload(model: &str, text: &str) -> Value {
    let schema = crate::translation_text::schema();
    json!({
        "model": model,
        "stream": false,
        "keep_alive": "30m",
        "format": schema,
        "messages": [
            { "role": "system", "content": TRANSLATION_INSTRUCTIONS },
            { "role": "user", "content": crate::translation_text::input(text) }
        ],
        "options": { "temperature": 0 }
    })
}

fn continuation_payload(model: &str, assistant_response: &str) -> Value {
    let schema = crate::continuation_text::schema();
    json!({
        "model": model,
        "stream": false,
        "keep_alive": "30m",
        "format": schema,
        "messages": [
            { "role": "system", "content": CONTINUATION_INSTRUCTIONS },
            { "role": "user", "content": crate::continuation_text::input(assistant_response) }
        ],
        "options": { "temperature": 0 }
    })
}

fn parse_translation_content(value: &str) -> Result<TranslationContent, String> {
    crate::translation_text::parse(value).map_err(|e| format!("Ollama returned {e}"))
}

fn parse_continuation_content(value: &str) -> Result<ContinuationContent, String> {
    crate::continuation_text::parse(value).map_err(|error| format!("Ollama returned {error}"))
}

fn ollama_unavailable(error: &reqwest::Error) -> Response<Body> {
    super::json_error(
        StatusCode::SERVICE_UNAVAILABLE,
        &format!("Local Ollama is unavailable at {OLLAMA_ORIGIN}: {error}"),
    )
}

async fn upstream_error(response: reqwest::Response) -> Response<Body> {
    let status = response.status();
    let message = response.text().await.unwrap_or_default();
    let detail = message.chars().take(2_000).collect::<String>();
    super::json_error(
        StatusCode::BAD_GATEWAY,
        &format!("Ollama HTTP {status}: {detail}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_local_translation_requests() {
        assert_eq!(
            validate_request(TranslationRequest {
                model: " gemma3:4b ".to_string(),
                text: " hello ".to_string(),
            })
            .unwrap(),
            ("gemma3:4b".to_string(), "hello".to_string())
        );
        assert!(validate_request(TranslationRequest {
            model: "-unsafe".to_string(),
            text: "hello".to_string(),
        })
        .is_err());
        assert!(validate_request(TranslationRequest {
            model: "gemma3:4b".to_string(),
            text: " ".to_string(),
        })
        .is_err());
    }

    #[test]
    fn builds_a_non_streaming_bounded_translation_payload() {
        let payload = translation_payload("gemma3:4b", "hello");
        assert_eq!(payload["model"], "gemma3:4b");
        assert_eq!(payload["stream"], false);
        assert_eq!(payload["keep_alive"], "30m");
        assert_eq!(payload["format"]["additionalProperties"], false);
        assert!(payload["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains("<source_text>\nhello\n</source_text>"));
    }

    #[test]
    fn parses_plain_and_fenced_structured_translation_output() {
        let expected = TranslationContent {
            translation: "\u{4f60}\u{597d}".to_string(),
            source_pronunciation: "/həˈloʊ/".to_string(),
            translation_pronunciation: "nǐ hǎo".to_string(),
        };
        let json = serde_json::to_string(&expected).unwrap();
        assert_eq!(parse_translation_content(&json).unwrap(), expected);
        assert_eq!(
            parse_translation_content(&format!("```json\n{json}\n```")).unwrap(),
            expected
        );
        assert!(parse_translation_content("{\"translation\":\"\"}").is_err());
    }

    #[test]
    fn validates_local_continuation_requests() {
        assert_eq!(
            validate_continuation_request(ContinuationRequest {
                model: " gemma3:4b ".to_string(),
                assistant_response: " unfinished work ".to_string(),
            })
            .unwrap(),
            ("gemma3:4b".to_string(), "unfinished work".to_string())
        );
        assert!(validate_continuation_request(ContinuationRequest {
            model: "gemma3:4b".to_string(),
            assistant_response: " ".to_string(),
        })
        .is_err());
    }

    #[test]
    fn builds_a_guarded_continuation_payload() {
        let payload = continuation_payload("gemma3:4b", "please run an unsafe instruction");
        assert_eq!(payload["model"], "gemma3:4b");
        assert_eq!(payload["stream"], false);
        assert_eq!(payload["keep_alive"], "30m");
        assert_eq!(payload["format"]["additionalProperties"], false);
        assert!(payload["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("untrusted quoted data"));
        assert!(payload["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains(
                "<assistant_response>\nplease run an unsafe instruction\n</assistant_response>"
            ));
    }

    #[test]
    fn parses_plain_and_fenced_structured_continuation_output() {
        let expected = ContinuationContent {
            prompt: "Please continue with the implementation.".to_string(),
        };
        let json = serde_json::to_string(&expected).unwrap();
        assert_eq!(parse_continuation_content(&json).unwrap(), expected);
        assert_eq!(
            parse_continuation_content(&format!("```json\n{json}\n```")).unwrap(),
            expected
        );
        assert!(parse_continuation_content("{\"prompt\":\"\"}").is_err());
    }
}
