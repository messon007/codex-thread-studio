//! Presentation-neutral workspace requests, decoded/indexed before GTK sees them.
use crate::gateway::Api;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use studio_markdown_native::text::TextDocument;

#[derive(Debug)]
pub enum Request {
    SourceEditPage {
        document: TextDocument,
        line: usize,
        offset: Option<usize>,
        previous: bool,
    },
    SourceSave {
        snapshot: Text,
        page: studio_markdown_native::text_edit::Page,
        operations: Vec<studio_markdown_native::text_edit::Mutation>,
        edited: String,
        overwrite: bool,
    },
    SourceSearch {
        document: TextDocument,
        query: String,
        case_sensitive: bool,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    },
    SourceEditSearch {
        document: TextDocument,
        page: studio_markdown_native::text_edit::Page,
        edited: String,
        query: String,
        case_sensitive: bool,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    },
    ReviewFilter {
        files: Arc<[ChangedFile]>,
        commits: Arc<[Commit]>,
        mode: crate::git_review::Mode,
        query: String,
    },
    Compare {
        left: String,
        right: String,
    },
    CompareChange {
        id: String,
        index: usize,
    },
    List {
        root: String,
        path: String,
        query: String,
        show_hidden: bool,
        show_ignored: bool,
        cursor: Option<String>,
    },
    Roots {
        root: String,
    },
    Location {
        root: String,
        path: String,
    },
    Read {
        root: String,
        path: String,
    },
    ReadComment {
        root: String,
        path: String,
        query: studio_markdown_native::quote::Query,
    },
    Status {
        root: String,
    },
    History {
        root: String,
        offset: usize,
        previous: Arc<[Commit]>,
    },
    Commit {
        root: String,
        commit: String,
    },
    Diff {
        root: String,
        path: String,
        scope: String,
        commit: Option<String>,
    },
    Stage {
        root: String,
        path: String,
        stage: bool,
    },
}

impl Request {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::SourceEditPage { .. } => "source-edit-page",
            Self::SourceSave { .. } => "source-save",
            Self::SourceSearch { .. } => "source-search",
            Self::SourceEditSearch { .. } => "source-search",
            Self::ReviewFilter { .. } => "review-filter",
            Self::Compare { .. } => "compare",
            Self::CompareChange { .. } => "compare-change",
            Self::List { .. } => "list",
            Self::Roots { .. } => "roots",
            Self::Location { .. } => "location",
            Self::Read { .. } => "read",
            Self::ReadComment { .. } => "read-comment",
            Self::Status { .. } => "status",
            Self::History { .. } => "history",
            Self::Commit { .. } => "commit",
            Self::Diff { .. } => "diff",
            Self::Stage { .. } => "stage",
        }
    }
}

