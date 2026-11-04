//! One coalesced, bounded CPU job for the selected native transcript.
use crate::{model::Item, protocol::Event};
use std::sync::Arc;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Context {
    pub backend: String,
    pub socket: u64,
    pub history: u64,
    pub thread: String,
}
#[derive(Clone, Debug)]
pub struct Request {
    pub context: Context,
    pub serial: u64,
    pub source: Arc<Item>,
}
#[derive(Debug)]
pub struct Response {
    pub request: Request,
    pub result: Result<Arc<Item>, String>,
}
pub fn candidate(
    items: &[Arc<Item>],
    active: Option<&str>,
    failed: Option<&Arc<Item>>,
) -> Option<Arc<Item>> {
    let eligible = |item: &&Arc<Item>| {
        item.role == "Assistant"
            && !item.body.is_empty()
            && item.markup.is_none()
            && !failed.is_some_and(|f| Arc::ptr_eq(f, item))
            && item
                .rich
                .as_ref()
                .is_none_or(|r| r.source_bytes != item.body.len())
    };
    // Finish a previous turn's remaining text before spending every preview
    // slot on a newer, continuously changing reply.
    items
        .iter()
        .rev()
        .find(|item| eligible(item) && Some(item.turn_id.as_str()) != active)
        .or_else(|| items.iter().rev().find(eligible))
        .cloned()
}
pub fn prepare(request: Request) -> Response {
    let result = (|| {
        if request.source.role != "Assistant" || request.source.body.len() > 129 * 1024 {
            return Err("Streaming preview exceeds its bounds".into());
        }
        let mut item = (*request.source).clone();
        match crate::chat_content::Content::parse_reusing(&item.body, item.rich.as_ref()) {
            Ok(content) => {
                item.rich = Some(content);
                item.markup = None;
            }
            Err(_) => {
                item.rich = None;
                item.markup = Some(crate::markdown::markup(&item.body));
            }
        }
        Ok(Arc::new(item))
    })();
    Response { request, result }
}
pub async fn run(
    mut input: tokio::sync::watch::Receiver<Option<Request>>,
    events: async_channel::Sender<Event>,
) {
    let mut ready = false;
    loop {
        if !ready && input.changed().await.is_err() {
            return;
        }
        ready = false;
        let Some(request) = input.borrow_and_update().clone() else {
            continue;
        };
        // Await this bounded parse even if selection changes. Dropping a
        // spawn_blocking future would leave detached CPU work behind.
        let failed = request.clone();
        let response = crate::worker::compute(move || Ok(prepare(request)))
            .await
            .unwrap_or_else(|error| Response {
                request: failed,
                result: Err(error),
            });
        match input.has_changed() {
            Ok(true) => {
                ready = true;
                continue;
            }
            Err(_) => return,
            _ => {}
        }
        if events.send(Event::Streaming(response)).await.is_err() {
            return;
        }
    }
}
pub fn apply(current: &mut Arc<Item>, response: &Response) -> bool {
    let source = &response.request.source;
    let Ok(item) = &response.result else {
        return false;
    };
    if current.id != source.id
        || current.turn_id != source.turn_id
        || current.role != source.role
        || current.origin != source.origin
        || !current.body.starts_with(&source.body)
    {
        return false;
    }
    if current
        .rich
        .as_ref()
        .is_some_and(|rich| rich.source_bytes >= source.body.len())
        || current.markup.is_some()
    {
        return false;
    }
    if item.rich.is_none() && current.body != source.body {
        return false;
    }
    if current.body == source.body
        && current.images == source.images
        && current.output == source.output
        && current.truncated == source.truncated
    {
        *current = item.clone();
    } else {
        let current = Arc::make_mut(current);
        current.rich = item.rich.clone();
        current.markup = if current.body == source.body {
            item.markup.clone()
        } else {
            None
        };
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    fn make_request(text: &str) -> Request {
        Request {
            context: Context {
                backend: "codex".into(),
                socket: 3,
                history: 4,
                thread: "t".into(),
            },
            serial: 1,
            source: Arc::new(Item::from_value(
                &serde_json::json!({"id":"a","type":"agentMessage","text":text,"turnId":"turn"}),
            )),
        }
    }
    #[test]
    fn snapshots_keep_newer_deltas_and_reject_replacements_and_completed_items() {
        let request = make_request("# Heading\n\nFirst 中文😀");
        let response = prepare(request.clone());
        let mut current = request.source.clone();
        Arc::make_mut(&mut current).append(" and newer");
        assert!(apply(&mut current, &response));
        assert!(current.body.ends_with(" and newer"));
        assert_eq!(
            current.rich.as_ref().unwrap().source_bytes,
            request.source.body.len()
        );
        assert!(!apply(&mut current, &response));
        let mut fully = (*current).clone();
        fully.rich = Some(crate::chat_content::Content::parse(&fully.body).unwrap());
        assert_ne!(current, Arc::new(fully));
        let mut changed = (*request.source).clone();
        changed.body = "replacement".into();
        assert!(!apply(&mut Arc::new(changed), &response));
        let mut changed = (*request.source).clone();
        changed.turn_id = "other".into();
        assert!(!apply(&mut Arc::new(changed), &response));
        let mut complete = request.source.clone();
        let content = crate::chat_content::Content::parse(&complete.body).unwrap();
        Arc::make_mut(&mut complete).rich = Some(content);
        assert!(!apply(&mut complete, &response));
        let mut metadata = request.source.clone();
        Arc::make_mut(&mut metadata).output = Some(crate::conversation_output::Reference {
            token: "newer-source".into(),
            bytes: 42,
            unavailable: None,
        });
        assert!(apply(&mut metadata, &response));
        assert_eq!(metadata.output.as_ref().unwrap().token, "newer-source");
        let mut invalid = make_request("small");
        Arc::make_mut(&mut invalid.source).body = "x".repeat(130 * 1024);
        assert!(prepare(invalid).result.is_err());
    }
    #[test]
    fn immutable_blocks_and_table_state_share_only_compatible_presentations() {
        let a = crate::chat_content::Content::parse(
            "Closed **paragraph**.\n\n| A | B |\n|---|---|\n| one | two |\n",
        )
        .unwrap();
        let b = crate::chat_content::Content::parse_reusing(
            "Closed **paragraph**.\n\n| A | B |\n|---|---|\n| one | two |\n| three | four |\n",
            Some(&a),
        )
        .unwrap();
        assert!(Arc::ptr_eq(&a.rows[0], &b.rows[0]));
        assert!(b.retain_state(&a, 1));
        assert!(!Arc::ptr_eq(&a.rows[1], &b.rows[1]));
        let a = crate::chat_content::Content::parse("[link][later]\n\n").unwrap();
        let b = crate::chat_content::Content::parse_reusing(
            "[link][later]\n\n[later]: https://example.invalid\n",
            Some(&a),
        )
        .unwrap();
        assert!(!Arc::ptr_eq(&a.rows[0], &b.rows[0]));
        assert!(!b.retain_state(&a, 0));
    }
    #[test]
    fn worker_watch_coalesces_a_burst_to_the_latest_snapshot() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let (input, receiver) = tokio::sync::watch::channel(None);
                let (events, output) = async_channel::bounded(2);
                let task = tokio::spawn(run(receiver, events));
                for serial in 1..=100 {
                    let mut r = make_request("**Newest 中文😀**");
                    r.serial = serial;
                    input.send_replace(Some(r));
                }
                let event = tokio::time::timeout(std::time::Duration::from_secs(3), output.recv())
                    .await
                    .unwrap()
                    .unwrap();
                let Event::Streaming(response) = event else {
                    panic!("Wrong native event")
                };
                assert_eq!(response.request.serial, 100);
                assert!(response.result.unwrap().rich.is_some());
                assert!(output.try_recv().is_err());
                drop(input);
                task.await.unwrap();
            });
    }
    #[test]
    fn a_new_live_reply_cannot_starve_the_previous_completed_preview() {
        let completed = make_request("finished text").source;
        let mut live = make_request("new live text").source;
        Arc::make_mut(&mut live).turn_id = "next-turn".into();
        assert!(Arc::ptr_eq(
            &candidate(&[completed.clone(), live.clone()], Some("next-turn"), None).unwrap(),
            &completed
        ));
        let ready = prepare(Request {
            source: completed.clone(),
            ..make_request("unused")
        })
        .result
        .unwrap();
        assert!(Arc::ptr_eq(
            &candidate(&[ready, live.clone()], Some("next-turn"), None).unwrap(),
            &live
        ));
        assert!(candidate(&[live.clone()], Some("next-turn"), Some(&live)).is_none());
    }
}
