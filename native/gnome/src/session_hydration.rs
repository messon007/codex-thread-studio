//! Initial history is a bounded private read after the live subscription opens.
use crate::{
    conversation_pages::{self, Mode, Position},
    gateway::Api,
    history_state::Lifecycle,
    protocol::Prepared,
    session_search::Context,
};
use serde_json::{json, Value};
use std::sync::LazyLock;
use tokio::sync::Semaphore;

static READS: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(2));

#[derive(Clone)]
pub struct Selection {
    pub thread: String,
    pub method: String,
    pub original: Value,
    pub retried: bool,
}
impl Selection {
    /// Only the ordinary session selection. Command/environment resumes and
    /// Router metadata reads retain their original RPC semantics.
    pub fn new(method: &str, params: &Value) -> Option<Self> {
        let object = params.as_object()?;
        let thread = object.get("threadId")?.as_str()?;
        if thread.is_empty() || thread.len() > 256 || thread.chars().any(char::is_control) {
            return None;
        }
        let simple = (method == "thread/resume" && object.len() == 1)
            || (method == "thread/read"
                && object.len() == 2
                && object.get("includeTurns") == Some(&Value::Bool(true)));
        simple.then(|| Self {
            thread: thread.into(),
            method: method.into(),
            original: params.clone(),
            retried: false,
        })
    }
    pub fn metadata(&self) -> Value {
        if self.method == "thread/resume" {
            json!({"threadId":self.thread,"excludeTurns":true})
        } else {
            json!({"threadId":self.thread,"includeTurns":false})
        }
    }
    pub fn retry(&mut self, error: &Value) -> bool {
        let message = error["message"].as_str().unwrap_or("").to_ascii_lowercase();
        if self.retried
            || !(crate::conversation_focus::unsupported(&message)
                || message.contains("unknown field")
                || message.contains("invalid params")
                || message.contains("invalid parameter")
                || error["code"] == -32602)
        {
            return false;
        }
        self.retried = true;
        true
    }
}

async fn read(
    api: &Api,
    context: &Context,
    mode: Mode,
    cursor: Option<String>,
) -> Result<conversation_pages::Page, String> {
    let operation = match mode {
        Mode::Items => "items",
        Mode::Paged => "turns",
        Mode::Legacy => "read",
    };
    let value = crate::conversation_focus::read(
        api,
        context,
        operation,
        cursor.clone().map(|c| ("cursor", c.into())),
    )
    .await?;
    if mode == Mode::Legacy
        && value.pointer("/thread/id").and_then(Value::as_str) != Some(&context.thread)
    {
        return Err("Backend history belongs to a different session".into());
    }
    crate::worker::compute(move || {
        let position = Position {
            cursor,
            ..Default::default()
        };
        if mode == Mode::Items {
            conversation_pages::codex_items(value, &position)
        } else {
            conversation_pages::codex(value, &position, mode)
        }
    })
    .await
}
async fn states(api: &Api, context: &Context) -> Result<Lifecycle, String> {
    let value = crate::conversation_focus::read(api, context, "states", None).await?;
    crate::worker::compute(move || {
        let turns = value["data"]
            .as_array()
            .ok_or("Backend returned no turn state page")?;
        if turns.len() > 4 {
            return Err("Backend exceeded the requested turn state page size".into());
        }
        crate::history_state::lifecycle(turns)
    })
    .await
}
pub async fn load(
    api: &Api,
    context: Context,
    mut payload: Value,
    mut prepared: Prepared,
) -> Result<(Value, Prepared), String> {
    if payload.pointer("/result/thread/id").and_then(Value::as_str) != Some(&context.thread) {
        return Err("Backend history belongs to a different session".into());
    }
    // A legacy server may ignore excludeTurns and already return full history.
    if prepared
        .history
        .as_ref()
        .is_some_and(|items| !items.is_empty())
    {
        return Ok((payload, prepared));
    }
    let _permit = READS
        .acquire()
        .await
        .map_err(|_| "History worker stopped")?;
    let item_cursor = conversation_pages::cursor(payload.pointer("/result/itemsBackwardsCursor"))?;
    let turn_cursor = conversation_pages::cursor(payload.pointer("/result/turnsBackwardsCursor"))?;
    let mut mode = Mode::Items;
    let page = loop {
        let cursor = match mode {
            Mode::Items => item_cursor.clone(),
            Mode::Paged => turn_cursor.clone(),
            Mode::Legacy => None,
        };
        match read(api, &context, mode, cursor).await {
            Ok(page) => break page,
            Err(error)
                if mode != Mode::Legacy && crate::conversation_focus::unsupported(&error) =>
            {
                mode = if mode == Mode::Items {
                    Mode::Paged
                } else {
                    Mode::Legacy
                };
            }
            Err(error) => return Err(error),
        }
    };
    let lifecycle = if let Some(state) = page.lifecycle {
        state
    } else {
        match states(api, &context).await {
            Ok(state) => state,
            // Older servers without summary view still need authoritative turn
            // identity; an idle-looking catalog row is not sufficient.
            Err(error)
                if crate::conversation_focus::unsupported(&error)
                    || error.to_ascii_lowercase().contains("invalid params")
                    || (error.to_ascii_lowercase().contains("unknown variant")
                        && error.to_ascii_lowercase().contains("summary")) =>
            {
                let state_page = match read(api, &context, Mode::Paged, None).await {
                    Ok(page) => page,
                    Err(error) if crate::conversation_focus::unsupported(&error) => {
                        read(api, &context, Mode::Legacy, None).await?
                    }
                    Err(error) => return Err(error),
                };
                state_page.lifecycle.ok_or("Missing conversation state")?
            }
            Err(error) => return Err(error),
        }
    };
    prepared.history = Some(page.items);
    prepared.active_turn = lifecycle.active_turn;
    prepared.active_shell = lifecycle.active_shell;
    // May contain a redundant large initialTurnsPage on experimental servers.
    if let Some(result) = payload["result"].as_object_mut() {
        result.remove("initialTurnsPage");
    }
    Ok((payload, prepared))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_rewrite_never_changes_router_reads_or_configured_resumes() {
        let mut selection = Selection::new("thread/resume", &json!({"threadId":"t"})).unwrap();
        assert_eq!(
            selection.metadata(),
            json!({"threadId":"t","excludeTurns":true})
        );
        assert!(selection.retry(&json!({"message":"unknown field excludeTurns"})));
        assert!(!selection.retry(&json!({"message":"unknown field excludeTurns"})));
        assert!(Selection::new("thread/resume", &json!({"threadId":"t","model":"x"})).is_none());
        assert!(
            Selection::new("thread/read", &json!({"threadId":"t","includeTurns":false})).is_none()
        );
        let selection =
            Selection::new("thread/read", &json!({"threadId":"t","includeTurns":true})).unwrap();
        assert_eq!(selection.metadata()["includeTurns"], false);
    }
}
