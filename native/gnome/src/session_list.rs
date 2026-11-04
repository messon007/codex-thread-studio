//! Coalesced session filtering/sorting on one Rust worker, independent of GTK.
use crate::{model::Thread, session_directory_policy::Filter};
use std::{collections::HashMap, sync::Arc};
#[derive(Clone, Debug)]
pub struct Status {
    pub active: bool,
    pub attention: bool,
    pub stale: bool,
}
#[derive(Clone, Debug)]
pub struct Row {
    pub thread: Thread,
    pub status: Option<Status>,
}
#[derive(Clone, Debug)]
pub struct Request {
    pub serial: u64,
    pub backend: String,
    pub rows: Arc<Vec<Row>>,
    pub pins: Vec<String>,
    pub pinned_only: bool,
    pub scope: u32,
    pub query: String,
    pub filter: Arc<Filter>,
}
pub fn prepare(request: Request) -> Result<Vec<Thread>, String> {
    let query = request.query.to_lowercase();
    if query.len() > 4096 {
        return Err("Session search exceeds 4096 bytes".into());
    }
    let ranks = request
        .pins
        .into_iter()
        .enumerate()
        .map(|(i, key)| (key, i))
        .collect::<HashMap<_, _>>();
    let mut rows = Vec::new();
    for row in request.rows.iter() {
        let thread = &row.thread;
        if request.filter.hidden(&thread.cwd) {
            continue;
        }
        let rank = ranks
            .get(&format!("{}:{}", request.backend, thread.id))
            .copied();
        if request.pinned_only && rank.is_none() {
            continue;
        }
        if request.scope == 3
            && !row
                .status
                .as_ref()
                .map_or(thread.activity == "active", |s| s.active && !s.stale)
        {
            continue;
        }
        if request.scope == 4 && !row.status.as_ref().is_some_and(|s| s.attention) {
            continue;
        }
        if !query.is_empty()
            && !thread.title.to_lowercase().contains(&query)
            && !thread.cwd.to_lowercase().contains(&query)
        {
            continue;
        }
        let mut thread = thread.clone();
        thread.pinned = rank.is_some();
        if let Some(s) = row.status.as_ref() {
            thread.activity = if s.stale {
                "unknown"
            } else if s.active {
                "active"
            } else if s.attention {
                "attention"
            } else {
                "idle"
            }
            .into();
        }
        rows.push((rank.unwrap_or(usize::MAX), thread));
    }
    rows.sort_by_key(|(rank, _)| *rank);
    Ok(rows.into_iter().map(|(_, thread)| thread).collect())
}
pub async fn run(
    mut requests: tokio::sync::watch::Receiver<Option<Request>>,
    events: async_channel::Sender<crate::protocol::Event>,
) {
    loop {
        if requests.changed().await.is_err() {
            break;
        }
        loop {
            let request = requests.borrow_and_update().clone();
            let Some(request) = request else {
                break;
            };
            let serial = request.serial;
            let result = crate::worker::compute(move || prepare(request)).await;
            // Finish the current bounded job, then skip superseded output. No
            // abandoned CPU jobs pile up when keys/lifecycle events arrive quickly.
            if requests.has_changed().unwrap_or(false) {
                continue;
            }
            if events
                .send(crate::protocol::Event::SessionRows { serial, result })
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
    fn hidden_rules_precede_pins_search_and_status_without_losing_literal_spaces() {
        let row = |id: &str, cwd: &str| Row {
            thread: Thread::from_value(&json!({"id":id,"name":format!("中文 {id}"),"cwd":cwd}))
                .unwrap(),
            status: Some(Status {
                active: true,
                attention: true,
                stale: false,
            }),
        };
        let base = Request {
            serial: 1,
            backend: "codex".into(),
            rows: Arc::new(vec![
                row("hidden", "/private"),
                row("a", "/visible"),
                row("b", "/public/build/keep"),
                row("drop", "/public/build/drop"),
            ]),
            pins: vec!["codex:hidden".into(), "codex:b".into()],
            pinned_only: false,
            scope: 0,
            query: "".into(),
            filter: Arc::new(
                Filter::new(
                    &["/private".into()],
                    &["**/build/".into(), "!/public/build/keep/".into()],
                )
                .unwrap(),
            ),
        };
        let rows = prepare(base.clone()).unwrap();
        assert_eq!(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["b", "a"]
        );
        assert!(rows[0].pinned);
        assert!(prepare(Request {
            query: "中文 a ".into(),
            ..base.clone()
        })
        .unwrap()
        .is_empty());
        assert_eq!(
            prepare(Request {
                scope: 4,
                pinned_only: true,
                ..base
            })
            .unwrap()
            .len(),
            1
        );
    }
}