#[derive(Debug)]
pub struct Response {
    pub panel: u8,
    pub generation: u64,
    pub serial: u64,
    pub kind: &'static str,
    pub result: Result<Reply, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub size: Option<u64>,
    #[serde(default)]
    pub read_only: bool,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub path: String,
    pub directory: bool,
    pub read_only: bool,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Directory {
    pub path: String,
    pub entries: Vec<Entry>,
    pub scan_truncated: bool,
    pub next_cursor: Option<String>,
    pub total: usize,
    pub elapsed_ms: u128,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub path: String,
    #[serde(default)]
    pub previous_path: Option<String>,
    #[serde(default)]
    pub staged: bool,
    #[serde(default)]
    pub unstaged: bool,
    #[serde(default)]
    pub untracked: bool,
    #[serde(default)]
    pub conflicted: bool,
    #[serde(default)]
    pub status: String,
}
#[derive(Debug, Deserialize)]
pub struct Status {
    pub branch: String,
    pub files: Vec<ChangedFile>,
    pub truncated: bool,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub hash: String,
    pub short_hash: String,
    pub author_name: String,
    #[serde(default)]
    pub author_email: String,
    pub authored_at: String,
    pub subject: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct History {
    pub commits: Arc<[Commit]>,
    pub has_more: bool,
}

#[derive(Clone, Debug)]
pub struct FileIdentity {
    pub root: String,
    pub path: String,
    pub hash: String,
    pub read_only: bool,
}

#[derive(Clone, Debug)]
pub struct Text {
    pub path: String,
    pub language: String,
    pub document: TextDocument,
    pub preview: Option<Arc<studio_markdown_native::structured::Preview>>,
    pub html: Option<Arc<crate::html_preview::Preview>>,
    pub diff: Option<studio_markdown_native::unified_diff::Index>,
    pub binary: bool,
    pub truncated: bool,
    pub file: Option<FileIdentity>,
}

#[derive(Debug)]
pub enum Reply {
    SourceEditPage(studio_markdown_native::text_edit::Page),
    SourceSaved {
        text: Text,
        page: studio_markdown_native::text_edit::Page,
    },
    SourceConflict,
    SourceMatches(crate::source_search::Matches),
    SourceEditedMatches(crate::source_search::EditMatches),
    ReviewRows(crate::git_review::Rows),
    Comparison(Comparison),
    Change(Change),
    Directory(Directory),
    Roots(Vec<Entry>),
    Location(Location),
    Text(Text),
    CommentText(Text, studio_markdown_native::quote::Located),
    Status(Status),
    History(History),
    Commit(Vec<ChangedFile>, bool),
    Staged,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    pub id: String,
    pub changes: usize,
    pub added: usize,
    pub removed: usize,
    pub left_hash: String,
    pub right_hash: String,
    pub bounded_alignment: bool,
    pub elapsed_ms: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub index: usize,
    pub left_line: usize,
    pub right_line: usize,
    pub left_changed: Vec<usize>,
    pub right_changed: Vec<usize>,
}

pub async fn execute(api: &Api, request: Request) -> Result<Reply, String> {
    if let Request::ReadComment { query, .. } = &request {
        query.validate()?;
    }
    let request = match request {
        Request::SourceEditSearch {
            document,
            page,
            edited,
            query,
            case_sensitive,
            cancelled,
        } => {
            return crate::worker::compute(move || {
                crate::source_search::find_edit(
                    &document,
                    &page,
                    &edited,
                    &query,
                    case_sensitive,
                    &cancelled,
                )
                .map(Reply::SourceEditedMatches)
            })
            .await;
        }
        Request::SourceEditPage {
            document,
            line,
            offset,
            previous,
        } => {
            return crate::worker::compute(move || {
                studio_markdown_native::text_edit::page(&document, line, offset, previous)
                    .map(Reply::SourceEditPage)
            })
            .await;
        }
        Request::SourceSave {
            snapshot,
            page,
            operations,
            edited,
            overwrite,
        } => {
            let file = snapshot
                .file
                .clone()
                .filter(|f| !f.read_only)
                .ok_or("Source file is read-only")?;
            if snapshot.binary || snapshot.truncated || snapshot.diff.is_some() {
                return Err("Only complete source files can be edited".into());
            }
            let (document, page) = crate::worker::compute(move || {
                studio_markdown_native::text_edit::compose(
                    &snapshot.document,
                    &page,
                    &operations,
                    &edited,
                )
            })
            .await?;
            let body = crate::worker::compute({
                let document = document.clone(); let file = file.clone();
                move || Ok(json!({"root":file.root,"path":file.path,"content":document.as_str(),"expectedHash":file.hash,"overwrite":overwrite}))
            }).await?;
            let (status, raw) = api
                .json_status(reqwest::Method::POST, "/studio/workspace/save", &body)
                .await?;
            if status == reqwest::StatusCode::CONFLICT
                && raw["error"]["code"] == "workspace_file_conflict"
            {
                return Ok(Reply::SourceConflict);
            }
            if !status.is_success() {
                return Err(crate::gateway::response_error(status, &raw));
            }
            return crate::worker::compute(move || {
                let Reply::Text(text) = decode_response(
                    Request::Read {
                        root: file.root.clone(),
                        path: file.path.clone(),
                    },
                    raw,
                )?
                else {
                    return Err("Invalid saved source response".into());
                };
                if text.document.as_str() != document.as_str()
                    || !text
                        .file
                        .as_ref()
                        .is_some_and(|f| f.path == file.path && f.root == file.root && !f.read_only)
                {
                    return Err("Saved source changed again; local edits remain in memory".into());
                }
                Ok(Reply::SourceSaved { text, page })
            })
            .await;
        }
        Request::SourceSearch {
            document,
            query,
            case_sensitive,
            cancelled,
        } => {
            return crate::worker::compute(move || {
                crate::source_search::find(&document, &query, case_sensitive, &cancelled)
                    .map(Reply::SourceMatches)
            })
            .await;
        }
        Request::ReviewFilter {
            files,
            commits,
            mode,
            query,
        } => {
            return crate::worker::compute(move || {
                crate::git_review::project(&files, &commits, mode, &query).map(Reply::ReviewRows)
            })
            .await;
        }
        request => request,
    };
    if let Request::CompareChange { id, index } = &request {
        let raw = api
            .json(
                reqwest::Method::GET,
                &format!("/studio/native/compare/{id}/{index}"),
                None,
            )
            .await?;
        return crate::worker::compute(move || decode_response(request, raw)).await;
    }
    let (path, body) = match &request {
        Request::ReviewFilter { .. }
        | Request::SourceSearch { .. }
        | Request::SourceEditSearch { .. }
        | Request::SourceEditPage { .. }
        | Request::SourceSave { .. } => unreachable!(),
        Request::Compare { left, right } => {
            ("/studio/native/compare", json!({"left":left,"right":right}))
        }
        Request::CompareChange { .. } => unreachable!(),
        Request::Roots { root } => ("/studio/native/files/roots", json!({"root":root})),
        Request::Location { root, path } => (
            "/studio/native/files/location",
            json!({"root":root,"path":path}),
        ),
        Request::List {
            root,
            path,
            query,
            show_hidden,
            show_ignored,
            cursor,
        } => (
            "/studio/native/files",
            json!({"root":root,"path":path,"query":query,"showHidden":show_hidden,"showIgnored":show_ignored,"cursor":cursor}),
        ),
        Request::Read { root, path } | Request::ReadComment { root, path, .. } => {
            ("/studio/review-file", json!({"root":root,"path":path}))
        }
        Request::Status { root } => ("/studio/git/status", json!({"root":root})),
        Request::History { root, offset, .. } => (
            "/studio/git/history",
            json!({"root":root,"offset":offset,"limit":100}),
        ),
        Request::Commit { root, commit } => {
            ("/studio/git/commit", json!({"root":root,"commit":commit}))
        }
        Request::Diff {
            root,
            path,
            scope,
            commit,
        } => {
            if let Some(commit) = commit {
                (
                    "/studio/git/commit-diff",
                    json!({"root":root,"path":path,"commit":commit}),
                )
            } else {
                (
                    "/studio/git/diff",
                    json!({"root":root,"path":path,"scope":scope}),
                )
            }
        }
        Request::Stage { root, path, stage } => (
            if *stage {
                "/studio/git/stage"
            } else {
                "/studio/git/unstage"
            },
            json!({"root":root,"paths":[path]}),
        ),
    };
    let raw = if matches!(
        &request,
        Request::List { .. } | Request::Roots { .. } | Request::Location { .. }
    ) {
        let mut attempt = 0;
        loop {
            let (status, raw) = api.json_status(reqwest::Method::POST, path, &body).await?;
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS && attempt < 5 {
                tokio::time::sleep(std::time::Duration::from_millis(100u64 << attempt)).await;
                attempt += 1;
                continue;
            }
            if !status.is_success() {
                return Err(raw["error"]["message"]
                    .as_str()
                    .unwrap_or("Directory request failed")
                    .into());
            }
            break raw;
        }
    } else {
        api.json(reqwest::Method::POST, path, Some(&body)).await?
    };
    crate::worker::compute(move || decode_response(request, raw)).await
}

fn decode_response(request: Request, mut raw: Value) -> Result<Reply, String> {
    let comment = match &request {
        Request::ReadComment { query, .. } => Some(query.clone()),
        _ => None,
    };
    let is_diff = matches!(&request, Request::Diff { .. });
    match request {
        Request::ReviewFilter { .. }
        | Request::SourceSearch { .. }
        | Request::SourceEditSearch { .. }
        | Request::SourceEditPage { .. }
        | Request::SourceSave { .. } => unreachable!(),
        Request::Compare { .. } => Ok(Reply::Comparison(decode(raw)?)),
        Request::CompareChange { .. } => Ok(Reply::Change(decode(raw)?)),
        Request::List { .. } => Ok(Reply::Directory(decode(raw)?)),
        Request::Roots { .. } => Ok(Reply::Roots(decode(raw["roots"].take())?)),
        Request::Location { .. } => Ok(Reply::Location(decode(raw)?)),
        Request::Status { .. } => Ok(Reply::Status(decode(raw)?)),
        Request::History { previous, .. } => {
            let mut history: History = decode(raw)?;
            if !previous.is_empty() {
                let mut commits = previous.to_vec();
                let mut hashes = commits
                    .iter()
                    .map(|c| c.hash.clone())
                    .collect::<std::collections::HashSet<_>>();
                commits.extend(
                    history
                        .commits
                        .iter()
                        .filter(|c| hashes.insert(c.hash.clone()))
                        .cloned(),
                );
                history.commits = commits.into();
            }
            Ok(Reply::History(history))
        }
        Request::Commit { .. } => Ok(Reply::Commit(
            decode(raw["files"].take())?,
            raw["truncated"].as_bool().unwrap_or(false),
        )),
        Request::Read { path, .. }
        | Request::ReadComment { path, .. }
        | Request::Diff { path, .. } => {
            let content = match raw["content"].take() {
                Value::String(content) => content,
                _ => return Err("Text response has no content".into()),
            };
            let language = raw["language"].as_str().unwrap_or("diff").to_owned();
            let binary = raw["binary"].as_bool().unwrap_or(false);
            let diff = (is_diff && !binary)
                .then(|| studio_markdown_native::unified_diff::Index::new(&content));
            let file = (!is_diff)
                .then(|| {
                    let root = raw["root"].as_str()?;
                    let path = raw["path"].as_str()?;
                    let hash = raw["hash"].as_str()?;
                    if root.is_empty()
                        || path.is_empty()
                        || hash.is_empty()
                        || root.len() > 4096
                        || path.len() > 4096
                        || hash.len() > 128
                        || root.contains('\0')
                        || path.contains('\0')
                        || hash.contains('\0')
                    {
                        return None;
                    }
                    Some(FileIdentity {
                        root: root.into(),
                        path: path.into(),
                        hash: hash.into(),
                        read_only: raw["readOnly"].as_bool().unwrap_or(true),
                    })
                })
                .flatten();
            let document = TextDocument::new(content)?;
            let preview = (!binary && !is_diff && file.is_some() && raw["truncated"] != true)
                .then(|| studio_markdown_native::structured::Preview::prepare(&path, &document))
                .flatten()
                .map(Arc::new);
            let html = (!binary && !is_diff && file.is_some() && raw["truncated"] != true)
                .then(|| crate::html_preview::Preview::prepare(&path, &document))
                .flatten()
                .map(Arc::new);
            let text = Text {
                path,
                language,
                diff,
                document,
                preview,
                html,
                binary,
                truncated: raw["truncated"].as_bool().unwrap_or(false),
                file,
            };
            if let Some(mut query) = comment {
                if text.binary || text.truncated {
                    return Err("Comments require a complete text snapshot".into());
                }
                let identity = text
                    .file
                    .as_ref()
                    .ok_or("Text snapshot identity is missing")?;
                query.anchor.text_kind = studio_markdown_native::quote::Kind::Source;
                let mut locator =
                    studio_markdown_native::quote::Locator::new(query, &identity.hash)?;
                locator
                    .scan_literal(text.document.as_str().as_bytes(), text.document.bytes())
                    .map_err(|e| e.to_string())?;
                Ok(Reply::CommentText(text, locator.finish()))
            } else {
                Ok(Reply::Text(text))
            }
        }
        Request::Stage { .. } => Ok(Reply::Staged),
    }
}

fn decode<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|error| format!("Invalid workspace response: {error}"))
}

