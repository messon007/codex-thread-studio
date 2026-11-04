//! Authoritative, bounded supervision evidence. All reads are independent of the
//! shortened GTK transcript. The owner must still validate its connection epoch,
//! approvals, human-action revision and queue state when accepting a result.
use crate::{
    conversation_pages::{self, Source},
    gateway::Api,
    turn_supervision::{Decision, Snapshot, TurnState, MAX_CONTEXT_UNITS},
};
use serde_json::Value;
use std::{
    collections::HashSet,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

const TEXT_BYTES: usize = MAX_CONTEXT_UNITS * 4;
const MAX_PAGES: usize = 32;
const MAX_PARTS: usize = 8192;

fn identifier(id: &str) -> bool {
    !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)
}
fn check_source(source: &Source) -> Result<(), String> {
    if source.backend_kind != "codex"
        || !identifier(&source.context.backend)
        || !identifier(&source.context.thread)
        || source.directory.is_empty()
        || source.directory.len() > 4096
        || source.directory.contains('\0')
    {
        return Err("Supervision requires an identified Codex session and project".into());
    }
    Ok(())
}
#[derive(Debug, PartialEq, Eq)]
struct Head {
    id: String,
    state: TurnState,
    active: Option<String>,
    error: bool,
}
impl Head {
    fn from_rows<'a>(rows: impl Iterator<Item = &'a Value>) -> Result<Self, String> {
        let mut latest = None;
        let mut active = None;
        let mut seen = HashSet::new();
        for turn in rows.take(4) {
            let id = turn["id"]
                .as_str()
                .filter(|id| identifier(id))
                .ok_or("Invalid supervision source turn identity")?;
            if !seen.insert(id) {
                return Err("Duplicate supervision source turn".into());
            }
            let state = match turn["status"].as_str() {
                Some("inProgress") => TurnState::InProgress,
                Some("completed") => TurnState::Completed,
                Some("failed") => TurnState::Failed,
                Some("interrupted") => TurnState::Interrupted,
                _ => return Err("Unknown supervision source turn state".into()),
            };
            if state == TurnState::InProgress {
                if active.is_some() {
                    return Err("More than one active supervision source turn".into());
                }
                active = Some(id.to_owned());
            }
            if latest.is_none() {
                latest = Some(Self {
                    id: id.into(),
                    state,
                    active: None,
                    error: !turn["error"].is_null(),
                });
            }
        }
        let mut latest = latest.ok_or("No source turn is available for supervision")?;
        if active.as_ref().is_some_and(|id| id != &latest.id) {
            return Err("The latest source turn conflicts with an active older turn".into());
        }
        latest.active = active;
        Ok(latest)
    }
    fn states(value: Value) -> Result<Self, String> {
        let rows = value["data"]
            .as_array()
            .ok_or("Invalid supervision source states")?;
        if rows.len() > 4 {
            return Err("Supervision source states exceeded their page limit".into());
        }
        Self::from_rows(rows.iter())
    }
}
struct Metadata {
    active: bool,
    blocked: bool,
}
fn metadata(value: Value, source: &Source) -> Result<Metadata, String> {
    let thread = &value["thread"];
    if thread["id"].as_str() != Some(&source.context.thread)
        || thread["cwd"].as_str() != Some(&source.directory)
        || thread["ephemeral"] == true
        || thread["archived"] == true
    {
        return Err("Supervision source identity, project or visibility changed".into());
    }
    let state = thread["status"]
        .as_str()
        .or_else(|| thread.pointer("/status/type").and_then(Value::as_str));
    let flags = thread.pointer("/status/activeFlags");
    let blocked = !matches!(state, Some("idle" | "active"))
        || flags.is_some_and(|flags| {
            flags.as_array().is_none_or(|flags| {
                // Both known flags require attention; an unknown future flag
                // is insufficient authority for an unattended continuation.
                !flags.is_empty()
            })
        });
    Ok(Metadata {
        active: state == Some("active"),
        blocked,
    })
}
fn append(output: &mut String, text: &str, separator: bool) -> Result<(), String> {
    let extra = usize::from(separator);
    if output
        .len()
        .saturating_add(text.len())
        .saturating_add(extra)
        > TEXT_BYTES
    {
        return Err("Supervision source text is too large; manual review required".into());
    }
    if separator {
        output.push('\n');
    }
    output.push_str(text);
    Ok(())
}
fn user_text(item: &Value) -> Result<String, String> {
    if let Some(text) = item["text"].as_str() {
        let mut output = String::new();
        append(&mut output, text, false)?;
        return Ok(output);
    }
    let mut output = String::new();
    if let Some(parts) = item["content"].as_array() {
        if parts.len() > MAX_PARTS {
            return Err("Supervision input has too many parts".into());
        }
        for (i, part) in parts.iter().enumerate() {
            // Image/skill attachments are not invented textual user instructions.
            append(&mut output, part["text"].as_str().unwrap_or(""), i != 0)?;
        }
    }
    Ok(output)
}
struct Texts {
    turn: String,
    seen: HashSet<String>,
    users: Vec<String>,
    user_bytes: usize,
    reply: String,
}
impl Texts {
    fn new(turn: String) -> Self {
        Self {
            turn,
            seen: HashSet::new(),
            users: Vec::new(),
            user_bytes: 0,
            reply: String::new(),
        }
    }
    fn item(&mut self, item: &Value) -> Result<(), String> {
        let id = item["id"]
            .as_str()
            .filter(|id| identifier(id))
            .ok_or("Invalid supervision source item identity")?;
        // Never silently combine ambiguous repeated/reordered source pages.
        if !self.seen.insert(id.into()) {
            return Err("Supervision source item pages overlap or changed".into());
        }
        match item["type"].as_str() {
            Some("userMessage") => {
                let text = user_text(item)?;
                if !text.is_empty() {
                    self.user_bytes = self
                        .user_bytes
                        .saturating_add(text.len())
                        .saturating_add(usize::from(!self.users.is_empty()));
                    if self.user_bytes > TEXT_BYTES {
                        return Err("Supervision user input exceeds its budget".into());
                    }
                    self.users.push(text);
                }
            }
            Some("agentMessage") if self.reply.is_empty() => {
                let text = item["text"]
                    .as_str()
                    .ok_or("Invalid supervision assistant text")?;
                append(
                    &mut self.reply,
                    crate::session_map_text::visible(text),
                    false,
                )?;
            }
            Some(_) => {}
            None => return Err("Invalid supervision source item kind".into()),
        }
        Ok(())
    }
    fn page(&mut self, value: Value) -> Result<Option<String>, String> {
        let rows = value["data"]
            .as_array()
            .ok_or("Invalid supervision source item page")?;
        if rows.len() > conversation_pages::PAGE_ITEMS {
            return Err("Supervision source item page exceeded its limit".into());
        }
        let next = conversation_pages::cursor(value.get("nextCursor"))?;
        if rows.is_empty() && next.is_some() {
            return Err("Supervision source returned an empty cursor page".into());
        }
        for entry in rows {
            let turn = entry["turnId"]
                .as_str()
                .filter(|id| identifier(id))
                .ok_or("Invalid supervision item turn")?;
            if turn != self.turn {
                if self.seen.is_empty() {
                    return Err("Source turn changed during supervision read".into());
                }
                return Ok(None);
            }
            self.item(&entry["item"])?;
        }
        Ok(next)
    }
    fn finish(self, source: &Source, head: Head, before: Metadata, after: Metadata) -> Snapshot {
        let original = self.users.into_iter().rev().collect::<Vec<_>>().join("\n");
        Snapshot {
            context: source.context.clone(),
            latest_turn: head.id.clone(),
            state: head.state,
            active_turn: head.active.or_else(|| after.active.then_some(head.id)),
            blocked: before.blocked || after.blocked,
            error: head.error,
            original,
            reply: self.reply,
        }
    }
}
trait History {
    async fn read(&mut self, operation: &str, cursor: Option<String>) -> Result<Value, String>;
}
struct Reader<'a> {
    api: &'a Api,
    source: &'a Source,
}
impl History for Reader<'_> {
    async fn read(&mut self, operation: &str, cursor: Option<String>) -> Result<Value, String> {
        crate::conversation_focus::read(
            self.api,
            &self.source.context,
            operation,
            cursor.map(|c| ("cursor", Value::String(c))),
        )
        .await
    }
}
async fn full(reader: &mut impl History, source: &Source) -> Result<(Head, Texts), String> {
    let (value, legacy) = match reader.read("turns", None).await {
        Ok(value) => (value, false),
        Err(error) if crate::conversation_focus::unsupported(&error) => {
            (reader.read("read", None).await?, true)
        }
        Err(error) => return Err(error),
    };
    let context = source.context.clone();
    crate::worker::compute(move || {
        if legacy && value.pointer("/thread/id").and_then(Value::as_str) != Some(&context.thread) {
            return Err("Supervision history belongs to another session".into());
        }
        let rows = if legacy {
            value.pointer("/thread/turns")
        } else {
            value.get("data")
        }
        .and_then(Value::as_array)
        .ok_or("Invalid supervision source turns")?;
        if !legacy && rows.len() > 4 {
            return Err("Supervision source turn page exceeded its limit".into());
        }
        let head = if legacy {
            Head::from_rows(rows.iter().rev())?
        } else {
            Head::from_rows(rows.iter())?
        };
        let latest = if legacy { rows.last() } else { rows.first() }.unwrap();
        let items = latest["items"]
            .as_array()
            .ok_or("Supervision source has no full item array")?;
        if items.len() > MAX_PAGES * conversation_pages::PAGE_ITEMS {
            return Err("Supervision source turn has too many items".into());
        }
        let mut texts = Texts::new(head.id.clone());
        for item in items.iter().rev() {
            texts.item(item)?;
        }
        Ok((head, texts))
    })
    .await
}
async fn evidence(reader: &mut impl History, source: &Source) -> Result<Snapshot, String> {
    let value = reader.read("summary", None).await?;
    let owned = source.clone();
    let before = crate::worker::compute(move || metadata(value, &owned)).await?;
    let initial = match reader.read("states", None).await {
        Ok(value) => Some(crate::worker::compute(move || Head::states(value)).await?),
        Err(error) if crate::conversation_focus::unsupported(&error) => None,
        Err(error) => return Err(error),
    };
    let (head, texts) = if let Some(head) = initial {
        let mut texts = Texts::new(head.id.clone());
        let mut cursor = None;
        let mut cursors = HashSet::new();
        let mut fallback = None;
        for page in 0..MAX_PAGES {
            let value = match reader.read("items", cursor).await {
                Ok(value) => value,
                Err(error) if page == 0 && crate::conversation_focus::unsupported(&error) => {
                    fallback = Some(full(reader, source).await?);
                    break;
                }
                Err(error) => return Err(error),
            };
            let (updated, next) = crate::worker::compute(move || {
                let next = texts.page(value)?;
                Ok((texts, next))
            })
            .await?;
            texts = updated;
            let Some(next) = next else {
                break;
            };
            if page + 1 == MAX_PAGES || !cursors.insert(next.clone()) {
                return Err(
                    "Supervision source paging exceeded its budget or repeated a cursor".into(),
                );
            }
            cursor = Some(next);
        }
        if let Some((full_head, full_texts)) = fallback {
            if full_head != head {
                return Err("Source turn changed during supervision read".into());
            }
            texts = full_texts;
        }
        let value = reader.read("states", None).await?;
        let after = crate::worker::compute(move || Head::states(value)).await?;
        if after != head {
            return Err("Source turn changed during supervision read".into());
        }
        (head, texts)
    } else {
        full(reader, source).await?
    };
    let value = reader.read("summary", None).await?;
    let owned = source.clone();
    let after = crate::worker::compute(move || metadata(value, &owned)).await?;
    let owned = source.clone();
    crate::worker::compute(move || Ok(texts.finish(&owned, head, before, after))).await
}
pub async fn read(api: &Api, source: &Source, cancel: &AtomicBool) -> Result<Snapshot, String> {
    check_source(source)?;
    let mut reader = Reader { api, source };
    let work = evidence(&mut reader, source);
    tokio::pin!(work);
    let deadline = tokio::time::sleep(Duration::from_secs(45));
    tokio::pin!(deadline);
    loop {
        if cancel.load(Ordering::Acquire) {
            return Err("Supervision source read cancelled".into());
        }
        tokio::select! {
            result = &mut work => return result,
            _ = &mut deadline => return Err("Supervision source read exceeded its time budget".into()),
            _ = tokio::time::sleep(Duration::from_millis(50)) => {}
        }
    }
}

pub async fn evaluate(
    api: &Api,
    ticket: u64,
    context: &crate::translation::Context,
    effort: String,
    input: String,
    cancel: &AtomicBool,
) -> Result<(Decision, Option<String>), String> {
    if input.len() > TEXT_BYTES || input.encode_utf16().count() > MAX_CONTEXT_UNITS {
        return Err("Supervision context is too large; manual review required".into());
    }
    let result = crate::translation::structured(
        api,
        ticket,
        context,
        crate::translation::StructuredTask {
            instructions: crate::turn_supervision::INSTRUCTIONS.into(),
            input,
            schema: crate::turn_supervision::schema(),
            model: context.model.clone(),
            effort,
            service_tier: String::new(),
            max_result: 512 * 1024,
            read_completed_thread: true,
        },
        cancel,
    )
    .await?;
    let decision = crate::worker::compute(move || Decision::parse(&result.text)).await?;
    Ok((decision, result.warning))
}

#[cfg(test)]
#[path = "turn_supervision_io_tests.rs"]
mod tests;
