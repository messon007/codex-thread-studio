//! Latest-turn resource extraction and filtering on the bounded Rust CPU pool.
//! Paths are classified without I/O. Opening still requires the file service's
//! project/shared-root authorization, including its symlink checks.
use crate::{model::Item, session_search::Context};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::{
    collections::HashMap,
    ops::Range,
    path::Path,
    sync::{Arc, OnceLock},
};

const ITEM_BYTES: usize = 1024 * 1024;
const TURN_BYTES: usize = 4 * ITEM_BYTES;
const ITEM_OCCURRENCES: usize = 100;
const MAX_OCCURRENCES: usize = 2000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Filter {
    #[default]
    All,
    Web,
    File,
    Code,
}
impl Filter {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Web => "Web",
            Self::File => "Files",
            Self::Code => "Code",
        }
    }
}
#[derive(Clone, Debug)]
pub struct Occurrence {
    pub item: Arc<Item>,
    pub excerpt: String,
    pub source: Range<usize>,
}
#[derive(Clone, Debug)]
pub struct Resource {
    pub address: String,
    pub title: String,
    pub kind: Filter,
    pub directory: bool,
    pub blocked: Option<&'static str>,
    pub occurrences: Vec<Occurrence>,
    order: usize,
    search: String,
}
#[derive(Debug, Default)]
pub struct Index {
    pub resources: Vec<Arc<Resource>>,
    pub truncated: bool,
    pub occurrences: usize,
}
#[derive(Clone, Debug)]
pub struct Request {
    pub context: Context,
    pub root: String,
    pub items: Vec<Arc<Item>>,
    pub serial: u64,
    pub force: bool,
    pub query: String,
    pub filter: Filter,
}
#[derive(Debug)]
pub struct Response {
    pub context: Context,
    pub serial: u64,
    pub result: Result<View, String>,
}
#[derive(Debug)]
pub struct View {
    pub index: Arc<Index>,
    pub rows: Vec<Arc<Resource>>,
}

/// Cheap GTK-side snapshot: retain only latest-turn immutable handles, never text.
pub fn latest(items: &[Arc<Item>]) -> Vec<Arc<Item>> {
    let Some(turn) = items
        .iter()
        .rev()
        .find(|i| !i.turn_id.is_empty())
        .map(|i| &i.turn_id)
    else {
        return Vec::new();
    };
    items
        .iter()
        .filter(|i| &i.turn_id == turn && matches!(i.role.as_str(), "You" | "Assistant" | "Plan"))
        .cloned()
        .collect()
}
pub fn same_items(a: &[Arc<Item>], b: &[Arc<Item>]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| Arc::ptr_eq(a, b))
}
pub fn filtered(index: Arc<Index>, query: &str, filter: Filter) -> View {
    let query = query.trim().to_lowercase();
    let rows = index
        .resources
        .iter()
        .filter(|r| {
            (filter == Filter::All || r.kind == filter)
                && (query.is_empty() || r.search.contains(&query))
        })
        .cloned()
        .collect();
    View { index, rows }
}

pub async fn run(
    api: crate::gateway::Api,
    mut input: tokio::sync::watch::Receiver<Option<Request>>,
    events: async_channel::Sender<crate::protocol::Event>,
) {
    let mut cached: Option<(Request, Arc<Index>)> = None;
    while input.changed().await.is_ok() {
        let Some(request) = input.borrow_and_update().clone() else {
            cached = None;
            continue;
        };
        let reuse = cached
            .as_ref()
            .filter(|(old, _)| {
                !request.force
                    && old.context == request.context
                    && old.root == request.root
                    && same_items(&old.items, &request.items)
            })
            .map(|(_, index)| index.clone());
        let context = request.context.clone();
        let serial = request.serial;
        let result = async {
            let index = if let Some(index) = reuse {
                index
            } else {
                let shared = crate::shared_directories::execute(
                    &api,
                    crate::shared_directories::Request::Read,
                )
                .await?;
                let work = request.clone();
                let index = crate::worker::compute(move || {
                    Ok(Arc::new(build(
                        &work.items,
                        &work.root,
                        &shared.directories,
                    )))
                })
                .await?;
                cached = Some((request.clone(), index.clone()));
                index
            };
            crate::worker::compute(move || Ok(filtered(index, &request.query, request.filter)))
                .await
        }
        .await;
        // Serial execution coalesces pending watch requests without starting an
        // unbounded set of uncancellable spawn_blocking jobs during streaming.
        if !input.has_changed().unwrap_or(true)
            && events
                .send(crate::protocol::Event::SessionResources(Response {
                    context,
                    serial,
                    result,
                }))
                .await
                .is_err()
        {
            break;
        }
    }
}

