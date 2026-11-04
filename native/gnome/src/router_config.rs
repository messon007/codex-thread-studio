//! Shared Router configuration I/O and bounded target discovery on Rust workers.
use crate::{
    gateway::Api,
    model::{Backend, Thread},
};
use futures_util::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

pub const DEFAULT_CONDITION: &str = "No regular session is suitable for the user query.";
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Fallback {
    pub session_key: String,
    pub condition: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Opening {
    pub responsibility: String,
    pub opening_message: String,
    pub revision: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub revision: String,
    pub controller_backend: String,
    pub controllers: BTreeMap<String, String>,
    pub fallbacks: Vec<Fallback>,
    pub openings: BTreeMap<String, Opening>,
    pub empty_opening_revision: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedOpening {
    pub session_key: String,
    #[serde(flatten)]
    pub opening: Opening,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryCandidate {
    pub thread_id: String,
    pub workspace: String,
    pub title: String,
    pub status: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recovery {
    pub backend: String,
    pub revision: String,
    pub journal_revision: String,
    pub state: String,
    pub workspace: Option<String>,
    pub thread_id: Option<String>,
    pub candidate: Option<RecoveryCandidate>,
}
#[derive(Clone, Debug)]
pub struct Target {
    pub backend: String,
    pub thread: Thread,
}
impl Target {
    pub fn key(&self) -> String {
        format!("{}:{}", self.backend, self.thread.id)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub backend: String,
    pub fallbacks: Vec<Fallback>,
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        let mut keys = HashSet::new();
        if self.fallbacks.len() > 3 {
            return Err("Choose at most three fallback sessions".into());
        }
        for fallback in &self.fallbacks {
            if !keys.insert(&fallback.session_key) {
                return Err("A fallback session cannot be selected twice".into());
            }
            if fallback.condition.trim().is_empty() || fallback.condition.chars().count() > 4096 {
                return Err("Each fallback needs a condition of at most 4,096 characters".into());
            }
        }
        Ok(())
    }
}
#[derive(Debug)]
pub enum Request {
    Read,
    Recovery {
        backend: String,
        revision: String,
        thread_id: Option<String>,
    },
    Adopt {
        backend: String,
        revision: String,
        journal_revision: String,
        thread_id: String,
    },
    Controller {
        revision: String,
        backend: String,
    },
    Save {
        revision: String,
        settings: Settings,
    },
    Responsibility {
        key: String,
        revision: String,
        text: String,
    },
    Targets(Vec<Backend>),
}
#[derive(Debug)]
pub enum Reply {
    Snapshot(Snapshot),
    Recovery(Recovery),
    Controller {
        snapshot: Snapshot,
        thread_id: String,
        warning: Option<String>,
    },
    Responsibility(SavedOpening),
    Targets {
        targets: Arc<Vec<Target>>,
        issues: Vec<String>,
    },
}
pub async fn execute(api: &Api, request: Request) -> Result<Reply, String> {
    match request {
        Request::Recovery {
            backend,
            revision,
            thread_id,
        } => {
            let raw=api.json(reqwest::Method::POST,"/studio/native/router/controller/recovery",Some(&json!({"backend":backend,"expectedRevision":revision,"threadId":thread_id}))).await?;
            crate::worker::compute(move || {
                let value: Recovery =
                    serde_json::from_value(raw).map_err(|_| "Invalid controller recovery reply")?;
                if value.backend != backend
                    || value.revision != revision
                    || value.journal_revision.len() != 16
                    || !matches!(
                        value.state.as_str(),
                        "none" | "uncertain" | "known" | "configured"
                    )
                    || value.workspace.as_ref().is_some_and(|s| s.len() > 4096)
                    || value
                        .thread_id
                        .as_ref()
                        .is_some_and(|s| s.is_empty() || s.len() > 256)
                    || value.candidate.as_ref().is_some_and(|c| {
                        thread_id.as_deref() != Some(&c.thread_id)
                            || value.workspace.as_deref() != Some(&c.workspace)
                            || c.title.chars().count() > 300
                            || !matches!(c.status.as_str(), "idle" | "notLoaded")
                    })
                    || thread_id.is_some() != value.candidate.is_some()
                {
                    return Err("Controller recovery scope changed; reload before adopting".into());
                }
                Ok(Reply::Recovery(value))
            })
            .await
        }
        Request::Adopt {
            backend,
            revision,
            journal_revision,
            thread_id,
        } => {
            let raw=api.json(reqwest::Method::POST,"/studio/native/router/controller/adopt",Some(&json!({"backend":backend,"expectedRevision":revision,"journalRevision":journal_revision,"threadId":thread_id}))).await?;
            crate::worker::compute(move || {
                if raw["backend"] != backend
                    || raw["threadId"] != thread_id
                    || raw["adopted"] != true
                    || raw["created"] != false
                {
                    return Err(
                        "Controller adoption acknowledgement changed identity; reload settings"
                            .into(),
                    );
                }
                let snapshot = parse_snapshot(raw["snapshot"].clone())?;
                if snapshot.controllers.get(&backend) != Some(&thread_id) {
                    return Err(
                        "Adoption reply did not register the inspected controller; reload settings"
                            .into(),
                    );
                }
                Ok(Reply::Controller {
                    snapshot,
                    thread_id,
                    warning: None,
                })
            })
            .await
        }
        Request::Controller { revision, backend } => {
            let raw = api
                .json(
                    reqwest::Method::POST,
                    "/studio/native/router/controller",
                    Some(&json!({"expectedRevision":revision,"backend":backend})),
                )
                .await?;
            crate::worker::compute(move || {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct Controller {
                    snapshot: Snapshot,
                    thread_id: String,
                    warning: Option<String>,
                }
                let reply: Controller =
                    serde_json::from_value(raw).map_err(|_| "Invalid Router controller reply")?;
                Ok(Reply::Controller {
                    snapshot: reply.snapshot,
                    thread_id: reply.thread_id,
                    warning: reply.warning,
                })
            })
            .await
        }
        Request::Read => {
            let raw = api
                .json(reqwest::Method::GET, "/studio/native/router", None)
                .await?;
            crate::worker::compute(move || parse_snapshot(raw).map(Reply::Snapshot)).await
        }
        Request::Save { revision, settings } => {
            settings.validate()?;
            let body = json!({"expectedRevision":revision,"controllerBackend":settings.backend,"fallbacks":settings.fallbacks});
            let raw = api
                .json(reqwest::Method::PATCH, "/studio/native/router", Some(&body))
                .await?;
            crate::worker::compute(move || parse_snapshot(raw).map(Reply::Snapshot)).await
        }
        Request::Responsibility {
            key,
            revision,
            text,
        } => {
            if text.chars().count() > 4096 {
                return Err("Responsibilities support at most 4,096 characters".into());
            }
            let raw=api.json(reqwest::Method::PATCH,"/studio/native/router/responsibility",Some(&json!({"sessionKey":key,"expectedRevision":revision,"responsibility":text}))).await?;
            crate::worker::compute(move || {
                serde_json::from_value(raw)
                    .map(Reply::Responsibility)
                    .map_err(|_| "Invalid saved responsibility".into())
            })
            .await
        }
        Request::Targets(backends) => {
            if backends.len() > 64 {
                return Err("Too many backend catalogs".into());
            }
            let mut loads = stream::iter(backends.into_iter().map(|backend| async move {
                let id = backend.id.clone();
                (id, load_targets(api, backend).await)
            }))
            .buffer_unordered(2);
            let mut targets = Vec::new();
            let mut issues = Vec::new();
            while let Some((backend, result)) = loads.next().await {
                match result {
                    Ok((rows, partial)) => {
                        targets.extend(rows);
                        if targets.len() > 2048 {
                            targets.truncate(2048);
                            issues.push(
                                "Partial target catalog: reached 2,048 sessions across backends"
                                    .into(),
                            );
                            break;
                        }
                        if let Some(partial) = partial {
                            issues.push(format!("{backend}: {partial}"));
                        }
                    }
                    Err(error) => issues.push(format!("{backend}: {error}")),
                }
            }
            crate::worker::compute(move || {
                targets.sort_by(|a, b| {
                    a.backend
                        .cmp(&b.backend)
                        .then_with(|| a.thread.title.cmp(&b.thread.title))
                        .then_with(|| a.thread.id.cmp(&b.thread.id))
                });
                targets.dedup_by(|a, b| a.key() == b.key());
                Ok(Reply::Targets {
                    targets: Arc::new(targets),
                    issues,
                })
            })
            .await
        }
    }
}
fn parse_snapshot(raw: Value) -> Result<Snapshot, String> {
    let snapshot: Snapshot =
        serde_json::from_value(raw).map_err(|_| "Invalid Router settings response")?;
    if snapshot.fallbacks.len() > 3
        || snapshot.controllers.len() > 64
        || snapshot.openings.len() > 4096
    {
        return Err("Router settings exceed the native inventory budget".into());
    }
    Ok(snapshot)
}
async fn load_targets(
    api: &Api,
    backend: Backend,
) -> Result<(Vec<Target>, Option<String>), String> {
    let mut output = Vec::new();
    let mut cursor: Option<String> = None;
    let mut cursors = HashSet::new();
    for page in 0..10 {
        let decoded = if backend.kind == "codex" {
            let body = json!({"backend":backend.id,"operation":"catalog","cursor":cursor});
            let mut attempt = 0;
            let raw = loop {
                match api
                    .json(
                        reqwest::Method::POST,
                        "/studio/native/conversation/read",
                        Some(&body),
                    )
                    .await
                {
                    Err(error) if error.contains("still initializing") && attempt < 10 => {
                        attempt += 1;
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                    other => break other?,
                }
            };
            crate::worker::compute(move || {
                let mut raw = raw;
                if let Some(rows) = raw["data"].as_array_mut() {
                    rows.retain(|row| row["ephemeral"] != true && row["archived"] != true);
                }
                crate::agents::Page::parse(&raw)
            })
            .await?
        } else if backend.kind == "opencode" {
            let raw = crate::opencode::catalog(api, cursor.as_deref()).await?;
            crate::worker::compute(move || crate::agents::Page::parse(&raw)).await?
        } else {
            continue;
        };
        for agent in decoded.agents {
            if agent.thread.parent.is_none() {
                output.push(Target {
                    backend: backend.id.clone(),
                    thread: agent.thread.clone(),
                });
            }
        }
        let Some(next) = decoded.cursor else {
            return Ok((output, None));
        };
        if !cursors.insert(next.clone()) {
            return Ok((
                output,
                Some("Partial target catalog: repeated cursor".into()),
            ));
        }
        cursor = Some(next);
        if page == 9 {
            return Ok((
                output,
                Some("Partial target catalog: reached 1,000 sessions / 10 pages".into()),
            ));
        }
    }
    Ok((output, None))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn authenticated_catalog_reads_follow_opaque_pages_and_exclude_ephemeral_and_child_sessions(
    ) {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            time::Duration,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for page in 0..2 {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut bytes = Vec::new();
                let boundary = loop {
                    let mut block = [0u8; 1024];
                    let count = socket.read(&mut block).unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&block[..count]);
                    if let Some(index) = bytes.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                        break index + 4;
                    }
                };
                let headers = String::from_utf8_lossy(&bytes[..boundary]).to_lowercase();
                assert!(headers.starts_with("post /studio/native/conversation/read http/1.1"));
                assert!(headers.contains("authorization: bearer 0123456789abcdef0123456789abcdef"));
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("content-length:")
                            .and_then(|value| value.trim().parse::<usize>().ok())
                    })
                    .unwrap();
                while bytes.len() < boundary + length {
                    let mut block = [0u8; 1024];
                    let count = socket.read(&mut block).unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&block[..count]);
                }
                let request: Value =
                    serde_json::from_slice(&bytes[boundary..boundary + length]).unwrap();
                assert_eq!(request["operation"], "catalog");
                assert_eq!(request["backend"], "codex");
                if page == 0 {
                    assert!(request["cursor"].is_null());
                } else {
                    assert_eq!(request["cursor"], "next:中文/+?");
                }
                let response = if page == 0 {
                    json!({"data":[{"id":"ordinary","cwd":"/one","name":"One"},{"id":"ephemeral","cwd":"/two","ephemeral":true},{"id":"child","cwd":"/three","parentThreadId":"ordinary"}],"nextCursor":"next:中文/+?"})
                } else {
                    json!({"data":[{"id":"second","cwd":"/second","name":"Two"}],"nextCursor":null})
                };
                let body = serde_json::to_vec(&response).unwrap();
                write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).unwrap();
                socket.write_all(&body).unwrap();
            }
        });
        let api = Api::from_access_url(&format!(
            "http://{address}/#token=0123456789abcdef0123456789abcdef"
        ))
        .unwrap();
        let reply = execute(
            &api,
            Request::Targets(vec![Backend {
                id: "codex".into(),
                name: "Codex".into(),
                kind: "codex".into(),
                socket_path: "/ws/codex".into(),
            }]),
        )
        .await
        .unwrap();
        server.join().unwrap();
        let Reply::Targets { targets, issues } = reply else {
            panic!("No catalog projection")
        };
        assert!(issues.is_empty());
        assert_eq!(
            targets
                .iter()
                .map(|target| target.thread.id.as_str())
                .collect::<Vec<_>>(),
            ["ordinary", "second"]
        );
    }
    #[test]
    fn fallback_order_conditions_and_duplicates_are_preserved_and_validated() {
        let settings = Settings {
            backend: "codex".into(),
            fallbacks: vec![
                Fallback {
                    session_key: "codex:one".into(),
                    condition: "First 中文".into(),
                },
                Fallback {
                    session_key: "opencode:two".into(),
                    condition: "Second".into(),
                },
            ],
        };
        settings.validate().unwrap();
        let mut duplicate = settings.clone();
        duplicate.fallbacks.push(settings.fallbacks[0].clone());
        assert!(duplicate.validate().unwrap_err().contains("twice"));
        let mut empty = settings;
        empty.fallbacks[0].condition = "  ".into();
        assert!(empty.validate().is_err());
    }
}
