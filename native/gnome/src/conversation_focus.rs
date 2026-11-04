//! Bounded read-only history retrieval for search/favorite source navigation.
use crate::{
    gateway::Api,
    model::Item,
    session_search::{Context, Occurrence},
};
use serde_json::{json, Value};
use std::{collections::HashSet, sync::Arc};
#[derive(Debug, Clone)]
pub struct Request {
    pub backend_kind: String,
    pub directory: String,
    pub context: Context,
    pub serial: u64,
    pub hit: Occurrence,
}
#[derive(Debug)]
pub struct Window {
    pub items: Vec<Arc<Item>>,
    pub target: String,
    pub excerpt: bool,
}
#[derive(Debug)]
pub struct Response {
    pub context: Context,
    pub serial: u64,
    pub result: Result<Window, String>,
}
pub async fn read(
    api: &Api,
    context: &Context,
    operation: &str,
    extra: Option<(&str, Value)>,
) -> Result<Value, String> {
    let mut body =
        json!({"backend":context.backend,"threadId":context.thread,"operation":operation});
    if let Some((name, value)) = extra {
        body[name] = value;
    }
    api.json(
        reqwest::Method::POST,
        "/studio/native/conversation/read",
        Some(&body),
    )
    .await
}
pub fn unsupported(error: &str) -> bool {
    let text = error.to_ascii_lowercase();
    [
        "method not found",
        "unknown method",
        "unsupported",
        "not supported",
        "does not support",
    ]
    .iter()
    .any(|word| text.contains(word))
}
pub async fn load(api: &Api, request: Request) -> Result<Window, String> {
    if request.backend_kind == "opencode" {
        return crate::opencode_history::focus(api, request).await;
    }
    if request.backend_kind != "codex" {
        return Err("Unsupported conversation backend".into());
    }
    let mut cursor = None;
    let mut visited = HashSet::new();
    loop {
        let value = match read(
            api,
            &request.context,
            "turns",
            cursor
                .clone()
                .map(|cursor| ("cursor", Value::String(cursor))),
        )
        .await
        {
            Ok(value) => value,
            Err(error) if unsupported(&error) => {
                let mut value = read(api, &request.context, "read", None).await?;
                let turns = value
                    .pointer_mut("/thread/turns")
                    .map(Value::take)
                    .ok_or("Backend returned no conversation turns")?;
                let hit = request.hit.clone();
                return crate::worker::compute(move || {
                    project(turns, &hit, false)?
                        .ok_or_else(|| "Search target is no longer in this conversation".into())
                })
                .await;
            }
            Err(error) => return Err(error),
        };
        let next = value["nextCursor"]
            .as_str()
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let hit = request.hit.clone();
        let window = crate::worker::compute(move || {
            let mut value = value;
            project(
                value
                    .get_mut("data")
                    .map(Value::take)
                    .ok_or("Backend returned no turn page")?,
                &hit,
                true,
            )
        })
        .await?;
        if let Some(window) = window {
            return Ok(window);
        }
        let Some(next) = next else {
            return Err("Search target is no longer in this conversation".into());
        };
        if !visited.insert(next.clone()) {
            return Err("Backend repeated its conversation cursor".into());
        }
        cursor = Some(next);
    }
}
fn project(turns: Value, hit: &Occurrence, descending: bool) -> Result<Option<Window>, String> {
    let Value::Array(mut turns) = turns else {
        return Err("Invalid conversation turn page".into());
    };
    if descending {
        turns.reverse();
    }
    let mut raw = Vec::new();
    for mut turn in turns {
        let id = turn["id"].as_str().unwrap_or("").to_owned();
        if let Some(Value::Array(items)) = turn.get_mut("items").map(Value::take) {
            raw.extend(items.into_iter().map(|item| (id.clone(), item)));
        }
    }
    let Some(index) = raw.iter().position(|(turn, item)| {
        *turn == hit.turn_id && item["id"].as_str() == Some(hit.item_id.as_str())
    }) else {
        return Ok(None);
    };
    let start = index.saturating_sub(40);
    let end = (index + 41).min(raw.len());
    let mut items = Vec::new();
    let mut excerpt = false;
    let needle = hit.snippet.get(hit.highlight.clone()).unwrap_or("");
    for (offset, (turn, value)) in raw.into_iter().enumerate().skip(start).take(end - start) {
        let mut item = if offset == index {
            Item::from_value_near(&value, needle)
        } else {
            Item::from_value(&value)
        };
        excerpt |= item.body.starts_with("[Excerpt around the selected match]");
        item.turn_id = turn;
        if item.role == "Assistant" {
            item.rich = crate::chat_content::Content::parse(&item.body).ok();
        }
        items.push(Arc::new(item));
    }
    // Keep the selected source while reducing surrounding context to the same
    // presentation budget used by live conversations.
    let target = items
        .iter()
        .position(|item| item.id == hit.item_id && item.turn_id == hit.turn_id)
        .ok_or("Search anchor disappeared")?;
    let mut before = target;
    let mut after = target + 1;
    let mut bytes = items[target].weight();
    while before > 0 || after < items.len() {
        let mut progress = false;
        if before > 0 && bytes + items[before - 1].weight() <= crate::model::MAX_VISIBLE_BYTES {
            before -= 1;
            bytes += items[before].weight();
            progress = true;
        }
        if after < items.len() && bytes + items[after].weight() <= crate::model::MAX_VISIBLE_BYTES {
            bytes += items[after].weight();
            after += 1;
            progress = true;
        }
        if !progress {
            break;
        }
    }
    Ok(Some(Window {
        items: items.drain(before..after).collect(),
        target: hit.item_id.clone(),
        excerpt,
    }))
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
        let request = input.borrow_and_update().clone();
        let Some(request) = request else {
            ready = false;
            continue;
        };
        let context = request.context.clone();
        let serial = request.serial;
        tokio::select! {
            incoming = input.changed() => { if incoming.is_err() { break; } ready = true; },
            result = load(&api, request) => { ready = false; if events.send(crate::protocol::Event::ConversationFocus(Response { context, serial, result })).await.is_err() { break; } },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hit() -> Occurrence {
        crate::session_search::remote(json!({"data":[{"turnId":"old","itemId":"target","snippet":"needle","snippetMatchRange":{"start":0,"end":6}}]})).unwrap().remove(0)
    }
    #[test]
    fn old_targets_keep_bounded_context_and_large_response_matches() {
        let mut items = (0..150)
            .map(|index| json!({"id":index.to_string(),"type":"agentMessage","text":"context"}))
            .collect::<Vec<_>>();
        items[75] = json!({"id":"target","type":"agentMessage","text":format!("{}needle{}", "中".repeat(100_000), "字".repeat(100_000))});
        let window = project(json!([{"id":"old","items":items}]), &hit(), false)
            .unwrap()
            .unwrap();
        assert_eq!(window.items.len(), 81);
        assert!(window.excerpt);
        assert!(window
            .items
            .iter()
            .find(|item| item.id == "target")
            .unwrap()
            .body
            .contains("needle"));
        assert!(window.items.iter().all(|item| item.turn_id == "old"));
        assert!(
            window.items.iter().map(|item| item.weight()).sum::<usize>()
                <= crate::model::MAX_VISIBLE_BYTES
        );
    }
}
