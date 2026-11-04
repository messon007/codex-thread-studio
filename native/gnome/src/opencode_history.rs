//! Paged OpenCode reads, independent from the live SSE projection and composer.
use crate::{
    conversation_focus::{Request, Window},
    gateway::Api,
    model::{text, Item},
    session_search::{Filter, Occurrence},
};
use regex::Regex;
use serde_json::Value;
use std::{collections::HashSet, sync::Arc};
const PAGE_SIZE: usize = 80;
const CONTEXT_ITEMS: usize = 40;
const SEARCH_METADATA_BYTES: usize = 8 * 1024 * 1024;
#[derive(Default)]
pub(crate) struct Cursors {
    seen: HashSet<String>,
    bytes: usize,
}
impl Cursors {
    pub(crate) fn advance(&mut self, next: &str) -> Result<bool, String> {
        if self.seen.contains(next) {
            return Err("OpenCode repeated its history cursor".into());
        }
        let weight = next.len() + 128;
        if self.bytes.saturating_add(weight) > SEARCH_METADATA_BYTES {
            return Ok(false);
        }
        self.bytes += weight;
        self.seen.insert(next.into());
        Ok(true)
    }
}

pub(crate) async fn page(
    api: &Api,
    thread: &str,
    directory: &str,
    cursor: Option<&str>,
) -> Result<(Value, Option<String>), String> {
    let page = api
        .history_page(&crate::opencode::message_page_path(
            thread, directory, cursor, PAGE_SIZE,
        ))
        .await?;
    let rows = page
        .0
        .as_array()
        .ok_or("OpenCode returned an invalid message page")?;
    if rows.is_empty() && page.1.is_some() {
        return Err("OpenCode returned a cursor for an empty history page".into());
    }
    Ok(page)
}
fn prepare(
    value: Value,
    hit: &Occurrence,
    newer: Vec<Arc<Item>>,
) -> Result<(Option<Window>, Vec<Arc<Item>>), String> {
    let raw = crate::opencode::message_items(&value)?;
    let target = raw
        .iter()
        .position(|item| text(item, "id") == hit.item_id && text(item, "turnId") == hit.turn_id);
    if let Some(target) = target {
        let start = target.saturating_sub(CONTEXT_ITEMS);
        let end = (target + CONTEXT_ITEMS + 1).min(raw.len());
        let needle = hit.snippet.get(hit.highlight.clone()).unwrap_or("");
        let mut items = raw[start..end]
            .iter()
            .enumerate()
            .map(|(offset, value)| {
                Arc::new(if start + offset == target {
                    Item::from_value_near(value, needle)
                } else {
                    Item::from_value(value)
                })
            })
            .collect::<Vec<_>>();
        items.extend(
            newer
                .into_iter()
                .take((target + CONTEXT_ITEMS + 1).saturating_sub(raw.len())),
        );
        let mut weight = 0;
        let mut kept = Vec::new();
        for item in items {
            let mut item = Arc::unwrap_or_clone(item);
            if item.role == "Assistant" {
                item.rich = crate::chat_content::Content::parse(&item.body).ok();
            }
            if item.weight() > crate::model::MAX_VISIBLE_BYTES {
                item.rich = None;
            }
            weight += item.weight();
            kept.push(Arc::new(item));
        }
        // Remove the most distant surrounding items first, preserving the target.
        let target = kept
            .iter()
            .position(|item| item.id == hit.item_id && item.turn_id == hit.turn_id)
            .ok_or("OpenCode source anchor disappeared")?;
        let mut begin = 0;
        let mut end = kept.len();
        while weight > crate::model::MAX_VISIBLE_BYTES && end - begin > 1 {
            if target - begin > end - 1 - target {
                weight = weight.saturating_sub(kept[begin].weight());
                begin += 1;
            } else {
                end -= 1;
                weight = weight.saturating_sub(kept[end].weight());
            }
        }
        let excerpt = kept[target].truncated;
        return Ok((
            Some(Window {
                items: kept.drain(begin..end).collect(),
                target: hit.item_id.clone(),
                excerpt,
            }),
            Vec::new(),
        ));
    }
    // Only the nearest newer context survives the scan, with the same text bound
    // as live items. Never accumulate complete raw history across HTTP pages.
    let mut context = raw
        .iter()
        .take(CONTEXT_ITEMS)
        .map(|item| Arc::new(Item::from_value(item)))
        .collect::<Vec<_>>();
    if context.len() < CONTEXT_ITEMS {
        context.extend(newer.into_iter().take(CONTEXT_ITEMS - context.len()));
    }
    Ok((None, context))
}
pub async fn focus(api: &Api, request: Request) -> Result<Window, String> {
    let mut cursor = None;
    let mut visited = Cursors::default();
    let mut newer = Vec::new();
    loop {
        let (value, next) = page(
            api,
            &request.context.thread,
            &request.directory,
            cursor.as_deref(),
        )
        .await?;
        let hit = request.hit.clone();
        let (window, context) = crate::worker::compute(move || prepare(value, &hit, newer)).await?;
        if let Some(window) = window {
            return Ok(window);
        }
        newer = context;
        let Some(next) = next else {
            return Err("The saved response is not in the retrieved OpenCode history. This server may not expose older pages, or the response was removed.".into());
        };
        if !visited.advance(&next)? {
            return Err("OpenCode history is too large to scan in one request".into());
        }
        cursor = Some(next);
    }
}

