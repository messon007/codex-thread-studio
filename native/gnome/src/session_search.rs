//! Literal, Unicode-aware session search on bounded Rust workers.
use crate::model::Item;
use regex::RegexBuilder;
use serde::Deserialize;
use std::{collections::HashSet, ops::Range, sync::Arc};

const MAX_QUERY_BYTES: usize = 4096;
const MAX_MATCHES: usize = 100;
const CONTEXT_CHARS: usize = 82;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    All,
    User,
    Assistant,
    Activity,
}
#[derive(Clone, Debug)]
pub struct Occurrence {
    pub turn_id: String,
    pub item_id: String,
    pub kind: Filter,
    pub snippet: String,
    pub highlight: Range<usize>,
    pub source: Option<Range<usize>>,
    pub block: Option<usize>,
    pub table_row: Option<usize>,
    pub paragraph_page: Option<usize>,
    pub paragraph_range: Option<Range<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    pub backend: String,
    pub thread: String,
    pub generation: u64,
}
#[derive(Debug, Clone)]
pub struct Request {
    pub context: Context,
    pub serial: u64,
    pub items: Vec<Arc<Item>>,
    pub query: String,
    pub case_sensitive: bool,
    pub filter: Filter,
    pub backend_kind: String,
    pub directory: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scope {
    #[default]
    Loaded,
    History,
    PartialHistory,
}
#[derive(Debug)]
pub struct Response {
    pub context: Context,
    pub serial: u64,
    pub result: Result<Vec<Occurrence>, String>,
    pub scope: Scope,
}

pub async fn run(
    api: crate::gateway::Api,
    mut input: tokio::sync::watch::Receiver<Option<Request>>,
    events: async_channel::Sender<crate::protocol::Event>,
) {
    let mut ready = false;
    loop {
        if !ready && input.changed().await.is_err() {
            break;
        }
        let request = input.borrow_and_update().clone();
        let Some(request) = request else {
            ready = false;
            continue;
        };
        let context = request.context.clone();
        let serial = request.serial;
        tokio::select! {
            incoming = input.changed() => { if incoming.is_err() { break; } ready = true; },
            response = execute(&api, request) => { ready = false; let (result, scope) = response; if events.send(crate::protocol::Event::SessionSearch(Response { context, serial, result, scope })).await.is_err() { break; } },
        }
    }
}

async fn execute(
    api: &crate::gateway::Api,
    request: Request,
) -> (Result<Vec<Occurrence>, String>, Scope) {
    let (remote, scope) = match request.backend_kind.as_str() {
        "codex" => match crate::conversation_focus::read(
            api,
            &request.context,
            "search",
            Some(("searchTerm", request.query.clone().into())),
        )
        .await
        {
            Ok(value) => (
                crate::worker::compute(move || remote(value))
                    .await
                    .map(Some),
                Scope::History,
            ),
            Err(error) if crate::conversation_focus::unsupported(&error) => {
                (Ok(None), Scope::Loaded)
            }
            Err(error) => (Err(error), Scope::History),
        },
        "opencode" => match crate::opencode_history::search(
            api,
            &request.context.thread,
            &request.directory,
            &request.query,
            request.case_sensitive,
            request.filter,
        )
        .await
        {
            Ok((hits, complete)) => (
                Ok(Some(hits)),
                if complete {
                    Scope::History
                } else {
                    Scope::PartialHistory
                },
            ),
            Err(error) => (Err(error), Scope::PartialHistory),
        },
        _ => (Ok(None), Scope::Loaded),
    };
    let remote = match remote {
        Ok(value) => value,
        Err(error) => return (Err(error), scope),
    };
    (
        crate::worker::compute(move || merge(&request, remote)).await,
        scope,
    )
}
fn merge(request: &Request, remote: Option<Vec<Occurrence>>) -> Result<Vec<Occurrence>, String> {
    let local = find(
        &request.items,
        &request.query,
        request.case_sensitive,
        request.filter,
    )?;
    let Some(remote) = remote else {
        return Ok(local);
    };
    let pattern = RegexBuilder::new(&regex::escape(&request.query))
        .case_insensitive(!request.case_sensitive)
        .size_limit(2 * 1024 * 1024)
        .dfa_size_limit(1024 * 1024)
        .build()
        .map_err(|error| error.to_string())?;
    let finals = final_answers(&request.items);
    let mut output = Vec::new();
    let mut included = HashSet::new();
    for mut hit in remote {
        let Some(found) = pattern.find(&hit.snippet) else {
            continue;
        };
        hit.highlight = found.range();
        let local_match = local
            .iter()
            .find(|local| local.turn_id == hit.turn_id && local.item_id == hit.item_id);
        if let Some(local) = local_match {
            hit = local.clone();
        } else {
            if let Some(loaded) = request
                .items
                .iter()
                .find(|item| item.turn_id == hit.turn_id && item.id == hit.item_id)
            {
                hit.kind = kind(loaded, &finals);
                // A complete visible message is authoritative for role/filter
                // semantics. An excerpt must not hide a real match outside it.
                if !loaded
                    .body
                    .ends_with("… Output truncated in the native view.")
                    && !loaded
                        .body
                        .starts_with("[Excerpt around the selected match]")
                {
                    continue;
                }
            }
            if request.filter != Filter::All && hit.kind != request.filter {
                continue;
            }
        }
        if included.insert((hit.turn_id.clone(), hit.item_id.clone())) {
            output.push(hit);
        }
    }
    for hit in local {
        if output.len() < MAX_MATCHES && included.insert((hit.turn_id.clone(), hit.item_id.clone()))
        {
            output.push(hit);
        }
    }
    output.truncate(MAX_MATCHES);
    Ok(output)
}

pub fn find(
    items: &[Arc<Item>],
    query: &str,
    case_sensitive: bool,
    filter: Filter,
) -> Result<Vec<Occurrence>, String> {
    if query.is_empty() {
        return Ok(Vec::new());
    }
    if query.len() > MAX_QUERY_BYTES {
        return Err("Search text exceeds 4 KiB".into());
    }
    let pattern = RegexBuilder::new(&regex::escape(query))
        .case_insensitive(!case_sensitive)
        .size_limit(2 * 1024 * 1024)
        .dfa_size_limit(1024 * 1024)
        .build()
        .map_err(|error| format!("Cannot search for this text: {error}"))?;
    let final_messages = final_answers(items);
    let mut results = Vec::new();
    for item in items {
        let kind = kind(item, &final_messages);
        if filter != Filter::All && kind != filter {
            continue;
        }
        let Some(found) = pattern.find(&item.body) else {
            continue;
        };
        results.push(occurrence(item, found, kind));
        if results.len() == MAX_MATCHES {
            break;
        }
    }
    Ok(results)
}

fn final_answers(items: &[Arc<Item>]) -> HashSet<(&str, &str)> {
    let mut final_turns = HashSet::new();
    items
        .iter()
        .rev()
        .filter(|item| item.role == "Assistant")
        .filter_map(|item| {
            final_turns
                .insert(item.turn_id.as_str())
                .then_some((item.turn_id.as_str(), item.id.as_str()))
        })
        .collect::<HashSet<_>>()
}
fn kind(item: &Item, final_messages: &HashSet<(&str, &str)>) -> Filter {
    match item.role.as_str() {
        "You" => Filter::User,
        "Assistant" if final_messages.contains(&(item.turn_id.as_str(), item.id.as_str())) => {
            Filter::Assistant
        }
        _ => Filter::Activity,
    }
}

/// The caller may search full source and then clear `source` for unloaded items.
pub(crate) fn occurrence(item: &Item, found: regex::Match<'_>, kind: Filter) -> Occurrence {
    let before = item.body[..found.start()]
        .chars()
        .rev()
        .take(CONTEXT_CHARS)
        .map(char::len_utf8)
        .sum::<usize>();
    let after = item.body[found.end()..]
        .chars()
        .take(CONTEXT_CHARS)
        .map(char::len_utf8)
        .sum::<usize>();
    let start = found.start() - before;
    let end = found.end() + after;
    let mut snippet = String::new();
    if start > 0 {
        snippet.push('…');
    }
    let highlight_start = snippet.len() + found.start() - start;
    snippet.push_str(&item.body[start..end]);
    if end < item.body.len() {
        snippet.push('…');
    }
    let block = item.rich.as_ref().and_then(|content| {
        content.rows.iter().position(|block| {
            block.block.source.start <= found.start() && block.block.source.end > found.start()
        })
    });
    let table_row =
        block.and_then(
            |index| match &item.rich.as_ref().unwrap().rows[index].block.kind {
                studio_markdown_native::BlockKind::Table { rows, .. } => {
                    rows.iter().position(|row| {
                        row.source.start <= found.start() && row.source.end > found.start()
                    })
                }
                _ => None,
            },
        );
    let paragraph_match = block.and_then(|index| {
        let content = match &item.rich.as_ref()?.rows[index].block.kind {
            studio_markdown_native::BlockKind::Paragraph(content)
            | studio_markdown_native::BlockKind::Heading { content, .. } => content,
            _ => return None,
        };
        let layout = content.attributes.as_ref()?;
        layout.project(&item.body, 0, found.range())
    });
    Occurrence {
        turn_id: item.turn_id.clone(),
        item_id: item.id.clone(),
        kind,
        snippet,
        highlight: highlight_start..highlight_start + found.len(),
        source: Some(found.range()),
        block,
        table_row,
        paragraph_page: paragraph_match.as_ref().map(|found| found.page),
        paragraph_range: paragraph_match.map(|found| found.characters),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Remote {
    turn_id: String,
    item_id: String,
    snippet: String,
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    snippet_match_range: Option<RemoteRange>,
}
#[derive(Deserialize)]
struct RemoteRange {
    start: usize,
    end: usize,
}

/// Upstream snippet offsets use UTF-16, while native slicing uses UTF-8 bytes.
pub fn remote(value: serde_json::Value) -> Result<Vec<Occurrence>, String> {
    let entries = value
        .get("data")
        .and_then(serde_json::Value::as_array)
        .ok_or("Invalid session search response")?;
    let mut results = Vec::new();
    for entry in entries.iter().take(MAX_MATCHES) {
        let entry: Remote = serde_json::from_value(entry.clone())
            .map_err(|_| "Invalid session search occurrence")?;
        if entry.turn_id.is_empty()
            || entry.item_id.is_empty()
            || entry.turn_id.len() > 256
            || entry.item_id.len() > 256
        {
            continue;
        }
        let mut snippet = entry.snippet;
        if snippet.len() > 8192 {
            let mut end = 8192;
            while !snippet.is_char_boundary(end) {
                end -= 1;
            }
            snippet.truncate(end);
        }
        let mut start = snippet.len();
        let mut end = snippet.len();
        let mut units = 0;
        if let Some(range) = entry.snippet_match_range {
            for (offset, ch) in snippet.char_indices() {
                if units <= range.start {
                    start = offset;
                }
                if units < range.end {
                    end = offset + ch.len_utf8();
                }
                units += ch.len_utf16();
            }
            if range.start >= units {
                start = snippet.len();
            }
            if range.end == 0 {
                end = 0;
            }
        }
        results.push(Occurrence {
            turn_id: entry.turn_id,
            item_id: entry.item_id,
            kind: match entry.kind.as_str() {
                "user" => Filter::User,
                "activity" => Filter::Activity,
                _ => Filter::Assistant,
            },
            highlight: start..end.max(start),
            snippet,
            source: None,
            block: None,
            table_row: None,
            paragraph_page: None,
            paragraph_range: None,
        });
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(id: &str, turn: &str, kind: &str, text: &str) -> Arc<Item> {
        let mut item = Item::from_value(
            &serde_json::json!({"id":id,"turnId":turn,"type":kind,"text":text,"content":[{"text":text}]}),
        );
        item.turn_id = turn.into();
        Arc::new(item)
    }
    #[test]
    fn literal_search_preserves_spaces_unicode_and_original_offsets() {
        let values = vec![item("a", "turn", "agentMessage", "😀 Äpfel [x]. text ")];
        let found = find(&values, "äPFEL [x]. ", false, Filter::All).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(
            &values[0].body[found[0].source.clone().unwrap()],
            "Äpfel [x]. "
        );
        assert_eq!(&found[0].snippet[found[0].highlight.clone()], "Äpfel [x]. ");
        assert!(find(&values, "text  ", false, Filter::All)
            .unwrap()
            .is_empty());
        assert!(find(&values, "äpfel", true, Filter::All)
            .unwrap()
            .is_empty());
    }
    #[test]
    fn unique_rendered_match_targets_the_worker_prepared_paragraph_page() {
        let source = format!("**begin** {} unique-tail 中文😀", "repeated ".repeat(1400));
        let mut value = Item::from_value(
            &serde_json::json!({"id":"long","turnId":"t","type":"agentMessage","text":source}),
        );
        value.turn_id = "t".into();
        value.rich = crate::chat_content::Content::parse(&value.body).ok();
        let items = vec![Arc::new(value)];
        let found = find(&items, "unique-tail 中文😀", true, Filter::All).unwrap();
        assert!(found[0].paragraph_page.is_some_and(|page| page > 0));
        let ambiguous = find(&items, "repeated", true, Filter::All).unwrap();
        assert_eq!(ambiguous[0].paragraph_page, Some(0));
        assert!(ambiguous[0].paragraph_range.is_some());
    }
    #[test]
    fn hidden_source_match_does_not_select_an_equal_visible_quote() {
        let source = format!(
            "[label](docs/needle.md) {} **needle**",
            "context ".repeat(1500)
        );
        let mut value = Item::from_value(
            &serde_json::json!({"id":"long","turnId":"t","type":"agentMessage","text":source}),
        );
        value.turn_id = "t".into();
        value.rich = crate::chat_content::Content::parse(&value.body).ok();
        let pattern = regex::Regex::new("needle").unwrap();
        let hits = pattern
            .find_iter(&value.body)
            .map(|found| occurrence(&value, found, Filter::Assistant))
            .collect::<Vec<_>>();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].paragraph_range, None);
        assert!(hits[1].paragraph_page.is_some_and(|page| page > 0));
        assert!(hits[1].paragraph_range.is_some());
    }
    #[test]
    fn final_answers_and_activity_are_classified_per_turn() {
        let values = vec![
            item("u", "one", "userMessage", "needle"),
            item("comment", "one", "agentMessage", "needle"),
            item("answer", "one", "agentMessage", "needle"),
            item("next", "two", "agentMessage", "needle"),
        ];
        assert_eq!(
            find(&values, "needle", false, Filter::Assistant)
                .unwrap()
                .iter()
                .map(|entry| entry.item_id.as_str())
                .collect::<Vec<_>>(),
            vec!["answer", "next"]
        );
        assert_eq!(
            find(&values, "needle", false, Filter::Activity).unwrap()[0].item_id,
            "comment"
        );
        assert_eq!(
            find(&values, "needle", false, Filter::User).unwrap()[0].item_id,
            "u"
        );
    }
    #[test]
    fn remote_highlights_convert_utf16_without_splitting_emoji() {
        let found = remote(serde_json::json!({"data":[{"turnId":"turn","itemId":"item","snippet":"😀 中文 match","snippetMatchRange":{"start":3,"end":5}}]})).unwrap();
        assert_eq!(&found[0].snippet[found[0].highlight.clone()], "中文");
    }
    #[test]
    fn remote_matches_outside_visible_excerpts_are_not_lost_or_misclassified() {
        let long = format!("{}distant NEEDLE 中文😀 ", "earlier ".repeat(20000));
        let request = Request {
            context: Context {
                backend: "codex".into(),
                thread: "thread".into(),
                generation: 1,
            },
            serial: 1,
            items: vec![item("a", "turn", "agentMessage", &long)],
            query: "NEEDLE 中文😀 ".into(),
            case_sensitive: true,
            filter: Filter::Assistant,
            backend_kind: "codex".into(),
            directory: String::new(),
        };
        let hits=remote(serde_json::json!({"data":[{"turnId":"turn","itemId":"a","snippet":"distant NEEDLE 中文😀 ","snippetMatchRange":{"start":8,"end":20}}]})).unwrap();
        assert!(
            find(&request.items, &request.query, true, Filter::Assistant)
                .unwrap()
                .is_empty()
        );
        let found = merge(&request, Some(hits.clone())).unwrap();
        assert_eq!(found.len(), 1);
        assert!(found[0].source.is_none());
        let mut request = request;
        request
            .items
            .push(item("b", "turn", "agentMessage", "later final answer"));
        assert!(merge(&request, Some(hits.clone())).unwrap().is_empty());
        request.filter = Filter::Activity;
        assert_eq!(
            merge(&request, Some(hits.clone())).unwrap()[0].kind,
            Filter::Activity
        );
        request.items = vec![item(
            "a",
            "turn",
            "agentMessage",
            "the message has since changed",
        )];
        request.filter = Filter::All;
        assert!(merge(&request, Some(hits)).unwrap().is_empty());
    }
}