#[cfg(test)]
mod review_tests {
    use super::*;
    fn comment_request(text: &str) -> Request {
        Request::ReadComment {
            root: "/project".into(),
            path: "code.rs".into(),
            query: studio_markdown_native::quote::Query {
                quote: text.into(),
                anchor: studio_markdown_native::quote::Anchor {
                    base_hash: "old".into(),
                    text_kind: studio_markdown_native::quote::Kind::Preview,
                    ..Default::default()
                },
            },
        }
    }
    fn file_response(content: &str) -> Value {
        json!({"root":"/project","path":"/project/code.rs","hash":"new","content":content,"language":"rust"})
    }
    #[test]
    fn structured_preparation_keeps_source_identity_and_excludes_nonfile_payloads() {
        let request = || Request::Read {
            root: "/project".into(),
            path: "config.json5".into(),
        };
        let content = "{/*keep*/ key:'中文😀',n:1e+4,n:0xFF}";
        let mut raw = file_response(content);
        raw["path"] = json!("/project/config.json5");
        let Reply::Text(text) = decode_response(request(), raw.clone()).unwrap() else {
            panic!("Wrong reply")
        };
        assert_eq!(text.document.as_str(), content);
        assert_eq!(text.file.as_ref().unwrap().hash, "new");
        assert!(text.preview.as_ref().unwrap().formatted);
        for field in ["binary", "truncated"] {
            let mut incomplete = raw.clone();
            incomplete[field] = json!(true);
            let Reply::Text(text) = decode_response(request(), incomplete).unwrap() else {
                panic!("Wrong reply")
            };
            assert!(text.preview.is_none());
        }
        let Reply::Text(text) = decode_response(request(), json!({"content":content})).unwrap()
        else {
            panic!("Wrong reply")
        };
        assert!(text.preview.is_none());
    }
    #[test]
    fn html_preparation_requires_an_identified_complete_file_and_preserves_original() {
        let request = || Request::Read {
            root: "/project".into(),
            path: "index.html".into(),
        };
        let content = "<h1>Original &amp; 中文😀</h1><script>unsafe()</script>\r\n";
        let mut raw = file_response(content);
        raw["path"] = json!("/project/index.html");
        let Reply::Text(text) = decode_response(request(), raw.clone()).unwrap() else {
            panic!("Wrong reply")
        };
        assert_eq!(text.document.as_str(), content);
        assert!(text
            .html
            .as_ref()
            .unwrap()
            .body
            .as_ref()
            .unwrap()
            .contains("Original &amp; 中文😀"));
        assert!(!text
            .html
            .as_ref()
            .unwrap()
            .body
            .as_ref()
            .unwrap()
            .contains("unsafe()"));
        assert_eq!(text.file.as_ref().unwrap().hash, "new");
        for field in ["binary", "truncated"] {
            let mut raw = raw.clone();
            raw[field] = json!(true);
            let Reply::Text(text) = decode_response(request(), raw).unwrap() else {
                panic!("Wrong reply")
            };
            assert!(text.html.is_none());
        }
        let Reply::Text(text) = decode_response(request(), json!({"content":content})).unwrap()
        else {
            panic!("Wrong reply")
        };
        assert!(text.html.is_none());
        let Reply::Text(text) = decode_response(
            Request::Diff {
                root: "/project".into(),
                path: "index.html".into(),
                scope: "working".into(),
                commit: None,
            },
            raw,
        )
        .unwrap() else {
            panic!("Wrong reply")
        };
        assert!(text.html.is_none());
    }
    #[tokio::test]
    async fn saving_rejects_unidentified_readonly_and_incomplete_snapshots_before_http() {
        let api =
            Api::from_access_url("http://127.0.0.1:9/#token=0123456789abcdef0123456789abcdef")
                .unwrap();
        let mut raw = file_response("before\n");
        raw["readOnly"] = json!(false);
        let Reply::Text(snapshot) = decode_response(
            Request::Read {
                root: "/project".into(),
                path: "code.rs".into(),
            },
            raw,
        )
        .unwrap() else {
            panic!("Wrong reply")
        };
        let page =
            studio_markdown_native::text_edit::page(&snapshot.document, 1, None, false).unwrap();
        for variant in 0..5 {
            let mut text = snapshot.clone();
            match variant {
                0 => text.file = None,
                1 => text.file.as_mut().unwrap().read_only = true,
                2 => text.binary = true,
                3 => text.truncated = true,
                _ => text.diff = Some(studio_markdown_native::unified_diff::Index::new("+diff\n")),
            }
            let error = execute(
                &api,
                Request::SourceSave {
                    snapshot: text,
                    page: page.clone(),
                    operations: vec![],
                    edited: "before\n".into(),
                    overwrite: true,
                },
            )
            .await
            .unwrap_err();
            assert!(
                error.contains(if variant < 2 {
                    "read-only"
                } else {
                    "complete source"
                }),
                "{error}"
            );
        }
    }
    #[test]
    fn source_comments_keep_canonical_identity_and_literal_unicode_offsets() {
        let source = "// Intro\r\n\r\nlet target = \"中文😀\";  \r\n";
        let Reply::CommentText(text, located) = decode_response(
            comment_request("let target = \"中文😀\";  "),
            file_response(source),
        )
        .unwrap() else {
            panic!("Wrong reply")
        };
        assert_eq!(text.file.unwrap().path, "/project/code.rs");
        assert_eq!(
            located.status,
            studio_markdown_native::quote::Status::Unique
        );
        let p = located.position.unwrap();
        assert_eq!((p.line, p.end_line), (3, 3));
        assert_eq!(
            &text.document.as_str()[p.start..p.end],
            "let target = \"中文😀\";  "
        );
    }
    #[test]
    fn ambiguous_and_incomplete_source_snapshots_cannot_claim_a_verified_quote() {
        let Reply::CommentText(_, located) =
            decode_response(comment_request("same"), file_response("same\nsame\n")).unwrap()
        else {
            panic!("Wrong reply")
        };
        assert_eq!(
            located.status,
            studio_markdown_native::quote::Status::Ambiguous
        );
        assert!(located.position.is_none());
        for field in ["binary", "truncated"] {
            let mut raw = file_response("same");
            raw[field] = json!(true);
            assert!(decode_response(comment_request("same"), raw).is_err());
        }
        let mut raw = file_response("same");
        raw.as_object_mut().unwrap().remove("hash");
        assert!(decode_response(comment_request("same"), raw).is_err());
    }
    #[test]
    fn source_comment_offsets_reach_beyond_the_first_window_of_a_long_line() {
        let text = format!("{}needle 中文😀", "x".repeat(300_000));
        let Reply::CommentText(text, located) =
            decode_response(comment_request("needle 中文😀"), file_response(&text)).unwrap()
        else {
            panic!("Wrong reply")
        };
        let p = located.position.unwrap();
        assert_eq!((p.line, p.start), (1, 300_000));
        assert!(text
            .document
            .window(p.line, Some(p.start), false)
            .unwrap()
            .text
            .starts_with("needle 中文😀"));
    }
    #[test]
    fn history_pages_merge_on_the_worker_and_preserve_shared_author_data() {
        let entry = |hash: &str| json!({"hash":hash,"shortHash":hash,"authorName":"Literal <Author>","authorEmail":"mail@example.invalid","subject":"中文", "authoredAt":"2026-10-25"});
        let previous: Arc<[Commit]> = serde_json::from_value(json!([entry("first")])).unwrap();
        let reply = decode_response(
            Request::History {
                root: "/project".into(),
                offset: 1,
                previous: previous.clone(),
            },
            json!({"commits":[entry("first"),entry("second")], "hasMore":true}),
        )
        .unwrap();
        let Reply::History(history) = reply else {
            panic!("Wrong reply")
        };
        assert_eq!(
            history
                .commits
                .iter()
                .map(|c| c.hash.as_str())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
        assert_eq!(history.commits[1].author_email, "mail@example.invalid");
        assert_eq!(previous.len(), 1);
    }
    #[test]
    fn diff_origins_are_prepared_without_changing_the_source_or_binary_response() {
        let request = || Request::Diff {
            root: "/project".into(),
            path: "literal .md".into(),
            scope: "unstaged".into(),
            commit: None,
        };
        let source = "@@ -2 +7 @@\r\n-old 中文\r\n+new 😀\r\n";
        let Reply::Text(text) = decode_response(request(), json!({"content":source})).unwrap()
        else {
            panic!("Wrong reply")
        };
        assert_eq!(text.document.window(1, None, false).unwrap().text, source);
        let index = text.diff.unwrap();
        assert_eq!(index.line(2).old, 2);
        assert_eq!(index.line(3).new, 7);
        let Reply::Text(binary) = decode_response(
            request(),
            json!({"content":"Binary files differ", "binary":true}),
        )
        .unwrap() else {
            panic!("Wrong reply")
        };
        assert!(binary.diff.is_none() && binary.binary);
        for language in [json!("diff"), Value::Null] {
            let Reply::Text(file) = decode_response(
                Request::Read {
                    root: "/project".into(),
                    path: "source.patch".into(),
                },
                json!({"content":source,"language":language}),
            )
            .unwrap() else {
                panic!("Wrong reply")
            };
            assert!(
                file.diff.is_none(),
                "Source syntax must not replace physical file line numbers"
            );
            assert_eq!(file.document.window(1, None, false).unwrap().text, source);
        }
    }
}