struct Scan {
    matches: Vec<Occurrence>,
    final_turns: HashSet<String>,
    ids: HashSet<(String, String)>,
    metadata_bytes: usize,
    limited: bool,
}
impl Default for Scan {
    fn default() -> Self {
        Self {
            matches: Vec::new(),
            final_turns: HashSet::new(),
            ids: HashSet::new(),
            metadata_bytes: 0,
            limited: false,
        }
    }
}
impl Scan {
    fn add(&mut self, value: Value, pattern: &Regex, filter: Filter) -> Result<(), String> {
        for value in crate::opencode::message_items(&value)?.into_iter().rev() {
            let id = text(&value, "id");
            let turn = text(&value, "turnId");
            if id.is_empty() || id.len() > 256 || turn.is_empty() || turn.len() > 256 {
                return Err("OpenCode returned an invalid source anchor".into());
            }
            // Hash-set metadata is separately bounded even for extremely long,
            // nonmatching histories; raw text is released with each page.
            self.metadata_bytes += id.len() + turn.len() + 128;
            if self.metadata_bytes > SEARCH_METADATA_BYTES {
                self.limited = true;
                break;
            }
            if !self.ids.insert((turn.into(), id.into())) {
                continue;
            }
            let kind = match text(&value, "type") {
                "userMessage" => Filter::User,
                "agentMessage" if self.final_turns.insert(turn.into()) => Filter::Assistant,
                _ => Filter::Activity,
            };
            if filter != Filter::All && filter != kind {
                continue;
            }
            let (role, body) = Item::source_content(&value);
            let item = Item {
                output: None,
                details: None,
                status: None,
                truncated: false,
                images: Vec::new(),
                origin: None,
                id: id.into(),
                turn_id: turn.into(),
                role: role.into(),
                body,
                markup: None,
                rich: None,
            };
            if let Some(found) = pattern.find(&item.body) {
                let mut hit = crate::session_search::occurrence(&item, found, kind);
                hit.source = None;
                self.matches.push(hit);
                if self.matches.len() == 100 {
                    break;
                }
            }
        }
        Ok(())
    }
}
pub async fn search(
    api: &Api,
    thread: &str,
    directory: &str,
    query: &str,
    case_sensitive: bool,
    filter: Filter,
) -> Result<(Vec<Occurrence>, bool), String> {
    if query.is_empty() {
        return Ok((Vec::new(), true));
    }
    if query.len() > 4096 {
        return Err("Search text exceeds 4 KiB".into());
    }
    let pattern = regex::RegexBuilder::new(&regex::escape(query))
        .case_insensitive(!case_sensitive)
        .size_limit(2 * 1024 * 1024)
        .dfa_size_limit(1024 * 1024)
        .build()
        .map_err(|error| error.to_string())?;
    let mut scan = Scan::default();
    let mut cursor = None;
    let mut visited = Cursors::default();
    let mut paginated = false;
    loop {
        let (value, next) = page(api, thread, directory, cursor.as_deref()).await?;
        let count = value.as_array().map_or(0, Vec::len);
        let matcher = pattern.clone();
        scan = crate::worker::compute(move || {
            scan.add(value, &matcher, filter)?;
            Ok(scan)
        })
        .await?;
        if scan.limited {
            return Ok((scan.matches, false));
        }
        if scan.matches.len() == 100 {
            return Ok((scan.matches, true));
        }
        let Some(next) = next else {
            return Ok((scan.matches, paginated || count < PAGE_SIZE));
        };
        paginated = true;
        if !visited.advance(&next)? {
            return Ok((scan.matches, false));
        }
        cursor = Some(next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    struct Fixture {
        api: Api,
        mode: Arc<std::sync::atomic::AtomicU8>,
        requests: Arc<std::sync::Mutex<Vec<String>>>,
        task: tokio::task::JoinHandle<()>,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            self.task.abort();
        }
    }
    async fn fixture() -> Fixture {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let mode = Arc::new(std::sync::atomic::AtomicU8::new(0));
        let requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let serving_mode = mode.clone();
        let serving_requests = requests.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let mode = serving_mode.clone();
                let requests = serving_requests.clone();
                tokio::spawn(async move {
                    let mut bytes = Vec::new();
                    let mut chunk = [0; 4096];
                    while !bytes.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                        let count = stream.read(&mut chunk).await.unwrap();
                        if count == 0 {
                            return;
                        }
                        bytes.extend_from_slice(&chunk[..count]);
                        assert!(bytes.len() < 16 * 1024);
                    }
                    let request = String::from_utf8(bytes).unwrap();
                    assert!(request
                        .to_ascii_lowercase()
                        .contains("authorization: bearer 0123456789abcdef0123456789abcdef"));
                    let target = request
                        .lines()
                        .next()
                        .unwrap()
                        .split_whitespace()
                        .nth(1)
                        .unwrap()
                        .to_owned();
                    requests.lock().unwrap().push(target.clone());
                    let url = url::Url::parse(&format!("http://127.0.0.1{target}")).unwrap();
                    let older = url
                        .query_pairs()
                        .any(|(name, value)| name == "before" && value == "a/+?&=x");
                    let mode = mode.load(std::sync::atomic::Ordering::Relaxed);
                    let old = json!({"info":{"id":"old","role":"assistant","parentID":"u"},"parts":[{"id":"answer","type":"text","text":"NEEDLE 中文😀 "}]});
                    let recent = json!({"info":{"id":"recent","role":"assistant","parentID":"new-user"},"parts":[{"id":"recent-part","type":"text","text":"newer surrounding context"}]});
                    let (value,cursor)=match mode {
                        1=>(json!([recent]),Some("a/+?&=x".to_owned())),
                        2=>(Value::Array((0..PAGE_SIZE).map(|index|json!({"info":{"id":format!("r{index}"),"role":"assistant","parentID":format!("u{index}")},"parts":[{"id":format!("p{index}"),"type":"text","text":"latest legacy history"}]})).collect()),None),
                        3=>(json!([recent]),Some("x".repeat(4097))),
                        _ if older=>(json!([old]),None),
                        _=>(json!([recent]),Some("a/+?&=x".to_owned())),
                    };
                    let body = serde_json::to_vec(&value).unwrap();
                    let cursor = cursor
                        .map(|cursor| format!("x-next-cursor: {cursor}\r\n"))
                        .unwrap_or_default();
                    let header=format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{cursor}\r\n",body.len());
                    stream.write_all(header.as_bytes()).await.unwrap();
                    stream.write_all(&body).await.unwrap();
                });
            }
        });
        Fixture {
            api: Api::from_access_url(&format!(
                "http://127.0.0.1:{port}/#token=0123456789abcdef0123456789abcdef"
            ))
            .unwrap(),
            mode,
            requests,
            task,
        }
    }
    fn hit() -> Occurrence {
        Occurrence {
            turn_id: "u".into(),
            item_id: "answer".into(),
            kind: Filter::Assistant,
            snippet: "NEEDLE 中文😀 ".into(),
            highlight: 0.."NEEDLE 中文😀 ".len(),
            source: None,
            block: None,
            table_row: None,
            paragraph_page: None,
            paragraph_range: None,
        }
    }
    #[test]
    fn full_source_search_and_focus_preserve_unicode_and_parent_anchors() {
        let body = format!("{}NEEDLE 中文😀 ", "Earlier text.\n\n".repeat(20000));
        let page = json!([{"info":{"id":"a","role":"assistant","parentID":"u"},"parts":[{"id":"answer","type":"text","text":body}]}]);
        let mut scan = Scan::default();
        scan.add(
            page.clone(),
            &Regex::new("NEEDLE 中文😀 ").unwrap(),
            Filter::Assistant,
        )
        .unwrap();
        assert_eq!(scan.matches.len(), 1);
        assert!(scan.matches[0].source.is_none());
        assert_eq!(
            &scan.matches[0].snippet[scan.matches[0].highlight.clone()],
            "NEEDLE 中文😀 "
        );
        let (window, _) = prepare(page, &hit(), Vec::new()).unwrap();
        let window = window.unwrap();
        assert!(window.excerpt);
        assert!(window.items[0].body.contains("NEEDLE 中文😀 "));
        assert_eq!(window.items[0].turn_id, "u");
    }
    #[test]
    fn search_final_answer_classification_survives_page_boundaries_and_overlap() {
        let page = |id: &str| json!([{"info":{"id":id,"role":"assistant","parentID":"u"},"parts":[{"id":id,"type":"text","text":"match"}]}]);
        let pattern = Regex::new("match").unwrap();
        let mut scan = Scan::default();
        scan.add(page("newest"), &pattern, Filter::Assistant)
            .unwrap();
        scan.add(page("older"), &pattern, Filter::Assistant)
            .unwrap();
        scan.add(page("newest"), &pattern, Filter::Assistant)
            .unwrap();
        assert_eq!(scan.matches.len(), 1);
        assert_eq!(scan.matches[0].item_id, "newest");
        scan.metadata_bytes = SEARCH_METADATA_BYTES;
        scan.add(page("ancient"), &pattern, Filter::All).unwrap();
        assert!(scan.limited);
    }
    #[tokio::test]
    async fn authenticated_pages_follow_opaque_cursors_and_report_legacy_limits() {
        let fixture = fixture().await;
        let request = Request {
            backend_kind: "opencode".into(),
            directory: "/tmp/中文 &project=1".into(),
            context: crate::session_search::Context {
                backend: "opencode".into(),
                thread: "s/?#".into(),
                generation: 1,
            },
            serial: 1,
            hit: hit(),
        };
        let window = focus(&fixture.api, request.clone()).await.unwrap();
        assert_eq!(
            window
                .items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["answer", "recent-part"]
        );
        let (found, complete) = search(
            &fixture.api,
            &request.context.thread,
            &request.directory,
            "NEEDLE 中文😀 ",
            true,
            Filter::Assistant,
        )
        .await
        .unwrap();
        assert!(complete);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].item_id, "answer");
        let urls = fixture.requests.lock().unwrap().clone();
        assert_eq!(urls.len(), 4);
        for target in urls {
            let url = url::Url::parse(&format!("http://127.0.0.1{target}")).unwrap();
            assert!(url
                .path()
                .starts_with("/opencode/session/s%2F%3F%23/message"));
            assert!(url
                .query_pairs()
                .any(|(name, value)| name == "directory" && value == request.directory));
        }
        fixture.mode.store(1, std::sync::atomic::Ordering::Relaxed);
        assert!(focus(&fixture.api, request.clone())
            .await
            .unwrap_err()
            .contains("repeated"));
        assert!(search(
            &fixture.api,
            &request.context.thread,
            &request.directory,
            "absent",
            true,
            Filter::All
        )
        .await
        .unwrap_err()
        .contains("repeated"));
        fixture.mode.store(2, std::sync::atomic::Ordering::Relaxed);
        let (found, complete) = search(
            &fixture.api,
            &request.context.thread,
            &request.directory,
            "absent",
            true,
            Filter::All,
        )
        .await
        .unwrap();
        assert!(!complete);
        assert!(found.is_empty());
        assert!(focus(&fixture.api, request.clone())
            .await
            .unwrap_err()
            .contains("older pages"));
        fixture.mode.store(3, std::sync::atomic::Ordering::Relaxed);
        assert!(focus(&fixture.api, request)
            .await
            .unwrap_err()
            .contains("invalid history cursor"));
    }
}
