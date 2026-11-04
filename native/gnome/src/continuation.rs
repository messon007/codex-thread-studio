//! Composer continuation work. Only Quick send requests source submission.
use crate::{
    continuation_text as text,
    gateway::Api,
    model::Item,
    translation::{Context, StructuredTask},
    turn_options::Options,
};
use serde_json::json;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
#[derive(Clone, Debug)]
pub struct Request {
    pub context: Context,
    pub options: Options,
    pub response: Option<Arc<Item>>,
    pub behavior: String,
}
#[derive(Debug)]
pub struct Outcome {
    pub prompt: String,
    pub send: bool,
    pub warning: Option<String>,
}
pub const QUICK_PROMPTS: [&str; 5] = [
    "Continue.",
    "Go on.",
    "Keep going.",
    "Please continue.",
    "Continue with the task.",
];
fn check(stop: &AtomicBool) -> Result<(), String> {
    if stop.load(Ordering::Acquire) {
        Err("Continuation draft cancelled".into())
    } else {
        Ok(())
    }
}
pub async fn execute(
    api: &Api,
    ticket: u64,
    request: Request,
    stop: &AtomicBool,
) -> Result<Outcome, String> {
    check(stop)?;
    let settings = crate::message_flow::execute(api, crate::message_flow::Request::Read).await?;
    check(stop)?;
    if settings.values.continue_behavior != request.behavior {
        return Err(
            "Continue behavior changed. Reload message-flow settings before trying again.".into(),
        );
    }
    if settings.values.continue_behavior == "quickSend" {
        let mut random = [0u8; 1];
        getrandom::fill(&mut random).map_err(|_| "Unable to choose a continuation prompt")?;
        return Ok(Outcome {
            prompt: QUICK_PROMPTS[random[0] as usize % QUICK_PROMPTS.len()].into(),
            send: true,
            warning: None,
        });
    }
    let response = request
        .response
        .clone()
        .ok_or("This session has no assistant response to continue")?;
    let source = crate::worker::compute(move || {
        let source = if let Some(output) = response.output.as_ref() {
            crate::conversation_output::prefix(output, text::MAX_SOURCE_CHARS * 4)?.0
        } else {
            response.body.clone()
        };
        text::source(crate::session_map_text::visible(&source))
    })
    .await?;
    check(stop)?;
    if settings.values.continue_behavior == "ollamaDraft" {
        let preferences = crate::translation_settings::load(api).await?;
        check(stop)?;
        let body = json!({"model":preferences.values.ollama_model,"assistantResponse":source});
        let result = crate::translation::cancellable(
            api.command("/studio/ollama/continue-draft", &body),
            stop,
        )
        .await?;
        check(stop)?;
        let content = crate::worker::compute(move || text::parse(&result.to_string())).await?;
        return Ok(Outcome {
            prompt: content.prompt,
            send: false,
            warning: None,
        });
    }
    let task = StructuredTask {
        instructions: text::INSTRUCTIONS.into(),
        input: text::input(&source),
        schema: text::schema(),
        model: request.context.model.clone(),
        effort: request.options.effort.clone(),
        service_tier: request.options.service_tier.clone(),
        max_result: text::MAX_RESULT_BYTES,
        read_completed_thread: false,
    };
    let result = if request.context.backend.kind == "opencode" {
        crate::opencode::utility_structured(api, &request.context, &task, stop, "continuation")
            .await?
    } else {
        crate::translation::structured(api, ticket, &request.context, task, stop).await?
    };
    check(stop)?;
    let content = crate::worker::compute(move || text::parse(&result.text))
        .await
        .map_err(|error| match &result.warning {
            Some(warning) => format!("{error}; {warning}"),
            None => error,
        })?;
    Ok(Outcome {
        prompt: content.prompt,
        send: false,
        warning: result.warning,
    })
}
