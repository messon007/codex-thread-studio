//! Bounded prompt metadata on the shared CPU pool, coalesced independently of I/O.
use crate::{model::Item, session_search::Context};
use std::{collections::HashSet, sync::Arc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marker {
    pub turn: String,
    pub item: String,
    pub preview: String,
}
#[derive(Clone, Debug)]
pub struct Request {
    pub serial: u64,
    pub context: Context,
    pub users: Vec<Arc<Item>>,
}
#[derive(Debug)]
pub struct Response {
    pub serial: u64,
    pub context: Context,
    pub result: Result<Vec<Marker>, String>,
}
fn preview(source: &str) -> String {
    let mut text = String::new();
    let mut length = 0;
    let mut space = false;
    let mut clipped = false;
    for c in source.chars() {
        if c.is_whitespace() {
            space = length > 0;
            continue;
        }
        if space {
            if length == 72 {
                clipped = true;
                break;
            }
            text.push(' ');
            length += 1;
            space = false;
        }
        if length == 72 {
            clipped = true;
            break;
        }
        text.push(if c == '\0' { '�' } else { c });
        length += 1;
    }
    // Detect a clipped prefix without counting/copying the entire message.
    if clipped {
        text.pop();
        text = text.trim_end().to_owned();
        text.push('…');
    }
    text
}
pub fn prepare(users: &[Arc<Item>]) -> Vec<Marker> {
    let mut seen = HashSet::new();
    users
        .iter()
        .filter(|item| {
            item.role == "You"
                && !item.id.is_empty()
                && !item.turn_id.is_empty()
                && seen.insert(item.turn_id.as_str())
        })
        .map(|item| Marker {
            turn: item.turn_id.clone(),
            item: item.id.clone(),
            preview: preview(&item.body),
        })
        .collect()
}
pub async fn run(
    mut requests: tokio::sync::watch::Receiver<Option<Request>>,
    events: async_channel::Sender<crate::protocol::Event>,
) {
    while requests.changed().await.is_ok() {
        loop {
            let Some(request) = requests.borrow_and_update().clone() else {
                break;
            };
            let serial = request.serial;
            let context = request.context;
            let result = crate::worker::compute(move || Ok(prepare(&request.users))).await;
            // Finish one CPU job before taking the latest input. No cancelled
            // spawn_blocking jobs accumulate while the user switches pages.
            if requests.has_changed().unwrap_or(false) {
                continue;
            }
            if events
                .send(crate::protocol::Event::TurnNavigation(Response {
                    serial,
                    context,
                    result,
                }))
                .await
                .is_err()
            {
                return;
            }
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn navigates_real_user_turns_with_literal_unicode_previews_and_attachment_fallback() {
        let user = |id: &str, turn: &str, text: &str| {
            let mut item = Item::from_value(&json!({"id":id,"turnId":turn,
                "type":"userMessage","content":[{"type":"text","text":text}]}));
            item.turn_id = turn.into();
            Arc::new(item)
        };
        let rows = prepare(&[
            user("one", "t1", "\t 中文😀  literal {index}\n next "),
            user("duplicate", "t1", "same turn"),
            Arc::new(Item::from_value(
                &json!({"id":"reply","turnId":"t2","type":"agentMessage","text":"not a prompt"}),
            )),
            user("image", "t3", ""),
            user("invalid", "", "no source identity"),
            user("long", "t4", &"😀".repeat(100)),
        ]);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].preview, "中文😀 literal {index} next");
        assert_eq!(rows[1].item, "image");
        assert!(rows[1].preview.is_empty());
        assert_eq!(rows[2].preview.chars().count(), 72);
        assert!(rows[2].preview.ends_with('…'));
        assert_eq!(preview(&"字".repeat(72)), "字".repeat(72));
    }
}
