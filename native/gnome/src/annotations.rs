//! Shared annotation records and prompt construction, without GTK or another database.
use crate::gateway::Api;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Capture {
    pub excerpt: String,
    pub source: Value,
}
/// Rendered selections span independent GTK blocks, not the browser's UTF-16
/// textContent coordinate space. Preserve exact quotes/identities; leave the
/// optional legacy offsets unset rather than inventing a character position.
pub fn chat_capture(
    item: &crate::model::Item,
    owner: &str,
    selection: Option<String>,
) -> Result<Capture, &'static str> {
    let excerpt = selection
        .filter(|s| !s.trim().is_empty() && s.len() <= 16384)
        .ok_or("Select a passage of at most 16 KiB from one chat item before commenting")?;
    let (key, turn, id) = if let Some(origin) = &item.origin {
        (
            format!("{}:{}", origin.backend, origin.thread),
            origin.turn.as_str(),
            origin.item.as_str(),
        )
    } else {
        (owner.to_owned(), item.turn_id.as_str(), item.id.as_str())
    };
    let identifier = |s: &str, limit| {
        !s.is_empty()
            && s.encode_utf16().take(limit + 1).count() <= limit
            && !s.chars().any(char::is_control)
    };
    if !identifier(&key, 320)
        || !key
            .split_once(':')
            .is_some_and(|(b, t)| !b.is_empty() && !t.is_empty())
        || !identifier(turn, 256)
        || !identifier(id, 256)
    {
        return Err("This reply has no usable persistent comment identity");
    }
    Ok(Capture {
        excerpt,
        source: json!({"provider":"chat","version":1,"anchor":{"sessionKey":key,"turnId":turn,"itemId":id,"startOffset":null,"endOffset":null}}),
    })
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EpubAnchor {
    pub root: String,
    pub file_path: String,
    pub book_hash: String,
    pub cfi_range: String,
    pub href: String,
    pub chapter_label: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfAnchor {
    #[serde(default)]
    pub root: String,
    pub file_path: String,
    #[serde(default)]
    pub document_hash: String,
    pub page: usize,
    #[serde(default)]
    pub rects: Vec<crate::pdf::Rect>,
    #[serde(default)]
    pub char_range: Option<[usize; 2]>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableAnchor {
    #[serde(default)]
    pub root: String,
    pub file_path: String,
    #[serde(default)]
    pub document_hash: String,
    pub sheet: String,
    pub range: String,
    #[serde(default)]
    pub header: String,
    #[serde(default)]
    pub sql: Option<String>,
    #[serde(default)]
    pub delimiter: Option<String>,
}
pub fn table_cell(input: &str) -> Result<(usize, usize), String> {
    if input.len() > 128 {
        return Err("Invalid table cell address".into());
    }
    let address = input.to_ascii_uppercase().replace('$', "");
    let end = address.bytes().take_while(u8::is_ascii_alphabetic).count();
    if end == 0 || end == address.len() {
        return Err("Select a single A1-style cell address".into());
    }
    let mut column = 0usize;
    for c in address[..end].bytes() {
        column = column
            .checked_mul(26)
            .and_then(|n| n.checked_add((c - b'A' + 1) as usize))
            .ok_or("Invalid table column")?;
        if column > 200 {
            return Err("Table cell is beyond the 200-column preview".into());
        }
    }
    if !address[end..].bytes().all(|c| c.is_ascii_digit()) {
        return Err("Invalid table row".into());
    }
    let row = address[end..]
        .parse::<usize>()
        .map_err(|_| "Invalid table row")?;
    if row == 0 || row > 10000 {
        return Err("Table cell is beyond the 10,000-row preview".into());
    }
    Ok((row - 1, column - 1))
}
impl TableAnchor {
    pub fn validate(&self) -> Result<(), String> {
        if self.file_path.is_empty()
            || self.file_path.len() > 4096
            || self.root.len() > 4096
            || self.file_path.contains('\0')
            || self.root.contains('\0')
            || self.document_hash.len() > 128
            || self.sheet.len() > 1024
            || self.sheet.contains('\0')
            || self.header.len() > 512
            || self
                .sql
                .as_ref()
                .is_some_and(|s| s.len() > 16384 || s.contains('\0'))
            || self
                .delimiter
                .as_deref()
                .is_some_and(|d| !matches!(d, "," | ";" | "\t"))
        {
            return Err("Invalid or oversized table comment anchor".into());
        }
        table_cell(&self.range)?;
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub enum Reopen {
    Document {
        root: String,
        path: String,
        anchor: studio_markdown_native::quote::Anchor,
    },
    Epub(EpubAnchor),
    Pdf(PdfAnchor),
    Table(TableAnchor),
    Browser(String),
}
pub fn reopen(source: &Value) -> Result<Reopen, String> {
    if source["version"].as_u64().unwrap_or(1) != 1 {
        return Err("This comment source version is not supported yet".into());
    }
    let anchor = &source["anchor"];
    match source["provider"].as_str() {
        Some("document") => {
            let root = anchor["root"].as_str().unwrap_or("");
            let path = anchor["filePath"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or("Comment file path is missing")?;
            if root.len() > 4096 || path.len() > 4096 || root.contains('\0') || path.contains('\0')
            {
                return Err("Invalid comment file path".into());
            }
            let line = anchor["startLine"]
                .as_u64()
                .filter(|n| *n > 0)
                .and_then(|n| usize::try_from(n).ok());
            Ok(Reopen::Document {
                root: root.into(),
                path: path.into(),
                anchor: studio_markdown_native::quote::Anchor {
                    base_hash: anchor["baseHash"].as_str().unwrap_or("").to_owned(),
                    start_line: line,
                    end_line: anchor["endLine"]
                        .as_u64()
                        .and_then(|n| usize::try_from(n).ok()),
                    text_kind: match anchor["textKind"].as_str() {
                        Some("source") => studio_markdown_native::quote::Kind::Source,
                        Some("preview") => studio_markdown_native::quote::Kind::Preview,
                        Some("flow") => studio_markdown_native::quote::Kind::Flow,
                        _ => studio_markdown_native::quote::Kind::Auto,
                    },
                },
            })
        }
        Some("epub") => {
            let anchor: EpubAnchor = serde_json::from_value(anchor.clone())
                .map_err(|_| "Invalid EPUB comment source")?;
            if anchor.file_path.is_empty()
                || anchor.file_path.len() > 4096
                || anchor.root.len() > 4096
                || anchor.file_path.contains('\0')
                || anchor.root.contains('\0')
                || anchor.book_hash.len() > 128
                || anchor.cfi_range.len() > 8192
                || anchor.href.len() > 8192
                || anchor.chapter_label.len() > 4096
            {
                return Err("EPUB comment source exceeds its bounds".into());
            }
            if !anchor.cfi_range.is_empty()
                && (crate::epub::cfi_chapter(&anchor.cfi_range).is_none()
                    || !anchor.cfi_range.ends_with(')'))
            {
                return Err("Invalid EPUB comment CFI".into());
            }
            if !anchor.href.is_empty() {
                let base = url::Url::parse("studio-epub://book/").unwrap();
                let target = base
                    .join(&anchor.href)
                    .map_err(|_| "Invalid EPUB chapter reference")?;
                if target.scheme() != base.scheme()
                    || target.host_str() != base.host_str()
                    || target.query().is_some()
                    || !target.username().is_empty()
                    || target.password().is_some()
                {
                    return Err("Invalid EPUB chapter reference".into());
                }
            }
            Ok(Reopen::Epub(anchor))
        }
        Some("browser") => {
            let input = anchor["url"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or("Comment URL is missing")?;
            if input.len() > 8192 {
                return Err("Comment URL is too long".into());
            }
            let url = url::Url::parse(input).map_err(|_| "Invalid comment URL")?;
            if !matches!(url.scheme(), "http" | "https")
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
            {
                return Err("Only HTTP and HTTPS comment sources can be opened".into());
            }
            Ok(Reopen::Browser(url.to_string()))
        }
        Some("table") => {
            let anchor: TableAnchor = serde_json::from_value(anchor.clone())
                .map_err(|_| "Invalid table comment source")?;
            anchor.validate()?;
            Ok(Reopen::Table(anchor))
        }
        Some("pdf") => {
            let anchor: PdfAnchor =
                serde_json::from_value(anchor.clone()).map_err(|_| "Invalid PDF comment source")?;
            if anchor.root.len() > 4096
                || anchor.root.contains('\0')
                || anchor.file_path.is_empty()
                || anchor.file_path.len() > 4096
                || anchor.file_path.contains('\0')
                || anchor.document_hash.len() > 128
                || anchor.page == 0
                || anchor.page > i32::MAX as usize
                || anchor.rects.len() > 128
                || anchor.rects.iter().any(|r| !r.valid())
                || anchor
                    .char_range
                    .is_some_and(|[a, b]| a >= b || b > 128000 || b - a > 16384)
            {
                return Err("Invalid or oversized PDF comment anchor".into());
            }
            Ok(Reopen::Pdf(anchor))
        }
        _ => Err("This comment source cannot be reopened by the native reader yet".into()),
    }
}
pub fn bounded(value: &str, max: usize) -> String {
    if value.len() <= max {
        return value.into();
    }
    let mut end = max;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].into()
}
impl Capture {
    pub fn validate(self) -> Result<Self, String> {
        if self.excerpt.trim().is_empty() || self.excerpt.len() > 16384 {
            return Err("Select a passage of at most 16 KiB to comment".into());
        }
        if serde_json::to_vec(&self.source)
            .map_err(|e| e.to_string())?
            .len()
            > 32768
        {
            return Err("Comment source anchor is too large".into());
        }
        Ok(self)
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Snapshot {
    pub drafts: Vec<Value>,
    pub additional: String,
    pub revision: String,
}
#[derive(Clone, Debug)]
pub struct Submission {
    pub dialog: u64,
    pub serial: u64,
    pub key: String,
    pub snapshot: Arc<Snapshot>,
}
#[derive(Debug)]
pub struct Consumed {
    pub snapshot: Snapshot,
    pub dirty: bool,
    pub conflict: bool,
}
#[derive(Debug)]
pub enum Request {
    Consume {
        key: String,
        submitted: Arc<Snapshot>,
        local: Snapshot,
    },
    Read(String),
    Save {
        key: String,
        snapshot: Snapshot,
    },
    Prompt {
        drafts: Vec<Value>,
        additional: String,
    },
}
#[derive(Debug)]
pub enum Reply {
    Consumed(Consumed),
    Snapshot(Snapshot),
    Prompt(String),
}
pub fn excerpt(draft: &Value) -> &str {
    draft["excerpt"]
        .as_str()
        .filter(|s| !s.is_empty())
        .or_else(|| draft["quote"].as_str())
        .unwrap_or("")
}
pub fn note(draft: &Value) -> &str {
    draft["note"]
        .as_str()
        .or_else(|| draft["comment"].as_str())
        .unwrap_or("")
}
pub fn set_note(draft: &mut Value, note: String) {
    if let Some(fields) = draft.as_object_mut() {
        fields.remove("comment");
        if note.is_empty() {
            fields.remove("note");
        } else {
            fields.insert("note".into(), Value::String(note));
        }
    }
}
pub fn source(draft: &Value) -> Value {
    draft.get("source").cloned().unwrap_or_else(||{
    if draft["target"]["kind"]=="fileRange"{json!({"provider":"document","version":1,"anchor":draft["target"]})}else{json!({"provider":"chat","version":1,"anchor":{"turnId":draft["turnId"],"itemId":draft["itemId"]}})}
})
}
pub fn location(source: &Value) -> String {
    let anchor = &source["anchor"];
    if source["provider"] == "chat" {
        let mut parts = Vec::new();
        for (field, label) in [
            ("sessionKey", "session"),
            ("turnId", "turn"),
            ("itemId", "item"),
        ] {
            if let Some(value) = anchor[field].as_str().filter(|s| !s.is_empty()) {
                parts.push(format!("{label} {value}"));
            }
        }
        return if parts.is_empty() {
            "Chat comment".into()
        } else {
            parts.join(" / ")
        };
    }
    if source["provider"] == "table" {
        let mut text = format!(
            "{} / {}!{}",
            anchor["filePath"].as_str().unwrap_or("Table"),
            anchor["sheet"].as_str().unwrap_or("Sheet"),
            anchor["range"].as_str().unwrap_or("A1")
        );
        if let Some(header) = anchor["header"].as_str().filter(|s| !s.is_empty()) {
            text.push_str(&format!(" / header {header}"));
        }
        if anchor["sql"].is_string() {
            text.push_str(" / SQL result cell");
        }
        if let Some(hash) = anchor["documentHash"].as_str().filter(|s| !s.is_empty()) {
            text.push_str(&format!(" / base {hash}"));
        }
        return text;
    }
    if source["provider"] == "pdf" {
        let mut text = format!(
            "{} / page {}",
            anchor["filePath"].as_str().unwrap_or("PDF"),
            anchor["page"].as_u64().unwrap_or(1)
        );
        if let Some(rects) = anchor["rects"].as_array().filter(|r| !r.is_empty()) {
            text.push_str(&format!(
                " / regions {}",
                serde_json::to_string(rects).unwrap_or_default()
            ));
        }
        if let Some(hash) = anchor["documentHash"].as_str().filter(|s| !s.is_empty()) {
            text.push_str(&format!(" / base {hash}"));
        }
        return text;
    }
    if source["provider"] == "epub" {
        let mut parts = Vec::new();
        if let Some(path) = anchor["filePath"].as_str() {
            parts.push(path.to_owned());
        }
        if let Some(label) = anchor["chapterLabel"].as_str().filter(|s| !s.is_empty()) {
            parts.push(format!("chapter {label}"));
        }
        if let Some(cfi) = anchor["cfiRange"].as_str().filter(|s| !s.is_empty()) {
            parts.push(format!("EPUB CFI {cfi}"));
        }
        if let Some(hash) = anchor["bookHash"].as_str().filter(|s| !s.is_empty()) {
            parts.push(format!("base {hash}"));
        }
        return parts.join(" / ");
    }

    let file = anchor["filePath"].as_str().unwrap_or("");
    if source["provider"] == "document" && !file.is_empty() {
        let line = anchor["startLine"].as_u64().unwrap_or(0);
        let end = anchor["endLine"].as_u64().unwrap_or(line);
        let mut text = file.to_owned();
        if line > 0 {
            text.push_str(&format!(" / line {line}"));
            if end > line {
                text.push_str(&format!("–{end}"));
            }
        }
        if let Some(hash) = anchor["baseHash"].as_str().filter(|s| !s.is_empty()) {
            text.push_str(&format!(" / base {hash}"));
        }
        return text;
    }
    for field in [
        "url",
        "path",
        "filePath",
        "chapterLabel",
        "itemId",
        "turnId",
    ] {
        if let Some(text) = anchor[field].as_str().filter(|s| !s.is_empty()) {
            return text.to_owned();
        }
    }
    source["provider"].as_str().unwrap_or("Comment").to_owned()
}
fn validate(snapshot: Snapshot) -> Result<Snapshot, String> {
    if snapshot.drafts.len() > 32
        || snapshot.additional.len() > 32768
        || snapshot.revision.len() != 64
    {
        return Err("Invalid annotation snapshot size or revision".into());
    }
    for draft in &snapshot.drafts {
        if !draft.is_object()
            || excerpt(draft).is_empty()
            || excerpt(draft).len() > 16384
            || note(draft).len() > 16384
            || draft["id"]
                .as_str()
                .is_none_or(|s| s.is_empty() || s.len() > 128)
        {
            return Err("Invalid comment record".into());
        }
    }
    Ok(snapshot)
}
fn prompt(drafts: Vec<Value>, additional: String) -> Result<String, String> {
    let snapshot = validate(Snapshot {
        drafts,
        additional,
        revision: "0".repeat(64),
    })?;
    if snapshot.drafts.is_empty() {
        return Err("Add a comment first".into());
    }
    let mut result=String::from("Please address every comment below. For file comments, read the current file first and locate the passage using its path, quote, and context. Preserve unrelated changes.\n\n");
    if snapshot
        .drafts
        .iter()
        .any(|d| source(d)["provider"] == "epub")
    {
        result.push_str("E-book excerpts are read-only: explain the quoted passage, its context and key concepts; do not assume the book itself can be edited.\n\n");
    }
    if snapshot
        .drafts
        .iter()
        .any(|d| source(d)["provider"] == "pdf")
    {
        result.push_str("PDF excerpts are read-only: answer using the page, region and quote; do not modify the PDF.\n\n");
    }
    for (index, draft) in snapshot.drafts.iter().enumerate() {
        let anchor = location(&source(draft))
            .replace('\r', "\\r")
            .replace('\n', "\\n");
        result.push_str(&format!(
            "{} > {}\n  @ {}\n",
            index + 1,
            excerpt(draft).replace("\r\n", "\n"),
            anchor
        ));
        if !note(draft).trim().is_empty() {
            result.push_str(&format!("  < {}\n", note(draft).trim()));
        }
        let source = source(draft);
        if source["provider"] == "table" {
            if let Some(sql) = source["anchor"]["sql"].as_str() {
                result.push_str("  This cell belongs to the result of this read-only query; its coordinates are not raw source coordinates:\n\n");
                for line in sql.lines() {
                    result.push_str("    ");
                    result.push_str(line);
                    result.push('\n');
                }
            }
        }
        result.push('\n');
    }
    if !snapshot.additional.trim().is_empty() {
        result.push_str(snapshot.additional.trim());
    }
    Ok(result.trim().to_owned())
}
fn merge_consumed(submitted: &Snapshot, local: Snapshot, mut server: Snapshot) -> Consumed {
    let mut conflict = false;
    let original_drafts = server.drafts.clone();
    for record in &local.drafts {
        let old = submitted
            .drafts
            .iter()
            .find(|old| old["id"] == record["id"]);
        if old.is_some_and(|old| old == record) {
            continue;
        }
        if let Some(existing) = server
            .drafts
            .iter_mut()
            .find(|existing| existing["id"] == record["id"])
        {
            if existing != record {
                conflict = true;
                *existing = record.clone();
            }
        } else {
            server.drafts.push(record.clone());
        }
    }
    for old in &submitted.drafts {
        if !local.drafts.iter().any(|record| record["id"] == old["id"])
            && server.drafts.iter().any(|record| record["id"] == old["id"])
        {
            conflict = true;
        }
    }
    let server_additional = server.additional.clone();
    if local.additional != submitted.additional {
        if !server.additional.is_empty() && server.additional != local.additional {
            conflict = true;
        }
        server.additional = local.additional;
    }
    // Compare the merged view with the authoritative post-cleanup snapshot.
    let dirty =
        conflict || server.drafts != original_drafts || server.additional != server_additional;
    Consumed {
        snapshot: server,
        dirty,
        conflict,
    }
}
async fn parse_snapshot(value: Value) -> Result<Reply, String> {
    crate::worker::compute(move || {
        let snapshot =
            serde_json::from_value(value).map_err(|e| format!("Invalid comments: {e}"))?;
        validate(snapshot).map(Reply::Snapshot)
    })
    .await
}
pub async fn execute(api: &Api, request: Request) -> Result<Reply, String> {
    match request {
        Request::Read(key) => {
            let query = url::form_urlencoded::Serializer::new(String::new())
                .append_pair("sessionKey", &key)
                .finish();
            parse_snapshot(
                api.json(
                    reqwest::Method::GET,
                    &format!("/studio/session-state/annotations?{query}"),
                    None,
                )
                .await?,
            )
            .await
        }
        Request::Save { key, snapshot } => {
            let body=crate::worker::compute(move||{
                let saved=validate(snapshot)?;
                Ok(json!({"sessionKey":key,"expectedRevision":saved.revision,"drafts":saved.drafts,"additional":saved.additional}))
            }).await?;
            parse_snapshot(
                api.json(
                    reqwest::Method::PUT,
                    "/studio/session-state/annotations",
                    Some(&body),
                )
                .await?,
            )
            .await
        }
        Request::Prompt { drafts, additional } => {
            crate::worker::compute(move || prompt(drafts, additional).map(Reply::Prompt)).await
        }
        Request::Consume {
            key,
            submitted,
            local,
        } => {
            let sent = submitted.clone();
            let body = crate::worker::compute(move || {
                validate((*sent).clone())?;
                Ok(json!({"sessionKey":key,"drafts":sent.drafts,"additional":sent.additional}))
            })
            .await?;
            let value = api
                .json(
                    reqwest::Method::POST,
                    "/studio/session-state/annotations/consume",
                    Some(&body),
                )
                .await?;
            #[cfg(debug_assertions)]
            if let Some(root) =
                std::env::var_os("STUDIO_GNOME_SMOKE_DIR").map(std::path::PathBuf::from)
            {
                if std::env::var_os("CODEX_THREAD_STUDIO_CODEX_BIN").map(std::path::PathBuf::from)
                    == Some(root.join("fake-codex.py"))
                    && std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from)
                        == Some(root.join("config"))
                    && root.join("lose-comment-cleanup-ack").is_file()
                {
                    std::fs::remove_file(root.join("lose-comment-cleanup-ack"))
                        .map_err(|e| e.to_string())?;
                    return Err("Private comment cleanup acknowledgement was lost".into());
                }
            }
            crate::worker::compute(move || {
                let server = validate(
                    serde_json::from_value(value)
                        .map_err(|e| format!("Invalid comment cleanup: {e}"))?,
                )?;
                Ok(Reply::Consumed(merge_consumed(&submitted, local, server)))
            })
            .await
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chat_capture_keeps_rendered_unicode_quotes_without_inventing_browser_offsets() {
        let item = crate::model::Item::from_value(
            &json!({"id":"reply","turnId":"turn","type":"agentMessage","text":"Markdown **bold** 中文😀"}),
        );
        let capture = chat_capture(&item, "codex:thread", Some(" bold 中文😀  \n".into())).unwrap();
        assert_eq!(capture.excerpt, " bold 中文😀  \n");
        assert_eq!(capture.source["anchor"]["sessionKey"], "codex:thread");
        assert_eq!(capture.source["anchor"]["turnId"], "turn");
        assert_eq!(capture.source["anchor"]["itemId"], "reply");
        assert!(capture.source["anchor"]["startOffset"].is_null());
        assert!(capture.source["anchor"]["endOffset"].is_null());
        assert!(chat_capture(&item, "codex:thread", Some("😀".repeat(4096))).is_ok());
        for quote in [None, Some("  \n".into()), Some("😀".repeat(4097))] {
            assert!(chat_capture(&item, "codex:thread", quote).is_err());
        }
        assert!(chat_capture(&item, "codex:", Some("quote".into())).is_err());
    }
    #[test]
    fn routed_chat_capture_preserves_real_target_identity_in_the_selected_library() {
        let mut item = crate::model::Item::from_value(
            &json!({"id":"router-wrapper","turnId":"router-turn","type":"agentMessage","text":"Routed response"}),
        );
        item.origin = Some(Arc::new(crate::model::Origin {
            backend: "opencode".into(),
            thread: "target".into(),
            title: "Target".into(),
            root: "/project".into(),
            turn: "source-turn".into(),
            item: "source-reply".into(),
            question: Arc::new("Original question".into()),
            excerpt: true,
        }));
        let capture =
            chat_capture(&item, "codex:router", Some("Routed quote 中文😀".into())).unwrap();
        assert_eq!(capture.source["anchor"]["sessionKey"], "opencode:target");
        assert_eq!(capture.source["anchor"]["turnId"], "source-turn");
        assert_eq!(capture.source["anchor"]["itemId"], "source-reply");
        let text = prompt(
            vec![json!({"id":"comment","excerpt":capture.excerpt,"source":capture.source})],
            String::new(),
        )
        .unwrap();
        assert!(text.contains("session opencode:target / turn source-turn / item source-reply"));
        assert!(!text.contains("router-wrapper"));
    }
    #[test]
    fn table_locations_and_sql_prompt_preserve_result_identity_and_literal_spaces() {
        assert_eq!(table_cell("$AA$17").unwrap(), (16, 26));
        for invalid in ["A0", "A+1", "A10001", "GS1", "A1:B2", "XFD1"] {
            assert!(table_cell(invalid).is_err(), "accepted {invalid}");
        }
        let source = json!({"provider":"table","version":1,"anchor":{"filePath":"/data/a  b.csv","sheet":"Data","range":"B3","documentHash":"hash","header":"value","sql":"SELECT 'a  b' AS value","delimiter":";"}});
        assert!(matches!(reopen(&source),Ok(Reopen::Table(a)) if a.sql.is_some()));
        let text = prompt(
            vec![json!({"id":"one","excerpt":" a  b ","source":source})],
            String::new(),
        )
        .unwrap();
        assert!(text.contains("    SELECT 'a  b' AS value") && text.contains("SQL result cell"));
        assert!(text.contains("/data/a  b.csv"));
        assert!(text.contains("1 >  a  b \n"));
    }
    #[test]
    fn source_reopening_preserves_epub_ranges_and_rejects_active_urls() {
        let source = json!({"provider":"epub","version":1,"anchor":{"root":"/books","filePath":"/books/a.epub","bookHash":"hash","href":"OPS/ch2.xhtml","cfiRange":"epubcfi(/6/4!/4/2,/1:2,/1:8)","chapterLabel":"Second"}});
        assert!(matches!(reopen(&source),Ok(Reopen::Epub(a)) if a.cfi_range.contains(',')));
        assert!(location(&source).contains("EPUB CFI epubcfi("));
        let pdf = json!({"provider":"pdf","version":1,"anchor":{"filePath":"/books/a.pdf","documentHash":"hash","page":2,"charRange":[2,8],"rects":[{"x":0.1,"y":0.2,"width":0.3,"height":0.02}]}});
        assert!(
            matches!(reopen(&pdf),Ok(Reopen::Pdf(a)) if a.page==2 && a.char_range==Some([2,8]))
        );
        assert!(location(&pdf).contains("page 2 / regions"));
        let mut invalid = pdf;
        invalid["anchor"]["page"] = json!(0);
        assert!(reopen(&invalid).is_err());
        for url in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://user:secret@example.com/",
        ] {
            assert!(
                reopen(&json!({"provider":"browser","version":1,"anchor":{"url":url}})).is_err()
            );
        }
        assert!(matches!(
            reopen(
                &json!({"provider":"browser","version":1,"anchor":{"url":"https://example.com/book?q=你好"}})
            ),
            Ok(Reopen::Browser(_))
        ));
        let mut bad = source;
        bad["anchor"]["href"] = json!("https://example.com/chapter");
        assert!(reopen(&bad).is_err());
        assert_eq!(bounded("中文😀", 7), "中文");
    }
    #[test]
    fn accepted_cleanup_merges_newer_local_and_shared_edits_without_silent_overwrites() {
        let record = |id: &str, note: &str| json!({"id":id,"excerpt":"text","note":note});
        let snapshot = |drafts: Vec<Value>, additional: &str| Snapshot {
            drafts,
            additional: additional.into(),
            revision: "0".repeat(64),
        };
        let sent = snapshot(vec![record("one", "old"), record("two", "old")], "sent");
        let local = snapshot(
            vec![
                record("one", "local"),
                record("two", "old"),
                record("new-local", ""),
            ],
            "new-local",
        );
        let server = snapshot(vec![record("new-server", "")], "");
        let merged = merge_consumed(&sent, local, server);
        assert!(merged.dirty);
        assert!(!merged.conflict);
        assert_eq!(merged.snapshot.drafts.len(), 3);
        assert_eq!(merged.snapshot.additional, "new-local");
        assert!(!merged.snapshot.drafts.iter().any(|r| r["id"] == "two"));
        let merged = merge_consumed(
            &sent,
            snapshot(vec![record("one", "local")], "local"),
            snapshot(vec![record("one", "remote")], "remote"),
        );
        assert!(merged.conflict);
        assert_eq!(merged.snapshot.drafts[0]["note"], "local");
        let text = prompt(vec![record("one", "note")], "instructions".into()).unwrap();
        assert_eq!(text.trim(), text);
    }
    #[test]
    fn legacy_and_opaque_records_keep_source_semantics() {
        let legacy = json!({"id":"a","quote":"中文😀","comment":"clarify","target":{"kind":"fileRange","filePath":"/tmp/a.md","startLine":3,"endLine":5,"baseHash":"revision"}});
        assert_eq!(excerpt(&legacy), "中文😀");
        assert_eq!(note(&legacy), "clarify");
        assert_eq!(
            location(&source(&legacy)),
            "/tmp/a.md / line 3–5 / base revision"
        );
        let mut cleared = legacy.clone();
        set_note(&mut cleared, String::new());
        assert_eq!(note(&cleared), "");
        assert!(cleared.get("comment").is_none());
        let opaque = json!({"source":{"provider":"future","version":8,"anchor":{"custom":[1,2]}}});
        assert_eq!(source(&opaque), opaque["source"]);
    }
}
