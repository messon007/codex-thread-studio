use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;

const MAX_SPEECH_CHARS: usize = 16_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakRequest {
    text: String,
    language: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechStatus {
    available: bool,
    engine: Option<&'static str>,
}

pub async fn status() -> Json<SpeechStatus> {
    Json(platform_status().await)
}

pub async fn speak(Json(request): Json<SpeakRequest>) -> Response {
    let text = request.text.trim();
    if text.is_empty() || text.chars().count() > MAX_SPEECH_CHARS {
        return error(
            StatusCode::BAD_REQUEST,
            "speech text must contain 1 to 16000 characters",
        );
    }
    if !matches!(
        request.language.as_str(),
        "en-US" | "zh-CN" | "zh-CN-pinyin"
    ) {
        return error(
            StatusCode::BAD_REQUEST,
            "speech language must be en-US, zh-CN, or zh-CN-pinyin",
        );
    }

    match platform_speak(text, &request.language).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(message) => error(StatusCode::SERVICE_UNAVAILABLE, &message),
    }
}

pub async fn stop() -> Response {
    match platform_stop().await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(message) => error(StatusCode::SERVICE_UNAVAILABLE, &message),
    }
}

fn error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": { "message": message } }))).into_response()
}

#[cfg(target_os = "linux")]
async fn platform_status() -> SpeechStatus {
    let available = tokio::process::Command::new("spd-say")
        .arg("--version")
        .output()
        .await
        .is_ok_and(|output| output.status.success());
    SpeechStatus {
        available,
        engine: available.then_some("speech-dispatcher"),
    }
}

#[cfg(target_os = "linux")]
async fn platform_speak(text: &str, language: &str) -> Result<(), String> {
    use std::process::Stdio;

    let language = speech_dispatcher_language(language);
    let child = tokio::process::Command::new("spd-say")
        .args([
            "--wait",
            "--priority",
            "important",
            "--application-name",
            "Codex Thread Studio",
            "--language",
            language,
            "--",
        ])
        .arg(text)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Unable to start Speech Dispatcher: {error}"))?;
    let output = child
        .wait_with_output()
        .await
        .map_err(|error| format!("Unable to wait for Speech Dispatcher: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if detail.is_empty() {
            "Speech Dispatcher could not play the text".to_string()
        } else {
            detail
        })
    }
}

#[cfg(target_os = "linux")]
fn speech_dispatcher_language(language: &str) -> &'static str {
    match language {
        "zh-CN" => "cmn",
        "zh-CN-pinyin" => "cmn-LATN-PINYIN",
        _ => "en-US",
    }
}

#[cfg(target_os = "linux")]
async fn platform_stop() -> Result<(), String> {
    let output = tokio::process::Command::new("spd-say")
        .arg("--cancel")
        .output()
        .await
        .map_err(|error| format!("Unable to stop Speech Dispatcher: {error}"))?;
    output
        .status
        .success()
        .then_some(())
        .ok_or_else(|| "Speech Dispatcher could not stop playback".to_string())
}

#[cfg(not(target_os = "linux"))]
async fn platform_status() -> SpeechStatus {
    SpeechStatus {
        available: false,
        engine: None,
    }
}

#[cfg(not(target_os = "linux"))]
async fn platform_speak(_text: &str, _language: &str) -> Result<(), String> {
    Err("Native speech synthesis is unavailable on this system".to_string())
}

#[cfg(not(target_os = "linux"))]
async fn platform_stop() -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speech_request_uses_bounded_text_and_known_languages() {
        assert_eq!(MAX_SPEECH_CHARS, 16_000);
        let request: SpeakRequest = serde_json::from_value(json!({
            "text": "hello",
            "language": "en-US"
        }))
        .expect("valid speech request");
        assert_eq!(request.text, "hello");
        assert_eq!(request.language, "en-US");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn speech_dispatcher_uses_its_mandarin_voice_identifier() {
        assert_eq!(speech_dispatcher_language("zh-CN"), "cmn");
        assert_eq!(
            speech_dispatcher_language("zh-CN-pinyin"),
            "cmn-LATN-PINYIN"
        );
        assert_eq!(speech_dispatcher_language("en-US"), "en-US");
    }
}
