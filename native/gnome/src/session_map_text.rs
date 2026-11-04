//! Private Map suffixes are consumed on workers, before any presentation snapshot.
use crate::session_map::types::MAX_MAP_BODY_BYTES;
pub use crate::session_map_protocol::{parse, visible, Update, END, START};
use serde_json::Value;
use std::collections::HashMap;
#[cfg(test)]
#[path = "session_map_text_tests.rs"]
mod tests;
#[derive(Debug)]
pub enum Notice {
    Started {
        thread: String,
        turn: String,
    },
    Message {
        thread: String,
        turn: String,
        update: Option<Result<Update, String>>,
    },
    Completed {
        thread: String,
        turn: String,
        completed: bool,
    },
}
#[derive(Default)]
struct Stream {
    prefix: String,
    suffix: String,
    hidden: bool,
    overflow: bool,
    order: u64,
}
#[derive(Default)]
pub struct Streams {
    items: HashMap<(String, String, String), Stream>,
    bytes: usize,
    sequence: u64,
    epoch: Option<u64>,
}
impl Streams {
    /// The tiny undecided marker prefix and at most 4 MiB of private metadata are
    /// retained. Full visible replies never accumulate in this filter.
    pub fn process(&mut self, payload: &mut Value) -> Vec<Notice> {
        let p = &payload["params"];
        let method = payload["method"].as_str().unwrap_or("");
        if method == "studio/appServer/status" {
            let epoch = p["generation"].as_u64();
            let changed = self.epoch.is_some() && epoch.is_some() && self.epoch != epoch;
            if changed || matches!(p["state"].as_str(), Some("starting" | "error" | "stopped")) {
                self.items.clear();
                self.bytes = 0;
            }
            if epoch.is_some() {
                self.epoch = epoch;
            }
            return Vec::new();
        }
        let thread = p["threadId"].as_str().unwrap_or("").to_owned();
        let turn = p["turnId"]
            .as_str()
            .or(p.pointer("/turn/id").and_then(Value::as_str))
            .unwrap_or("")
            .to_owned();
        let mut notices = Vec::new();
        if thread.is_empty() || thread.len() > 256 || turn.is_empty() || turn.len() > 256 {
            return notices;
        }
        match method {
            "turn/started" => notices.push(Notice::Started { thread, turn }),
            "item/agentMessage/delta" => {
                let id = p["itemId"].as_str().unwrap_or("").to_owned();
                if id.is_empty() || id.len() > 256 {
                    payload["params"]["delta"] = "".into();
                    return notices;
                }
                let key = (thread, turn, id);
                if !self.items.contains_key(&key) && self.items.len() >= 128 {
                    payload["params"]["delta"] = "".into();
                    return notices;
                }
                if !self.items.contains_key(&key) {
                    self.sequence += 1;
                    self.items.insert(
                        key.clone(),
                        Stream {
                            order: self.sequence,
                            ..Default::default()
                        },
                    );
                }
                let stream = self.items.get_mut(&key).unwrap();
                let delta = p["delta"].as_str().unwrap_or("");
                let joined = if stream.hidden {
                    String::new()
                } else {
                    let mut s = std::mem::take(&mut stream.prefix);
                    s.push_str(delta);
                    s
                };
                let (visible, metadata) = if stream.hidden {
                    ("", delta)
                } else if let Some(index) = joined.find(START) {
                    stream.hidden = true;
                    (&joined[..index], &joined[index..])
                } else {
                    let prefix = (1..START.len())
                        .rev()
                        .find(|&n| joined.ends_with(&START[..n]))
                        .unwrap_or(0);
                    stream.prefix = joined[joined.len() - prefix..].into();
                    (&joined[..joined.len() - prefix], "")
                };
                if !metadata.is_empty() && !stream.overflow {
                    if stream.suffix.len() + metadata.len()
                        > MAX_MAP_BODY_BYTES + START.len() + END.len()
                        || self.bytes + metadata.len() > 4 * 1024 * 1024
                    {
                        stream.overflow = true;
                        self.bytes -= stream.suffix.len();
                        stream.suffix.clear();
                    } else {
                        stream.suffix.push_str(metadata);
                        self.bytes += metadata.len();
                    }
                }
                payload["params"]["delta"] = visible.into();
            }
            "item/completed" if p["item"]["type"] == "agentMessage" => {
                let id = p["item"]["id"].as_str().unwrap_or("").to_owned();
                let stream = self.items.remove(&(thread.clone(), turn.clone(), id));
                let update = if let Some(text) = p["item"]["text"].as_str() {
                    parse(text)
                } else {
                    stream.as_ref().and_then(|s| {
                        if s.overflow {
                            Some(Err("Map update exceeds its byte budget".into()))
                        } else {
                            parse(&s.suffix)
                        }
                    })
                };
                if let Some(s) = stream {
                    self.bytes -= s.suffix.len();
                }
                notices.push(Notice::Message {
                    thread,
                    turn,
                    update,
                });
            }
            "turn/completed" => {
                // Some providers deliver only deltas plus a completion event.
                let mut keys = self
                    .items
                    .keys()
                    .filter(|(t, u, _)| t == &thread && u == &turn)
                    .cloned()
                    .collect::<Vec<_>>();
                keys.sort_by_key(|key| self.items[key].order);
                for key in keys {
                    if let Some(s) = self.items.remove(&key) {
                        self.bytes -= s.suffix.len();
                        if s.hidden || s.overflow {
                            notices.push(Notice::Message {
                                thread: thread.clone(),
                                turn: turn.clone(),
                                update: if s.overflow {
                                    Some(Err("Map update exceeds its byte budget".into()))
                                } else {
                                    parse(&s.suffix)
                                },
                            });
                        }
                    }
                }
                for item in p["turn"]["items"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|v| v["type"] == "agentMessage")
                {
                    notices.push(Notice::Message {
                        thread: thread.clone(),
                        turn: turn.clone(),
                        update: item["text"].as_str().and_then(parse),
                    });
                }
                notices.push(Notice::Completed {
                    thread,
                    turn,
                    completed: p["turn"]["status"] == "completed",
                });
            }
            _ => {}
        }
        // Turn summaries can carry full items even when no item/completed event
        // was emitted. The typed observations above own the private suffix.
        for item in payload
            .pointer_mut("/params/turn/items")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            if item["type"] == "agentMessage" {
                if let Some(Value::String(text)) = item.get_mut("text") {
                    text.truncate(visible(text).len());
                }
            }
        }
        notices
    }
}
