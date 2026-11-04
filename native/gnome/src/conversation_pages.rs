//! Bounded, read-only transcript windows, independent of live delivery state.
use crate::{gateway::Api, model::Item, session_search::Context};
use serde_json::Value;
use std::{collections::HashSet, sync::Arc};

pub const PAGE_ITEMS: usize = 120;
const OPENCODE_MESSAGES: usize = 80;
const TRAIL_BYTES: usize = 4 * 1024 * 1024;
const TRAIL_PAGES: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Anchor {
    pub turn: String,
    pub item: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Position {
    pub cursor: Option<String>,
    /// Strict for slices within a provider page; optional at overlapping seams.
    pub before: Option<Anchor>,
    pub required: bool,
}
impl Position {
    fn weight(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.cursor.as_ref().map_or(0, String::capacity)
            + self
                .before
                .as_ref()
                .map_or(0, |a| a.turn.capacity() + a.item.capacity())
    }
}
#[derive(Default)]
pub struct Trail {
    pages: Vec<Position>,
    bytes: usize,
}
impl Trail {
    pub fn len(&self) -> usize {
        self.pages.len()
    }
    pub fn last(&self) -> Option<&Position> {
        self.pages.last()
    }
    pub fn can_push(&self, position: &Position) -> Result<(), String> {
        if self.pages.len() >= TRAIL_PAGES
            || self.bytes.saturating_add(position.weight()) > TRAIL_BYTES
        {
            return Err("History navigation reached its memory limit. Return to Live before browsing again.".into());
        }
        Ok(())
    }
    pub fn contains(&self, position: &Position) -> bool {
        self.pages.contains(position)
            || (!position.required
                && position.cursor.is_some()
                && self.pages.iter().any(|p| p.cursor == position.cursor))
    }
    pub fn push(&mut self, position: Position) -> Result<(), String> {
        self.can_push(&position)?;
        self.bytes += position.weight();
        self.pages.push(position);
        Ok(())
    }
    pub fn pop(&mut self) {
        if let Some(p) = self.pages.pop() {
            self.bytes = self.bytes.saturating_sub(p.weight());
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Items,
    Paged,
    Legacy,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub context: Context,
    pub backend_kind: String,
    pub directory: String,
}
#[derive(Clone, Debug)]
pub struct Request {
    pub source: Source,
    pub serial: u64,
    pub position: Position,
    pub mode: Mode,
}
#[derive(Debug)]
pub struct Page {
    pub items: Vec<Arc<Item>>,
    pub older: Option<Position>,
    pub mode: Mode,
    pub partial: bool,
    pub excerpts: bool,
    pub lifecycle: Option<crate::history_state::Lifecycle>,
}
#[derive(Debug)]
pub struct Response {
    pub source: Source,
    pub serial: u64,
    pub result: Result<Page, String>,
}
fn identifier(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
pub(crate) fn cursor(value: Option<&Value>) -> Result<Option<String>, String> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.is_empty() => Ok(None),
        Some(Value::String(s)) if s.len() <= 4096 && !s.chars().any(char::is_control) => {
            Ok(Some(s.clone()))
        }
        _ => Err("Backend returned an invalid conversation cursor".into()),
    }
}
pub(crate) fn codex_items(mut value: Value, position: &Position) -> Result<Page, String> {
    let next = cursor(value.get("nextCursor"))?;
    let Value::Array(entries) = value
        .get_mut("data")
        .map(Value::take)
        .ok_or("Backend returned no history item page")?
    else {
        return Err("Backend returned invalid history items".into());
    };
    if entries.len() > PAGE_ITEMS || (entries.is_empty() && next.is_some()) {
        return Err("Backend returned an invalid item page size or cursor".into());
    }
    let mut raw = Vec::new();
    for mut entry in entries.into_iter().rev() {
        let turn = entry["turnId"]
            .as_str()
            .filter(|s| identifier(s))
            .ok_or("Backend returned an invalid item turn identity")?;
        let turn: Arc<str> = turn.into();
        let item = entry
            .get_mut("item")
            .map(Value::take)
            .filter(Value::is_object)
            .ok_or("Backend returned an invalid history item entry")?;
        raw.push((turn, item));
    }
    project(raw, position, next, Mode::Items, false)
}
pub(crate) fn codex(value: Value, position: &Position, mode: Mode) -> Result<Page, String> {
    let mut value = value;
    let next = if mode == Mode::Legacy {
        None
    } else {
        cursor(value.get("nextCursor"))?
    };
    let mut turns = if mode == Mode::Legacy {
        value.pointer_mut("/thread/turns").map(Value::take)
    } else {
        value.get_mut("data").map(Value::take)
    }
    .ok_or("Backend returned no conversation turns")?;
    let turns = turns
        .as_array_mut()
        .ok_or("Backend returned invalid conversation turns")?;
    if mode == Mode::Paged && turns.len() > 4 {
        return Err("Backend exceeded the requested turn page size".into());
    }
    if turns.is_empty() && next.is_some() {
        return Err("Backend returned a cursor for an empty turn page".into());
    }
    if mode == Mode::Paged {
        turns.reverse();
    }
    let lifecycle = crate::history_state::lifecycle(turns)?;
    let mut raw = Vec::new();
    for turn in turns {
        let turn_id = turn["id"]
            .as_str()
            .filter(|s| identifier(s))
            .ok_or("Backend returned an invalid turn identity")?;
        let turn_id: Arc<str> = turn_id.into();
        if let Some(items) = turn.get_mut("items").map(Value::take) {
            let Value::Array(items) = items else {
                return Err("Backend returned invalid turn items".into());
            };
            raw.extend(items.into_iter().map(|value| (turn_id.clone(), value)));
        }
    }
    let mut page = project(raw, position, next, mode, false)?;
    page.lifecycle = Some(lifecycle);
    Ok(page)
}
fn opencode(value: Value, position: &Position, next: Option<String>) -> Result<Page, String> {
    let rows = value
        .as_array()
        .ok_or("OpenCode returned an invalid message page")?;
    if rows.len() > OPENCODE_MESSAGES {
        return Err("OpenCode exceeded the requested message page size".into());
    }
    if rows.is_empty() && next.is_some() {
        return Err("OpenCode returned a cursor for an empty message page".into());
    }
    let partial = rows.len() == OPENCODE_MESSAGES && next.is_none();
    let raw = crate::opencode::message_items(&value)?
        .into_iter()
        .map(|item| (Arc::from(crate::model::text(&item, "turnId")), item))
        .collect();
    project(raw, position, next, Mode::Paged, partial)
}
fn project(
    raw: Vec<(Arc<str>, Value)>,
    position: &Position,
    next: Option<String>,
    mode: Mode,
    partial: bool,
) -> Result<Page, String> {
    if next.is_some() && next == position.cursor {
        return Err("Backend repeated its conversation cursor".into());
    }
    // Deduplicate overlapping parts within a page without changing source order.
    let mut ids = HashSet::new();
    let mut values = Vec::new();
    for (turn, value) in raw {
        let id = crate::model::text(&value, "id");
        if !identifier(&turn) || !identifier(id) {
            return Err("Backend returned an invalid history item identity".into());
        }
        if ids.insert((turn.clone(), id.to_owned())) {
            values.push((turn, value));
        }
    }
    let end = if let Some(anchor) = &position.before {
        match values.iter().position(|(turn, v)| turn.as_ref() == anchor.turn && v["id"].as_str() == Some(&anchor.item)) {
            Some(index) => index,
            None if position.required => return Err("History changed around this page. The current view is retained; return to Live and reopen history.".into()),
            None => values.len(),
        }
    } else {
        values.len()
    };
    let mut items = Vec::new();
    let mut bytes = 0usize;
    let mut start = end;
    let mut excerpts = false;
    for (turn, value) in values[..end].iter().rev().take(PAGE_ITEMS) {
        let mut item = Item::from_value(value);
        item.turn_id = turn.to_string();
        if item.role == "Assistant" {
            item.rich = crate::chat_content::Content::parse(&item.body).ok();
            if item.rich.is_none() {
                item.markup = Some(crate::markdown::markup(&item.body));
            }
        }
        if item.weight() > crate::model::MAX_VISIBLE_BYTES {
            item.rich = None;
            item.markup = None;
        }
        let weight = item.weight();
        if bytes.saturating_add(weight) > crate::model::MAX_VISIBLE_BYTES {
            if items.is_empty() {
                return Err("This history item exceeds the native presentation limit".into());
            }
            break;
        }
        bytes += weight;
        excerpts |= item.truncated;
        items.push(Arc::new(item));
        start -= 1;
    }
    items.reverse();
    let older = if start > 0 {
        let first = items.first().ok_or("History page has no anchor")?;
        Some(Position {
            cursor: position.cursor.clone(),
            before: Some(Anchor {
                turn: first.turn_id.clone(),
                item: first.id.clone(),
            }),
            required: true,
        })
    } else {
        next.map(|cursor| Position {
            cursor: Some(cursor),
            before: values.first().map(|(turn, item)| Anchor {
                turn: turn.to_string(),
                item: crate::model::text(item, "id").into(),
            }),
            required: false,
        })
    };
    Ok(Page {
        items,
        older,
        mode,
        partial,
        excerpts,
        lifecycle: None,
    })
}
pub async fn load(api: &Api, request: Request) -> Result<Page, String> {
    if request.source.backend_kind == "opencode" {
        let (value, next) = api
            .history_page(&crate::opencode::message_page_path(
                &request.source.context.thread,
                &request.source.directory,
                request.position.cursor.as_deref(),
                OPENCODE_MESSAGES,
            ))
            .await?;
        return crate::worker::compute(move || opencode(value, &request.position, next)).await;
    }
    if request.source.backend_kind != "codex" {
        return Err("Unsupported conversation backend".into());
    }
    let mut mode = request.mode;
    let value = loop {
        let operation = match mode {
            Mode::Items => "items",
            Mode::Paged => "turns",
            Mode::Legacy => "read",
        };
        let result = crate::conversation_focus::read(
            api,
            &request.source.context,
            operation,
            request
                .position
                .cursor
                .clone()
                .map(|c| ("cursor", Value::String(c))),
        )
        .await;
        match result {
            Ok(value) => break value,
            Err(error)
                if mode != Mode::Legacy
                    && request.position == Position::default()
                    && crate::conversation_focus::unsupported(&error) =>
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
    if mode == Mode::Legacy
        && value.pointer("/thread/id").and_then(Value::as_str)
            != Some(&request.source.context.thread)
    {
        return Err("Backend history belongs to a different session".into());
    }
    crate::worker::compute(move || {
        if mode == Mode::Items {
            codex_items(value, &request.position)
        } else {
            codex(value, &request.position, mode)
        }
    })
    .await
}
pub async fn run(
    api: Api,
    mut input: tokio::sync::watch::Receiver<Option<Request>>,
    events: async_channel::Sender<crate::protocol::Event>,
) {
    let mut ready = false;
    loop {
        if !ready && input.changed().await.is_err() {
            break;
        }
        let Some(request) = input.borrow_and_update().clone() else {
            ready = false;
            continue;
        };
        let source = request.source.clone();
        let serial = request.serial;
        tokio::select! {
            incoming = input.changed() => { if incoming.is_err() { break; } ready = true; },
            result = load(&api, request) => {
                ready = false;
                if events.send(crate::protocol::Event::ConversationPage(Response { source, serial, result })).await.is_err() { break; }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn turns(count: usize) -> Value {
        json!({"data":[{"id":"t","status":"completed","items":(0..count).map(|i| json!({"id":format!("i{i}"),"type":"agentMessage","text":format!("body {i}")})).collect::<Vec<_>>()}]})
    }
    #[test]
    fn huge_single_turn_has_no_item_gaps_and_newer_positions_round_trip() {
        let mut position = Position::default();
        let mut all = Vec::new();
        let mut trail = Trail::default();
        loop {
            let page = codex(turns(450), &position, Mode::Paged).unwrap();
            assert!(page.items.len() <= PAGE_ITEMS);
            assert!(page
                .items
                .iter()
                .all(|i| i.turn_id == "t" && i.rich.is_some()));
            all.splice(0..0, page.items.iter().map(|i| i.id.clone()));
            let Some(older) = page.older else {
                break;
            };
            trail.push(position).unwrap();
            position = older;
        }
        assert_eq!(all, (0..450).map(|i| format!("i{i}")).collect::<Vec<_>>());
        assert_eq!(trail.len(), 3);
        let previous = codex(turns(450), trail.last().unwrap(), Mode::Paged).unwrap();
        assert_eq!(previous.items.first().unwrap().id, "i90");
        trail.pop();
        assert_eq!(trail.len(), 2);
    }
    #[test]
    fn cursor_seams_overlap_without_duplicates_and_mutation_is_explicit() {
        let first = codex(json!({"data":[{"id":"new","status":"completed","items":[{"id":"b","type":"agentMessage","text":"B"}]}],"nextCursor":"opaque/+?&=中文"}), &Position::default(), Mode::Paged).unwrap();
        let before = first.older.unwrap();
        let overlap = codex(json!({"data":[{"id":"new","status":"completed","items":[{"id":"b","type":"agentMessage","text":"B"}]},{"id":"old","status":"completed","items":[{"id":"a","type":"agentMessage","text":"A"}]}]}), &before, Mode::Paged).unwrap();
        assert_eq!(overlap.items.len(), 1);
        assert_eq!(overlap.items[0].id, "a");
        assert!(codex(
            json!({"data":[],"nextCursor":"bad"}),
            &Position::default(),
            Mode::Paged
        )
        .is_err());
        assert!(codex(
            json!({"data":[],"nextCursor":123}),
            &Position::default(),
            Mode::Paged
        )
        .is_err());
        assert!(codex(
            turns(1),
            &Position {
                before: Some(Anchor {
                    turn: "t".into(),
                    item: "removed".into()
                }),
                required: true,
                ..Default::default()
            },
            Mode::Paged
        )
        .unwrap_err()
        .contains("retained"));
    }
    #[test]
    fn legacy_order_and_opencode_part_paging_preserve_source_anchors() {
        let mut raw = turns(250);
        let mut legacy = json!({"thread":{"turns":[]}});
        legacy["thread"]["turns"] = raw["data"].take();
        let page = codex(legacy, &Position::default(), Mode::Legacy).unwrap();
        assert_eq!(page.mode, Mode::Legacy);
        assert_eq!(page.items[0].id, "i130");
        assert!(page.older.is_some());
        let raw = json!([{"info":{"id":"a","role":"assistant","parentID":"user"},"parts":(0..250).map(|i|json!({"id":format!("p{i}"),"type":"text","text":format!("中文😀 {i}")})).collect::<Vec<_>>()}]);
        let first = opencode(raw.clone(), &Position::default(), None).unwrap();
        assert_eq!(first.items.len(), PAGE_ITEMS);
        assert!(first.items.iter().all(|i| i.turn_id == "user"));
        let second = opencode(raw, &first.older.unwrap(), None).unwrap();
        assert_eq!(second.items.last().unwrap().id, "p129");
    }
    #[test]
    fn navigation_metadata_is_bounded_without_discarding_a_pending_position() {
        let mut trail = Trail::default();
        for i in 0..TRAIL_PAGES {
            trail
                .push(Position {
                    cursor: Some(i.to_string()),
                    ..Default::default()
                })
                .unwrap();
        }
        let last = trail.last().unwrap().clone();
        assert!(trail.push(Position::default()).is_err());
        assert_eq!(trail.last(), Some(&last));
    }
    #[test]
    fn malformed_sources_cycles_and_incomplete_legacy_pages_are_explicit() {
        let mut value = turns(1);
        value["nextCursor"] = json!("loop");
        assert!(codex(
            value,
            &Position {
                cursor: Some("loop".into()),
                ..Default::default()
            },
            Mode::Paged
        )
        .unwrap_err()
        .contains("repeated"));
        let mut value = turns(1);
        value["data"][0]["items"][0]["id"] = json!("bad\0id");
        assert!(codex(value, &Position::default(), Mode::Paged).is_err());
        let messages=Value::Array((0..OPENCODE_MESSAGES).map(|i|json!({"info":{"id":format!("m{i}"),"role":"assistant","parentID":"u"},"parts":[{"id":format!("p{i}"),"type":"text","text":"body"}]})).collect());
        assert!(
            opencode(messages, &Position::default(), None)
                .unwrap()
                .partial
        );
        let mut value = turns(1);
        value["data"][0]["items"][0]["text"] = json!("中文😀".repeat(20_000));
        let page = codex(value, &Position::default(), Mode::Paged).unwrap();
        assert!(page.excerpts);
        assert!(page.items[0].weight() <= crate::model::MAX_VISIBLE_BYTES);
        let mut trail = Trail::default();
        trail
            .push(Position {
                cursor: Some("a".into()),
                ..Default::default()
            })
            .unwrap();
        assert!(trail.contains(&Position {
            cursor: Some("a".into()),
            before: Some(Anchor {
                turn: "u".into(),
                item: "different-seam".into()
            }),
            required: false
        }));
    }
    #[test]
    fn preferred_item_pages_keep_cross_turn_order_and_validate_entries() {
        let raw = json!({"data":[{"turnId":"new","item":{"id":"b","type":"agentMessage","text":"newest"}},{"turnId":"old","item":{"id":"a","type":"userMessage","content":[{"type":"text","text":"oldest 中文😀"}]}}],"nextCursor":"items:opaque/+?"});
        let page = codex_items(raw, &Position::default()).unwrap();
        assert_eq!(page.mode, Mode::Items);
        assert_eq!(page.items[0].id, "a");
        assert_eq!(page.items[0].turn_id, "old");
        assert_eq!(page.items[1].turn_id, "new");
        assert_eq!(
            page.older.unwrap().cursor.as_deref(),
            Some("items:opaque/+?")
        );
        for value in [
            json!({"data":[],"nextCursor":"bad"}),
            json!({"data":[{"turnId":"t","item":null}]}),
            json!({"data":[{"item":{"id":"x"}}]}),
        ] {
            assert!(codex_items(value, &Position::default()).is_err());
        }
    }
}
