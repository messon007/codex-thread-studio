//! Shared-store Session Maps. HTTP, validation and tree preparation stay off GTK.
#[allow(dead_code)]
#[path = "../../../src-tauri/src/session_map_types.rs"]
pub mod types;
use crate::gateway::Api;
use reqwest::{Method, StatusCode};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use types::{MapOperation, SessionMap};
#[cfg(test)]
#[path = "session_map_tests.rs"]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Anchor {
    pub backend: String,
    pub thread: String,
}
impl Anchor {
    pub fn path(&self) -> Result<String, String> {
        let mut bytes = self.backend.bytes();
        if self.backend.len() > 64
            || !bytes.next().is_some_and(|c| c.is_ascii_lowercase())
            || !bytes
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'-' | b'_'))
            || self.thread.trim().is_empty()
            || self.thread.len() > 256
            || self.thread.chars().any(char::is_control)
        {
            return Err("Invalid Session Map identity".into());
        }
        let encode = |value: &str| {
            percent_encoding::utf8_percent_encode(value, percent_encoding::NON_ALPHANUMERIC)
                .to_string()
        };
        Ok(format!(
            "/studio/session-map/{}/{}",
            encode(&self.backend),
            encode(&self.thread)
        ))
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub index: usize,
    pub depth: usize,
}
#[derive(Debug)]
pub struct Snapshot {
    pub map: Arc<SessionMap>,
    pub rows: Arc<[Row]>,
    pub trail: Arc<[usize]>,
    pub done: usize,
}
impl Snapshot {
    pub fn decode(value: Value, anchor: &Anchor) -> Result<Self, String> {
        anchor.path()?;
        let map: SessionMap =
            serde_json::from_value(value).map_err(|_| "Gateway returned an invalid Session Map")?;
        if map.backend != anchor.backend || map.thread_id != anchor.thread || map.revision == 0 {
            return Err("Session Map identity/revision does not match the request".into());
        }
        types::validate_snapshot(&map)?;
        let by_id: HashMap<_, _> = map
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| (item.id.as_str(), i))
            .collect();
        let mut children: HashMap<Option<&str>, Vec<usize>> = HashMap::new();
        for (index, item) in map
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| !item.archived)
        {
            children
                .entry(item.parent_id.as_deref())
                .or_default()
                .push(index);
        }
        for items in children.values_mut() {
            items.sort_by(|a, b| {
                map.items[*a]
                    .position
                    .cmp(&map.items[*b].position)
                    .then_with(|| map.items[*a].title.cmp(&map.items[*b].title))
                    .then_with(|| a.cmp(b))
            });
        }
        let mut rows = Vec::new();
        let mut seen = HashSet::new();
        fn walk<'a>(
            parent: Option<&'a str>,
            depth: usize,
            map: &'a SessionMap,
            children: &HashMap<Option<&'a str>, Vec<usize>>,
            seen: &mut HashSet<usize>,
            rows: &mut Vec<Row>,
        ) {
            if let Some(items) = children.get(&parent) {
                for &index in items {
                    if seen.insert(index) {
                        rows.push(Row { index, depth });
                        walk(
                            Some(&map.items[index].id),
                            depth + 1,
                            map,
                            children,
                            seen,
                            rows,
                        );
                    }
                }
            }
        }
        walk(None, 0, &map, &children, &mut seen, &mut rows);
        for (index, _) in map
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| !item.archived)
        {
            if seen.insert(index) {
                rows.push(Row { index, depth: 0 });
            }
        }
        let mut trail = Vec::new();
        let mut current = map.current_item_id.as_deref();
        while let Some(index) = current.and_then(|id| by_id.get(id)).copied() {
            if map.items[index].archived {
                break;
            }
            trail.push(index);
            current = map.items[index].parent_id.as_deref();
        }
        trail.reverse();
        let done = rows
            .iter()
            .filter(|row| map.items[row.index].state == "done")
            .count();
        Ok(Self {
            map: Arc::new(map),
            rows: rows.into(),
            trail: trail.into(),
            done,
        })
    }
}
#[derive(Clone, Debug)]
pub enum Actor {
    User,
    Assistant,
}
#[derive(Clone, Debug)]
pub enum Request {
    Load(Anchor),
    Create(types::CreateMapRequest),
    Apply {
        anchor: Anchor,
        map_id: String,
        revision: u64,
        actor: Actor,
        source_turn: Option<String>,
        operations: Vec<MapOperation>,
    },
    Undo {
        anchor: Anchor,
        map_id: String,
        revision: u64,
    },
    Delete {
        anchor: Anchor,
        map_id: String,
        revision: u64,
    },
}
#[derive(Clone, Debug)]
pub struct Failure {
    pub message: String,
    pub conflict: bool,
    /// A mutation's transport/snapshot failed after dispatch. Never replay it.
    pub uncertain: bool,
}
impl Failure {
    fn rejected(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            conflict: false,
            uncertain: false,
        }
    }
}
#[derive(Debug)]
struct Prepared {
    method: Method,
    path: String,
    body: Option<Value>,
    anchor: Anchor,
    expected: Option<u64>,
    expected_map_id: Option<String>,
    delete: bool,
}
fn prepare(request: Request) -> Result<Prepared, String> {
    let expected_map_id = match &request {
        Request::Apply { map_id, .. }
        | Request::Undo { map_id, .. }
        | Request::Delete { map_id, .. } => Some(map_id.clone()),
        _ => None,
    };
    if expected_map_id
        .as_ref()
        .is_some_and(|id| id.trim().is_empty() || id.len() > 128)
    {
        return Err("Invalid Session Map identity".into());
    }
    let (method, path, body, anchor, expected, delete) = match request {
        Request::Load(anchor) => (Method::GET, anchor.path()?, None, anchor, None, false),
        Request::Create(request) => {
            let anchor = Anchor {
                backend: request.backend.clone(),
                thread: request.thread_id.clone(),
            };
            anchor.path()?;
            (
                Method::POST,
                "/studio/session-map".into(),
                Some(serde_json::to_value(request).map_err(|e| e.to_string())?),
                anchor,
                Some(1),
                false,
            )
        }
        Request::Apply {
            anchor,
            map_id,
            revision,
            actor,
            source_turn,
            operations,
        } => {
            if operations.is_empty() || operations.len() > types::MAX_OPERATIONS {
                return Err("Invalid Session Map operations".into());
            }
            let actor = match actor {
                Actor::User => "user",
                Actor::Assistant => "assistant",
            };
            let body = json!({"mapId":map_id,"baseRevision":revision,"actor":actor,"sourceTurnId":source_turn,"operations":operations});
            (
                Method::POST,
                format!("{}/operations", anchor.path()?),
                Some(body),
                anchor,
                Some(
                    revision
                        .checked_add(1)
                        .ok_or("Invalid Session Map revision")?,
                ),
                false,
            )
        }
        Request::Undo {
            anchor,
            map_id,
            revision,
        } => (
            Method::POST,
            format!(
                "{}/undo?{}",
                anchor.path()?,
                version_query(revision, &map_id)
            ),
            None,
            anchor,
            Some(
                revision
                    .checked_add(1)
                    .ok_or("Invalid Session Map revision")?,
            ),
            false,
        ),
        Request::Delete {
            anchor,
            map_id,
            revision,
        } => (
            Method::DELETE,
            format!("{}?{}", anchor.path()?, version_query(revision, &map_id)),
            None,
            anchor,
            Some(revision),
            true,
        ),
    };
    if let Some(body) = &body {
        if serde_json::to_vec(body).map_err(|e| e.to_string())?.len() > types::MAX_MAP_BODY_BYTES {
            return Err("Session Map request exceeds 512 KiB".into());
        }
    }
    Ok(Prepared {
        method,
        path,
        body,
        anchor,
        expected,
        expected_map_id,
        delete,
    })
}
fn version_query(revision: u64, map_id: &str) -> String {
    format!(
        "revision={revision}&mapId={}",
        percent_encoding::utf8_percent_encode(map_id, percent_encoding::NON_ALPHANUMERIC)
    )
}
pub async fn execute(api: &Api, request: Request) -> Result<Option<Arc<Snapshot>>, Failure> {
    let prepared = crate::worker::compute(move || prepare(request))
        .await
        .map_err(Failure::rejected)?;
    let read = prepared.method == Method::GET;
    let (status, value) = api
        .json_status_optional(prepared.method, &prepared.path, prepared.body.as_ref())
        .await
        .map_err(|message| Failure {
            message,
            conflict: false,
            uncertain: !read,
        })?;
    if read && status == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !status.is_success() {
        return Err(Failure {
            message: crate::session_map_protocol::notice(&crate::gateway::response_error(
                status, &value,
            )),
            conflict: status == StatusCode::CONFLICT,
            uncertain: false,
        });
    }
    crate::worker::compute(move || {
        let snapshot = Snapshot::decode(value, &prepared.anchor)?;
        if prepared
            .expected_map_id
            .as_ref()
            .is_some_and(|id| snapshot.map.id != *id)
        {
            return Err(
                "Session Map reply belongs to a replacement map; reload before another change"
                    .into(),
            );
        }
        if prepared
            .expected
            .is_some_and(|expected| snapshot.map.revision != expected)
        {
            return Err(
                "Session Map reply has an unexpected revision; reload before another change".into(),
            );
        }
        Ok(if prepared.delete {
            None
        } else {
            Some(Arc::new(snapshot))
        })
    })
    .await
    .map_err(|message| Failure {
        message,
        conflict: false,
        uncertain: !read,
    })
}
