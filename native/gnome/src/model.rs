use serde::Deserialize;
use serde_json::Value;

pub const MAX_VISIBLE_ITEMS: usize = 400;
pub const MAX_VISIBLE_BYTES: usize = 32 * 1024 * 1024;
const MAX_ITEM_BYTES: usize = 128 * 1024;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Backend {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub socket_path: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Thread {
    pub id: String,
    pub parent: Option<String>,
    pub title: String,
    pub cwd: String,
    pub pinned: bool,
    pub activity: String,
}

impl Thread {
    pub fn from_value(value: &Value) -> Option<Self> {
        let id = value.get("id")?.as_str()?.to_owned();
        if id.is_empty()
            || id.len() > 256
            || text(value, "cwd").len() > 4096
            || value["parentThreadId"]
                .as_str()
                .is_some_and(|parent| parent.len() > 256)
        {
            return None;
        }
        let title = value
            .get("name")
            .or_else(|| value.get("title"))
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                value
                    .get("agentNickname")
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
            })
            .or_else(|| value.get("preview").and_then(Value::as_str))
            .unwrap_or("New session");
        Some(Self {
            parent: value["parentThreadId"]
                .as_str()
                .filter(|parent| !parent.is_empty())
                .map(str::to_owned),
            pinned: false,
            activity: value["status"]
                .as_str()
                .or(value.pointer("/status/type").and_then(Value::as_str))
                .unwrap_or("")
                .chars()
                .filter(|c| !c.is_control())
                .take(64)
                .collect(),
            id,
            title: title
                .lines()
                .next()
                .unwrap_or("New session")
                .chars()
                .take(120)
                .collect(),
            cwd: text(value, "cwd").to_owned(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin {
    pub backend: String,
    pub thread: String,
    pub title: String,
    pub root: String,
    pub turn: String,
    pub item: String,
    pub question: std::sync::Arc<String>,
    pub excerpt: bool,
}
#[derive(Clone, Debug)]
pub struct Item {
    pub output: Option<crate::conversation_output::Reference>,
    pub details: Option<crate::conversation_output::Reference>,
    pub status: Option<crate::activity_details::Status>,
    pub truncated: bool,
    pub images: Vec<crate::conversation_images::Image>,
    pub origin: Option<std::sync::Arc<Origin>>,
    pub id: String,
    pub turn_id: String,
    pub role: String,
    pub body: String,
    pub markup: Option<String>,
    pub rich: Option<std::sync::Arc<crate::chat_content::Content>>,
}

impl PartialEq for Item {
    fn eq(&self, other: &Self) -> bool {
        self.images == other.images
            && self.output == other.output
            && self.details == other.details
            && self.status == other.status
            && self.truncated == other.truncated
            && self.origin == other.origin
            && self.id == other.id
            && self.turn_id == other.turn_id
            && self.role == other.role
            && self.body == other.body
            && self.markup == other.markup
            && self.rich.as_ref().map(|rich| rich.source_bytes)
                == other.rich.as_ref().map(|rich| rich.source_bytes)
    }
}

impl Item {
    pub fn from_value(value: &Value) -> Self {
        Self::project(value, None)
    }
    pub fn from_value_near(value: &Value, needle: &str) -> Self {
        Self::project(value, Some(needle))
    }
    fn project(value: &Value, needle: Option<&str>) -> Self {
        let (role, mut body) = Self::source_content(value);
        let partial = value["_studioTruncated"] == true;
        let details = crate::activity_details::snapshot(value, partial);
        let status = details
            .as_ref()
            .and_then(|_| crate::activity_details::status(value));
        let source_bytes = body.len();
        let mut truncated = body.len() > MAX_ITEM_BYTES || partial;
        let mut output = None;
        if body.len() > MAX_ITEM_BYTES {
            let mut preview = if let Some(found) = needle
                .filter(|needle| !needle.is_empty())
                .and_then(|needle| body.find(needle))
            {
                let mut start = found.saturating_sub(48 * 1024);
                while !body.is_char_boundary(start) {
                    start += 1;
                }
                let mut end = (start + 110 * 1024).min(body.len());
                while !body.is_char_boundary(end) {
                    end -= 1;
                }
                format!(
                    "[Excerpt around the selected match]\n\n{}",
                    &body[start..end]
                )
            } else {
                let mut end = MAX_ITEM_BYTES;
                while !body.is_char_boundary(end) {
                    end -= 1;
                }
                format!("{}\n… Output truncated in the native view.", &body[..end])
            };
            preview.shrink_to_fit();
            let full = std::mem::replace(&mut body, preview);
            if !partial {
                output = Some(crate::conversation_output::retain(full));
            }
        }
        if partial {
            output=Some(crate::conversation_output::Reference{token:String::new(),bytes:source_bytes,unavailable:Some("This live user-message preview is bounded. Reload the conversation or its history page to retrieve the full message.".into())});
        }
        if body.contains('\0') {
            let mut display = body.replace('\0', "␀");
            if display.len() > MAX_ITEM_BYTES {
                let mut end = MAX_ITEM_BYTES;
                while !display.is_char_boundary(end) {
                    end -= 1;
                }
                display.truncate(end);
                display.push_str("\n… Output truncated in the native view.");
                display.shrink_to_fit();
                truncated = true;
            }
            let original = std::mem::replace(&mut body, display);
            if output.is_none() {
                output = Some(crate::conversation_output::retain(original));
            }
        }
        Self {
            output,
            details,
            status,
            truncated,
            images: crate::conversation_images::project(value),
            origin: None,
            id: text(value, "id").to_owned(),
            turn_id: text(value, "turnId").to_owned(),
            role: role.into(),
            body,
            markup: None,
            rich: None,
        }
    }

    /// Extract full source on a worker; views apply their own presentation bound.
    pub fn source_content(value: &Value) -> (&'static str, String) {
        let kind = text(value, "type");
        let (role, body) = match kind {
            "userMessage" => (
                "You",
                value
                    .get("content")
                    .and_then(Value::as_array)
                    .map(|parts| {
                        parts
                            .iter()
                            .filter_map(|part| {
                                part.get("text").and_then(Value::as_str).or_else(|| {
                                    match text(part, "type") {
                                        "image" | "localImage" | "nativeImage" => {
                                            Some("[Attached image]")
                                        }
                                        "file" => Some("[Attached file]"),
                                        _ => None,
                                    }
                                })
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default(),
            ),
            "agentMessage" => (
                "Assistant",
                crate::session_map_text::visible(text(value, "text")).to_owned(),
            ),
            "plan" => ("Plan", crate::activity_details::plan(value)),
            "reasoning" => ("Reasoning", crate::activity_details::reasoning(value)),
            "commandExecution" => (
                "Command",
                format!(
                    "$ {}\n{}",
                    crate::activity_details::command(value),
                    text(value, "aggregatedOutput")
                ),
            ),
            "fileChange" => (
                "Files",
                value
                    .get("changes")
                    .and_then(Value::as_array)
                    .map(|changes| {
                        changes
                            .iter()
                            .map(|v| format!("{}\n{}", text(v, "path"), text(v, "diff")))
                            .collect::<Vec<_>>()
                            .join("\n\n")
                    })
                    .unwrap_or_default(),
            ),
            "mcpToolCall" => (
                "Tool",
                format!(
                    "{} / {}\n{}",
                    text(value, "server"),
                    text(value, "tool"),
                    value.get("result").unwrap_or(&Value::Null)
                ),
            ),
            _ => (
                "Activity",
                serde_json::to_string_pretty(value).unwrap_or_default(),
            ),
        };
        (role, body)
    }

    pub fn append(&mut self, delta: &str) {
        if delta.is_empty() {
            return;
        }
        // Deltas are intentionally not accumulated in the GTK model after the
        // preview bound. A complete provider item supplies a fresh snapshot.
        self.output = None;
        self.markup = None;
        // A coalesced Rust worker replaces this snapshot while text streams.
        // Keep its visible blocks until the next snapshot is ready.
        let mut take = delta
            .len()
            .min(MAX_ITEM_BYTES.saturating_sub(self.body.len()));
        while !delta.is_char_boundary(take) {
            take -= 1;
        }
        let accepted = &delta[..take];
        let display = if accepted.contains('\0') {
            std::borrow::Cow::Owned(accepted.replace('\0', "␀"))
        } else {
            std::borrow::Cow::Borrowed(accepted)
        };
        let mut show = display
            .len()
            .min(MAX_ITEM_BYTES.saturating_sub(self.body.len()));
        while !display.is_char_boundary(show) {
            show -= 1;
        }
        self.body.push_str(&display[..show]);
        self.truncated |= take < delta.len() || show < display.len();
        if self.truncated
            && !self
                .body
                .ends_with("… Output truncated in the native view.")
        {
            self.body
                .push_str("\n… Output truncated in the native view.");
            self.body.shrink_to_fit();
        }
    }

    pub fn weight(&self) -> usize {
        std::mem::size_of::<Self>()
            + self
                .output
                .as_ref()
                .map_or(0, crate::conversation_output::Reference::weight)
            + self
                .details
                .as_ref()
                .map_or(0, crate::conversation_output::Reference::weight)
            + self.status.as_ref().map_or(0, |s| s.text.capacity())
            + self
                .images
                .iter()
                .map(crate::conversation_images::Image::weight)
                .sum::<usize>()
            + self.id.capacity()
            + self.turn_id.capacity()
            + self.role.capacity()
            + self.body.capacity()
            + self.markup.as_ref().map_or(0, String::capacity)
            + self.origin.as_ref().map_or(0, |origin| {
                // Conservative accounting: shared provenance can be counted more
                // than once, but must not escape transcript/cache byte budgets.
                std::mem::size_of::<Origin>()
                    + origin.backend.capacity()
                    + origin.thread.capacity()
                    + origin.title.capacity()
                    + origin.root.capacity()
                    + origin.turn.capacity()
                    + origin.item.capacity()
                    + origin.question.capacity()
            })
            + self.rich.as_ref().map_or(0, |rich| {
                rich.bytes + std::mem::size_of::<crate::chat_content::Content>()
            })
    }
}

pub fn trim_items(items: &mut Vec<std::sync::Arc<Item>>) {
    let mut bytes = 0usize;
    let mut retained = 0;
    for item in items.iter().rev().take(MAX_VISIBLE_ITEMS) {
        let next = bytes.saturating_add(item.weight());
        if next > MAX_VISIBLE_BYTES && retained > 0 {
            break;
        }
        bytes = next;
        retained += 1;
    }
    if retained < items.len() {
        items.drain(..items.len() - retained);
    }
}

pub fn history(thread: &Value) -> Vec<Item> {
    // Select the bounded tail first, then project in display order so source-cache
    // recency favors the newest attachments rather than the oldest visited turn.
    let mut selected = Vec::with_capacity(MAX_VISIBLE_ITEMS);
    if let Some(turns) = thread["turns"].as_array() {
        'history: for turn in turns.iter().rev() {
            if let Some(items) = turn["items"].as_array() {
                for item in items.iter().rev() {
                    if selected.len() == MAX_VISIBLE_ITEMS {
                        break 'history;
                    }
                    selected.push((item, text(turn, "id")));
                }
            }
        }
    }
    selected
        .into_iter()
        .rev()
        .map(|(value, turn)| {
            let mut item = Item::from_value(value);
            if item.turn_id.is_empty() {
                item.turn_id = turn.into();
            }
            item
        })
        .collect()
}

pub fn active_turn(thread: &Value) -> Option<String> {
    thread
        .get("turns")?
        .as_array()?
        .iter()
        .rev()
        .find(|turn| text(turn, "status") == "inProgress")?
        .get("id")?
        .as_str()
        .map(str::to_owned)
}

pub fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn history_preserves_roles_and_detects_active_turn() {
        let thread = json!({"turns":[{"id":"t","status":"inProgress","items":[
            {"id":"u","type":"userMessage","content":[{"type":"text","text":"Hello"}]},
            {"id":"a","type":"agentMessage","text":"你好"}]}]});
        let items = history(&thread);
        assert_eq!(items[0].role, "You");
        assert_eq!(items[1].body, "你好");
        assert_eq!(active_turn(&thread).as_deref(), Some("t"));
        assert!(items.iter().all(|item| item.turn_id == "t"));
    }
    #[test]
    fn long_output_is_bounded_on_unicode_boundaries() {
        let item = Item::from_value(
            &json!({"id":"a","type":"agentMessage","text":"字".repeat(MAX_ITEM_BYTES)}),
        );
        assert!(item.body.len() < MAX_ITEM_BYTES + 100);
        assert!(item.body.capacity() < MAX_ITEM_BYTES + 1024);
        assert!(item.body.ends_with("native view."));
        assert!(
            item.truncated
                && item
                    .output
                    .as_ref()
                    .is_some_and(|s| s.unavailable.is_none())
        );
    }
    #[test]
    fn streaming_at_the_exact_preview_bound_never_allocates_a_large_delta() {
        let mut item = Item::from_value(
            &json!({"id":"stream","type":"agentMessage","text":"x".repeat(MAX_ITEM_BYTES)}),
        );
        assert!(!item.truncated);
        item.append(&"字😀".repeat(MAX_ITEM_BYTES));
        assert!(item.truncated);
        assert!(item.output.is_none());
        assert!(item.body.len() < MAX_ITEM_BYTES + 100);
        assert!(item.body.capacity() < MAX_ITEM_BYTES + 1024);
        let complete = Item::from_value(
            &json!({"id":"stream","type":"agentMessage","text":"字😀".repeat(MAX_ITEM_BYTES)}),
        );
        assert!(complete.output.is_some());
    }
    #[test]
    fn nul_display_is_safe_and_full_source_can_still_be_exported_exactly() {
        let original = "hello\0中文😀\r\n";
        let item = Item::from_value(&json!({"id":"control","type":"agentMessage","text":original}));
        assert!(!item.truncated);
        assert!(!item.body.contains('\0'));
        assert!(item.body.contains('␀'));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source.txt");
        crate::conversation_output::export(item.output.unwrap(), path.clone()).unwrap();
        assert_eq!(std::fs::read(path).unwrap(), original.as_bytes());
        let item = Item::from_value(
            &json!({"id":"controls","type":"agentMessage","text":"\0".repeat(MAX_ITEM_BYTES)}),
        );
        assert!(item.truncated);
        assert!(item.body.len() < MAX_ITEM_BYTES + 100);
    }
    #[test]
    fn transcript_budget_includes_rich_blocks_and_keeps_the_newest_message() {
        let mut item = Item::from_value(
            &json!({"id":"large","type":"agentMessage","text":"x".repeat(MAX_ITEM_BYTES - 100)}),
        );
        item.rich = Some(crate::chat_content::Content::parse(&item.body).unwrap());
        assert!(item.weight() > item.body.len() * 2);
        let mut items = vec![std::sync::Arc::new(item); MAX_VISIBLE_ITEMS];
        items.push(std::sync::Arc::new(Item::from_value(
            &json!({"id":"newest","type":"agentMessage","text":"done"}),
        )));
        trim_items(&mut items);
        assert!(items.len() < MAX_VISIBLE_ITEMS);
        assert!(items.iter().map(|item| item.weight()).sum::<usize>() <= MAX_VISIBLE_BYTES);
        assert_eq!(items.last().unwrap().id, "newest");
    }
}