pub fn build(items: &[Arc<Item>], root: &str, shared: &[String]) -> Index {
    let mut resources: Vec<Resource> = Vec::new();
    let mut identities = HashMap::<String, usize>::new();
    let mut budget = TURN_BYTES;
    let mut index = Index::default();
    for item in latest(items) {
        if budget == 0 || index.occurrences >= MAX_OCCURRENCES {
            index.truncated = true;
            break;
        }
        let limit = ITEM_BYTES.min(budget);
        let (text, partial) = if let Some(reference) = &item.output {
            crate::conversation_output::prefix(reference, limit)
                .unwrap_or_else(|_| (bounded(&item.body, limit).to_owned(), true))
        } else {
            (
                bounded(&item.body, limit).to_owned(),
                item.truncated || item.body.len() > limit,
            )
        };
        budget = budget.saturating_sub(text.len());
        index.truncated |= partial;
        let mut code = false;
        let mut link = 0usize;
        let mut occurrences = 0;
        for (event, range) in Parser::new_ext(
            &text,
            Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH,
        )
        .into_offset_iter()
        {
            if occurrences >= ITEM_OCCURRENCES || index.occurrences >= MAX_OCCURRENCES {
                index.truncated = true;
                break;
            }
            let mut candidates = Vec::new();
            match event {
                Event::Start(Tag::CodeBlock(_)) => code = true,
                Event::End(TagEnd::CodeBlock) => code = false,
                Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) => {
                    link += 1;
                    candidates.push((dest_url.into_string(), range.clone()));
                }
                Event::End(TagEnd::Link | TagEnd::Image) => link = link.saturating_sub(1),
                Event::Code(value) if !code => {
                    if looks_like_path(&value, false) {
                        candidates.push((value.into_string(), range.clone()));
                    }
                }
                Event::Text(_) if !code && link == 0 => {
                    // Match original bytes, not entity-decoded display offsets.
                    let source = &text[range.clone()];
                    let mut urls = Vec::new();
                    for found in urls_pattern().find_iter(source) {
                        let address = trim(found.as_str());
                        let start = range.start + found.start();
                        urls.push(found.range());
                        candidates.push((address.to_owned(), start..start + address.len()));
                        if candidates.len() >= ITEM_OCCURRENCES {
                            break;
                        }
                    }
                    for found in paths_pattern().find_iter(source) {
                        if urls
                            .iter()
                            .any(|r| r.start < found.end() && found.start() < r.end)
                        {
                            continue;
                        }
                        let address = trim(found.as_str());
                        if looks_like_path(address, true) {
                            let start = range.start + found.start();
                            candidates.push((address.to_owned(), start..start + address.len()));
                        }
                        if candidates.len() >= ITEM_OCCURRENCES {
                            break;
                        }
                    }
                }
                _ => {}
            }
            for (address, range) in candidates {
                if occurrences >= ITEM_OCCURRENCES || index.occurrences >= MAX_OCCURRENCES {
                    index.truncated = true;
                    break;
                }
                let Some((identity, mut resource)) = normalize(
                    &address,
                    item.origin.as_ref().map_or(root, |o| o.root.as_str()),
                    shared,
                ) else {
                    continue;
                };
                let snippet = excerpt(&text, range.clone());
                let occurrence = Occurrence {
                    item: item.clone(),
                    excerpt: snippet,
                    source: range,
                };
                if let Some(&position) = identities.get(&identity) {
                    let existing = &mut resources[position];
                    existing.address = resource.address;
                    existing.search = resource.search;
                    existing.order = index.occurrences;
                    existing.occurrences.push(occurrence);
                } else {
                    identities.insert(identity, resources.len());
                    resource.order = index.occurrences;
                    resource.occurrences.push(occurrence);
                    resources.push(resource);
                }
                occurrences += 1;
                index.occurrences += 1;
            }
        }
    }
    resources.sort_by_key(|resource| std::cmp::Reverse(resource.order));
    index.resources = resources.into_iter().map(Arc::new).collect();
    index
}
fn bounded(text: &str, limit: usize) -> &str {
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}
fn excerpt(text: &str, range: Range<usize>) -> String {
    let mut start = range.start.saturating_sub(70);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    let mut end = range.end.saturating_add(90).min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[start..end]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(240)
        .collect()
}
fn urls_pattern() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r#"(?i)https?://[^\s<>\"'`]+"#).unwrap())
}
fn paths_pattern() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r"(?:\.{0,2}/)?(?:[\p{L}\p{N}_.@+\-]+/)+[\p{L}\p{N}_.@+\-]+(?:/|:\d+(?::\d+)?)?",
        )
        .unwrap()
    })
}
fn trim(text: &str) -> &str {
    text.trim_end_matches(['.', ',', ';', ')', ']', '}', '，', '。', '；'])
}
fn looks_like_path(text: &str, strong: bool) -> bool {
    if text.contains("://") {
        return false;
    }
    let path = text
        .split('#')
        .next()
        .unwrap_or(text)
        .split(':')
        .next()
        .unwrap_or(text);
    if path.is_empty() || path.chars().any(char::is_control) {
        return false;
    }
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let known = matches!(
        ext.to_ascii_lowercase().as_str(),
        "md" | "markdown"
            | "mdown"
            | "mdx"
            | "txt"
            | "csv"
            | "tsv"
            | "xlsx"
            | "json"
            | "jsonl"
            | "yaml"
            | "yml"
            | "toml"
            | "xml"
            | "html"
            | "htm"
            | "pdf"
            | "epub"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "svg"
            | "rs"
            | "py"
            | "js"
            | "ts"
            | "tsx"
            | "jsx"
            | "mjs"
            | "cjs"
            | "c"
            | "h"
            | "cpp"
            | "hpp"
            | "go"
            | "java"
            | "sh"
            | "sql"
            | "log"
            | "ini"
            | "cfg"
            | "conf"
            | "lock"
            | "patch"
            | "diff"
            | "wasm"
            | "mp4"
            | "webm"
            | "mp3"
            | "wav"
    );
    known
        || path.ends_with('/')
        || !strong
            && (path.contains('/')
                || matches!(path, "Makefile" | "Dockerfile" | "LICENSE" | "Justfile"))
}
fn normalize(address: &str, root: &str, shared: &[String]) -> Option<(String, Resource)> {
    if address.len() > 4096 || address.is_empty() || address.chars().any(char::is_control) {
        return None;
    }
    let credentials = url::Url::parse(address).ok().filter(|url| {
        matches!(url.scheme(), "http" | "https")
            && (!url.username().is_empty() || url.password().is_some())
    });
    let target = if let Some(url) = credentials {
        crate::links::Target::External(url.into())
    } else {
        crate::links::resolve(address, Path::new(root), None).ok()?
    };
    let (identity, title, kind, directory, blocked) = match target {
        crate::links::Target::Heading(_) => return None,
        crate::links::Target::External(url) => {
            let mut parsed = url::Url::parse(&url).ok()?;
            let blocked = (!parsed.username().is_empty() || parsed.password().is_some())
                .then_some("The URL contains a username or password");
            let title = parsed.host_str()?.to_owned();
            parsed.set_fragment(None);
            (format!("web:{parsed}"), title, Filter::Web, false, blocked)
        }
        crate::links::Target::Document {
            path,
            line,
            heading,
        } => {
            if !looks_like_path(address, false) && path.extension().is_none() {
                return None;
            }
            let directory = address.ends_with('/');
            let traversal = path
                .components()
                .any(|c| c == std::path::Component::ParentDir);
            let inside = !root.is_empty() && path.starts_with(root)
                || !directory && shared.iter().any(|base| path.starts_with(base));
            let blocked =
                (traversal || !inside).then_some("The path is outside the current session root");
            let title = path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            (
                format!("file:{}:{line:?}:{heading:?}", path.display()),
                title,
                if line.is_some() {
                    Filter::Code
                } else {
                    Filter::File
                },
                directory,
                blocked,
            )
        }
    };
    let decoded = percent_encoding::percent_decode_str(address).decode_utf8_lossy();
    let search = format!("{title}\n{address}\n{decoded}").to_lowercase();
    Some((
        identity,
        Resource {
            address: address.into(),
            title,
            kind,
            directory,
            blocked,
            occurrences: Vec::new(),
            order: 0,
            search,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(turn: &str, id: &str, kind: &str, text: &str) -> Arc<Item> {
        let content = serde_json::json!({"id":id,"turnId":turn,"type":kind,"text":text,"content":[{"text":text}]});
        Arc::new(Item::from_value(&content))
    }
    #[test]
    fn latest_narratives_merge_reference_links_and_skip_tool_and_fenced_text() {
        let items=vec![item("old","old","agentMessage","[Old](old.md)"),item("new","q","userMessage","`docs/中文.md:12:2`"),item("new","a","agentMessage","[Read][d]\n\n[d]: docs/中文.md:12:2\n\nhttps://EXAMPLE.com:443/help#one https://example.com/help#two\n\n```\nignored/tool.rs\n```\n\n![Image](assets/a%20b.png)"),item("new","tool","commandExecution","secret/tool.rs")];
        let index = build(&items, "/project", &[]);
        assert_eq!(index.resources.len(), 3);
        assert_eq!(index.occurrences, 5);
        let code = index
            .resources
            .iter()
            .find(|r| r.kind == Filter::Code)
            .unwrap();
        assert_eq!(code.occurrences.len(), 2);
        assert_eq!(code.occurrences[0].item.id, "q");
        assert!(index.resources.iter().all(|r| r.blocked.is_none()));
        assert_eq!(
            filtered(Arc::new(index), "中文", Filter::Code).rows.len(),
            1
        );
    }
    #[test]
    fn untrusted_paths_credentials_and_schemes_do_not_gain_authority() {
        let inputs="[Traversal](../private/key.md) [Outside](/other/key.md) [Shared](/shared/docs/a.md) [Boundary](/shared/docs-other/a.md) [Unsafe](javascript:alert) [Password](https://u:p@example.com/a) [Loopback](http://127.0.0.1:38669/project/README.md:1)";
        let index = build(
            &[item("t", "a", "agentMessage", inputs)],
            "/project",
            &["/shared/docs".into()],
        );
        assert_eq!(index.resources.len(), 6);
        assert_eq!(
            index
                .resources
                .iter()
                .filter(|r| r.blocked.is_some())
                .count(),
            4
        );
        assert!(index
            .resources
            .iter()
            .any(|r| r.kind == Filter::Code && r.blocked.is_none()));
        assert_eq!(
            filtered(Arc::new(index), "SHARED", Filter::File).rows.len(),
            2
        );
    }
    #[test]
    fn output_snapshot_and_unicode_limits_are_explicit() {
        let text = format!("{}\n[End](docs/end.md)", "中".repeat(50_000));
        let index = build(&[item("t", "a", "agentMessage", &text)], "/project", &[]);
        assert_eq!(index.resources.len(), 1);
        assert!(!index.truncated);
        let text = (0..130)
            .map(|i| format!("[File](docs/{i}.md)\n"))
            .collect::<String>();
        let index = build(&[item("t", "a", "agentMessage", &text)], "/project", &[]);
        assert_eq!(index.resources.len(), 100);
        assert!(index.truncated);
        let giant = format!("{}[Past budget](last.md)", "中".repeat(ITEM_BYTES));
        let index = build(&[item("t", "a", "agentMessage", &giant)], "/project", &[]);
        assert!(index.resources.is_empty());
        assert!(index.truncated);
    }
    #[test]
    fn most_recent_occurrence_and_loopback_credentials_keep_their_meaning() {
        let index = build(
            &[item(
                "t",
                "p",
                "plan",
                "[First](a.md) [Second](b.md) [Again](a.md)",
            )],
            "/project",
            &[],
        );
        assert_eq!(index.resources[0].address, "a.md");
        assert_eq!(index.resources[0].occurrences.len(), 2);
        let (_, resource) = normalize(
            "http://user:secret@127.0.0.1:38669/project/a.md",
            "/project",
            &[],
        )
        .unwrap();
        assert_eq!(resource.kind, Filter::Web);
        assert!(resource.blocked.is_some());
        let index = build(
            &[item(
                "t",
                "a",
                "agentMessage",
                "[Read](docs/%E4%B8%AD%E6%96%87.md)",
            )],
            "/project",
            &[],
        );
        assert_eq!(
            filtered(Arc::new(index), "中文.md", Filter::File)
                .rows
                .len(),
            1
        );
        let index = Arc::new(build(
            &[item(
                "t",
                "a",
                "agentMessage",
                "[Read](docs/%E4%B8%AD%E6%96%87/readme.md)",
            )],
            "/home/openresource/project",
            &[],
        ));
        assert_eq!(filtered(index.clone(), "中文", Filter::File).rows.len(), 1);
        assert!(filtered(index, "openresource", Filter::File)
            .rows
            .is_empty());
    }
}
