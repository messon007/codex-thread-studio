//! Bounded document I/O, parsing and edit composition. This module has no GTK API.
use std::{collections::HashMap, sync::Arc};

use reqwest::Method;
use serde::Deserialize;
use serde_json::{json, Value};
use studio_markdown_native::{
    edit_buffer::EditBuffer, BlockKind, Cell, DisplayBlock, Fragment, SourceWindow,
};
use tokio::sync::mpsc;

use crate::{gateway::Api, protocol::Event};
#[path = "document_resume.rs"]
mod resume;

#[derive(Debug)]
pub enum Request {
    Open {
        root: String,
        path: String,
        read_only: bool,
    },
    Resume {
        revision: String,
        text: Option<String>,
    },
    Index,
    Fragment(usize),
    SelectableFragment {
        page: usize,
        cancel: Arc<std::sync::atomic::AtomicBool>,
    },
    Table {
        id: usize,
        row: usize,
        columns: Vec<usize>,
    },
    SelectionTables {
        reader: String,
        revision: String,
        ids: Vec<usize>,
        limit: usize,
        reply: tokio::sync::oneshot::Sender<
            Result<Vec<studio_markdown_native::flow::TableText>, String>,
        >,
    },
    SelectionPrefix {
        reader: String,
        revision: String,
        id: usize,
        characters: usize,
        reply:
            tokio::sync::oneshot::Sender<Result<studio_markdown_native::flow::TablePrefix, String>>,
    },
    Source {
        line: usize,
        offset: Option<usize>,
        previous: bool,
    },
    Edit {
        line: usize,
        offset: Option<usize>,
        previous: bool,
    },
    Insert {
        offset: usize,
        text: String,
    },
    Delete {
        from: usize,
        to: usize,
    },
    Save {
        text: String,
    },
    Search(String),
    LocateQuote(studio_markdown_native::quote::Query),
    Diagram {
        id: usize,
        kind: String,
        source: String,
        dark: bool,
        style: crate::diagram_style::Style,
    },
    CancelDiagram {
        id: usize,
    },
    CancelDiagrams,
    Heartbeat,
    Close,
}

#[derive(Debug)]
pub struct Message {
    pub key: u64,
    pub generation: u64,
    pub serial: u64,
    pub request: Request,
}

#[derive(Debug)]
pub struct Response {
    pub key: u64,
    pub generation: u64,
    pub serial: u64,
    pub kind: &'static str,
    pub result: Result<Reply, String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub rows: usize,
    pub row_start: usize,
    pub table: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Heading {
    pub id: usize,
    pub level: u8,
    pub label: String,
    pub line: usize,
    pub page: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Table {
    pub first_page: usize,
    pub end_page: usize,
    pub rows: usize,
    pub columns: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    pub name: String,
    pub revision: String,
    pub source_bytes: usize,
    pub source_lines: usize,
    pub pages: Vec<Page>,
    pub toc: Vec<Heading>,
    pub tables: Vec<Table>,
}

impl Index {
    fn validate(mut self) -> Result<Self, String> {
        if self.source_bytes > 128 * 1024 * 1024
            || self.pages.len() > 100_000
            || self.toc.len() > 100_000
            || self.tables.len() > self.pages.len()
        {
            return Err("Document metadata exceeds its budget".into());
        }
        for page in &self.pages {
            if page.start > page.end
                || page.end > self.source_bytes
                || page.line == 0
                || page.line > self.source_lines
                || page.table.is_some_and(|id| id >= self.tables.len())
            {
                return Err("Document source page is invalid".into());
            }
            if let Some(id) = page.table {
                if page.row_start.saturating_add(page.rows) > self.tables[id].rows {
                    return Err("Table source row is invalid".into());
                }
            }
        }
        for heading in &self.toc {
            if heading.page >= self.pages.len()
                || heading.line == 0
                || heading.line > self.source_lines
            {
                return Err("Document heading is invalid".into());
            }
        }
        for table in &mut self.tables {
            for column in &mut table.columns {
                *column = studio_markdown_native::column_label(column);
            }
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditPage {
    pub document_id: String,
    pub revision: String,
    pub window: SourceWindow,
    pub source_lines: usize,
    pub source_bytes: usize,
    pub warning: Option<String>,
}

#[derive(Debug)]
pub struct TablePage {
    pub id: usize,
    pub columns: Vec<usize>,
    pub start: usize,
    pub end: usize,
    pub page_size: usize,
    pub total: usize,
    pub header: Vec<Cell>,
    pub rows: Vec<Vec<Cell>>,
}

#[derive(Debug)]
pub enum Reply {
    Resumed {
        reader_id: String,
        editor: Option<EditPage>,
    },
    Open {
        read_only: bool,
        reader_id: String,
    },
    Index(Option<Arc<Index>>),
    Fragment {
        page: usize,
        blocks: Vec<DisplayBlock>,
        line: usize,
    },
    SelectableFragment {
        page: usize,
        line: usize,
        prepared: Box<crate::flow_prepare::Prepared>,
    },
    Table(TablePage),
    Source(SourceWindow),
    Edit(EditPage),
    Saved(EditPage),
    Search(Vec<SearchHit>),
    LocatedQuote(studio_markdown_native::quote::Located),
    Diagram {
        id: usize,
        image: crate::diagram::Image,
    },
    Ack,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SearchHit {
    pub line: usize,
    #[serde(rename = "snippet")]
    pub preview: String,
}

struct Editor {
    page: EditPage,
    buffer: EditBuffer,
}

struct Session {
    generation: u64,
    reader: String,
    document: String,
    editor: Option<Editor>,
    poisoned: Option<String>,
    index: Option<Arc<Index>>,
    origin: Origin,
    valid: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct Origin {
    root: String,
    path: String,
    read_only: bool,
}

impl Request {
    fn coalescing_key(&self) -> Option<(&'static str, usize)> {
        match self {
            Self::Index => Some(("index", 0)),
            Self::Fragment(_) | Self::SelectableFragment { .. } => Some(("fragment", 0)),
            Self::Table { id, .. } => Some(("table", *id)),
            Self::Source { .. } => Some(("source", 0)),
            Self::Search(_) => Some(("search", 0)),
            Self::LocateQuote(_) => Some(("locate-quote", 0)),
            Self::Heartbeat => Some(("heartbeat", 0)),
            _ => None,
        }
    }
    fn kind(&self) -> &'static str {
        match self {
            Self::Open { .. } => "open",
            Self::Resume { .. } => "resume",
            Self::Index => "index",
            Self::Fragment(_) | Self::SelectableFragment { .. } => "fragment",
            Self::Table { .. } => "table",
            Self::SelectionTables { .. } => "selection-tables",
            Self::SelectionPrefix { .. } => "selection-prefix",
            Self::Source { .. } => "source",
            Self::Edit { .. } => "edit",
            Self::Insert { .. } | Self::Delete { .. } => "mutation",
            Self::Save { .. } => "save",
            Self::Search(_) => "search",
            Self::LocateQuote(_) => "locate-quote",
            Self::Diagram { .. } => "diagram",
            Self::CancelDiagram { .. } | Self::CancelDiagrams => "diagram-cancel",
            Self::Heartbeat => "heartbeat",
            Self::Close => "close",
        }
    }
}

pub async fn run(api: Api, input: mpsc::Receiver<Message>, output: async_channel::Sender<Event>) {
    let diagrams = crate::diagram_queue::Queue::new(output.clone());
    run_with_queue(api, input, output, diagrams).await;
}

async fn run_with_queue(
    api: Api,
    input: mpsc::Receiver<Message>,
    output: async_channel::Sender<Event>,
    diagrams: crate::diagram_queue::Queue,
) {
    let (_sender, control) = tokio::sync::watch::channel(Some(api));
    run_actor(control, input, output, diagrams).await;
}

pub async fn run_recoverable(
    control: tokio::sync::watch::Receiver<Option<Api>>,
    input: mpsc::Receiver<Message>,
    output: async_channel::Sender<Event>,
) {
    let diagrams = crate::diagram_queue::Queue::new(output.clone());
    run_actor(control, input, output, diagrams).await;
}

async fn run_actor(
    mut control: tokio::sync::watch::Receiver<Option<Api>>,
    mut input: mpsc::Receiver<Message>,
    output: async_channel::Sender<Event>,
    diagrams: crate::diagram_queue::Queue,
) {
    let mut api = control.borrow_and_update().clone();
    let mut controls_open = true;
    let mut sessions: HashMap<u64, Session> = HashMap::new();
    let selections = Arc::new(tokio::sync::Semaphore::new(2));
    let mut carry = None;
    loop {
        let mut message = match carry.take() {
            Some(message) => message,
            None => tokio::select! {
                biased;
                changed = control.changed(), if controls_open => {
                    if changed.is_err() { controls_open = false; }
                    else {
                        api = control.borrow_and_update().clone();
                        for (key, session) in &mut sessions {
                            session.valid = false;
                            diagrams.cancel(*key);
                        }
                    }
                    continue;
                }
                message = input.recv() => match message {Some(message) => message, None => break},
            },
        };
        if let Some(key) = message.request.coalescing_key() {
            while let Ok(next) = input.try_recv() {
                if next.key == message.key
                    && next.generation == message.generation
                    && next.request.coalescing_key() == Some(key)
                {
                    message = next;
                } else {
                    carry = Some(next);
                    break;
                }
            }
        }
        if let Request::SelectionPrefix {
            reader,
            revision,
            id,
            characters,
            mut reply,
        } = message.request
        {
            let valid = sessions
                .get(&message.key)
                .filter(|s| s.valid && s.generation == message.generation && s.reader == reader)
                .and_then(|s| {
                    s.index
                        .as_ref()
                        .filter(|i| i.revision == revision)
                        .map(|index| (s.document.clone(), index.clone()))
                });
            let Some((document, index)) = valid else {
                let _ = reply.send(Err("Selection document snapshot changed".into()));
                continue;
            };
            let Ok(permit) = selections.clone().try_acquire_owned() else {
                let _ = reply.send(Err("Selection workers are busy; try again".into()));
                continue;
            };
            let Some(api) = api.clone() else {
                let _ = reply.send(Err(
                    "Gateway stopped; selection snapshot is unavailable".into()
                ));
                continue;
            };
            tokio::spawn(async move {
                let _permit = permit;
                let result = tokio::select! {
                    result=selection_prefix(&api,&reader,&document,&revision,id,characters,&index)=>result,
                    _=reply.closed()=>return,
                };
                let _ = reply.send(result);
            });
            continue;
        }
        if matches!(message.request, Request::SelectionTables { .. }) {
            let Request::SelectionTables {
                reader,
                revision,
                ids,
                limit,
                mut reply,
            } = message.request
            else {
                unreachable!()
            };
            let valid = sessions
                .get(&message.key)
                .filter(|s| s.valid && s.generation == message.generation && s.reader == reader)
                .and_then(|s| {
                    s.index
                        .as_ref()
                        .filter(|i| i.revision == revision)
                        .map(|index| (s.document.clone(), index.clone()))
                });
            let Some((document, index)) = valid else {
                let _ = reply.send(Err("Selection document snapshot changed".into()));
                continue;
            };
            let Ok(permit) = selections.clone().try_acquire_owned() else {
                let _ = reply.send(Err("Selection workers are busy; try again".into()));
                continue;
            };
            let Some(api) = api.clone() else {
                let _ = reply.send(Err(
                    "Gateway stopped; selection snapshot is unavailable".into()
                ));
                continue;
            };
            tokio::spawn(async move {
                let _permit = permit;
                let result = tokio::select! {
                    result=selection_tables(&api,&reader,&document,&revision,&ids,limit,&index)=>result,
                    _=reply.closed()=>return,
                };
                let _ = reply.send(result);
            });
            continue;
        }
        if let Request::CancelDiagram { id } = message.request {
            diagrams.cancel_one(message.key, message.generation, id);
            continue;
        }
        if matches!(message.request, Request::CancelDiagrams) {
            diagrams.cancel_generation(message.key, message.generation);
            continue;
        }
        if matches!(message.request, Request::Open { .. } | Request::Close) {
            diagrams.cancel(message.key);
        }
        if matches!(message.request, Request::Diagram { .. }) {
            if let Err(message) = diagrams.submit(message) {
                if output
                    .send(Event::Document(Response {
                        key: message.key,
                        generation: message.generation,
                        serial: message.serial,
                        kind: "diagram",
                        result: Err(
                            "Diagram render queue is full or the source exceeds 128 KiB; try again"
                                .into(),
                        ),
                    }))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            continue;
        }
        let kind = message.request.kind();
        let result = match &message.request {
            // Local edit composition survives process replacement. Never cancel
            // an admitted keystroke when the API watch changes.
            Request::Insert { offset, text } => tracked_editor(&mut sessions, &message)
                .and_then(|editor| editor.buffer.insert(*offset, text))
                .map(|()| Reply::Ack),
            Request::Delete { from, to } => tracked_editor(&mut sessions, &message)
                .and_then(|editor| editor.buffer.delete(*from, *to))
                .map(|()| Reply::Ack),
            Request::Close if api.is_none() => {
                if sessions
                    .get(&message.key)
                    .is_some_and(|s| s.generation == message.generation)
                {
                    sessions.remove(&message.key);
                }
                Ok(Reply::Ack)
            }
            _ => {
                if let Some(active) = &api {
                    tokio::select! {
                        biased;
                        changed = control.changed(), if controls_open => {
                            if changed.is_err() { controls_open = false; }
                            else { api = control.borrow_and_update().clone(); }
                            for (key, session) in &mut sessions { session.valid = false; diagrams.cancel(*key); }
                            Err("Gateway changed; the pending document request was not replayed".into())
                        }
                        result = handle(active, &mut sessions, &message) => result,
                    }
                } else {
                    Err("Gateway stopped; the document service is unavailable".into())
                }
            }
        };
        // Successful keystrokes need no UI events; a failure poisons save until
        // the page is explicitly reloaded, so partially tracked edits never save.
        if kind == "mutation" && result.is_ok() {
            continue;
        }
        if let Err(error) = &result {
            if kind == "mutation" {
                if let Some(session) = sessions
                    .get_mut(&message.key)
                    .filter(|s| s.generation == message.generation)
                {
                    session.poisoned = Some(error.clone());
                }
            }
        }
        if output
            .send(Event::Document(Response {
                key: message.key,
                generation: message.generation,
                serial: message.serial,
                kind,
                result,
            }))
            .await
            .is_err()
        {
            break;
        }
    }
}

fn tracked_editor<'a>(
    sessions: &'a mut HashMap<u64, Session>,
    message: &Message,
) -> Result<&'a mut Editor, String> {
    sessions
        .get_mut(&message.key)
        .filter(|s| s.generation == message.generation)
        .and_then(|s| s.editor.as_mut())
        .ok_or_else(|| "Editor has no matching page".into())
}

async fn handle(
    api: &Api,
    sessions: &mut HashMap<u64, Session>,
    message: &Message,
) -> Result<Reply, String> {
    if let Request::Resume { revision, text } = &message.request {
        return resume::run(api, sessions, message, revision, text.as_deref()).await;
    }
    if let Request::Open {
        root,
        path,
        read_only,
    } = &message.request
    {
        if !sessions.contains_key(&message.key) && sessions.len() >= 4 {
            return Err("Close a document before opening another".into());
        }
        let value = api
            .json(
                Method::POST,
                if *read_only {
                    "/studio/native/markdown/open-readonly"
                } else {
                    "/studio/native/markdown/open"
                },
                Some(&json!({"root":root,"path":path})),
            )
            .await?;
        let reader = value["readerId"]
            .as_str()
            .ok_or("Reader identifier missing")?
            .to_owned();
        let canonical_path = |key: &str| {
            value[key]
                .as_str()
                .filter(|s| {
                    s.len() <= 32768 && !s.contains('\0') && std::path::Path::new(s).is_absolute()
                })
                .map(str::to_owned)
                .ok_or_else(|| format!("Canonical document {key} is missing or invalid"))
        };
        let origin = Origin {
            root: canonical_path("root")?,
            path: canonical_path("path")?,
            read_only: value["readOnly"].as_bool().unwrap_or(true),
        };
        if let Some(previous) = sessions.remove(&message.key) {
            let _ = api
                .json(
                    Method::POST,
                    "/studio/markdown/close",
                    Some(&json!({"id":previous.reader})),
                )
                .await;
        }
        sessions.insert(
            message.key,
            Session {
                generation: message.generation,
                reader: reader.clone(),
                document: String::new(),
                editor: None,
                poisoned: None,
                index: None,
                origin,
                valid: true,
            },
        );
        return Ok(Reply::Open {
            read_only: value["readOnly"].as_bool().unwrap_or(true),
            reader_id: reader,
        });
    }
    let session = sessions
        .get_mut(&message.key)
        .ok_or("Open a document first")?;
    if session.generation != message.generation {
        return Ok(Reply::Ack);
    }
    if !session.valid && !matches!(message.request, Request::Close) {
        return Err("Revalidate the document snapshot before using this service".into());
    }
    let base = format!("/studio/native/markdown/{}", session.reader);
    match &message.request {
        Request::SelectionTables { .. } | Request::SelectionPrefix { .. } => {
            Err("Selection request requires its independent read lane".into())
        }
        Request::Index => {
            let mut value = api
                .json(Method::GET, &format!("{base}/document"), None)
                .await?;
            session.document = value["documentId"]
                .as_str()
                .ok_or("Document identifier missing")?
                .into();
            match value["state"].as_str() {
                Some("ready") => {
                    let raw = value["index"].take();
                    let index = crate::worker::compute(move || {
                        decode::<Index>(raw)?.validate().map(Arc::new)
                    })
                    .await?;
                    session.index = Some(index.clone());
                    Ok(Reply::Index(Some(index)))
                }
                Some("indexing") => Ok(Reply::Index(None)),
                _ => Err(value["error"]
                    .as_str()
                    .unwrap_or("Document indexing failed")
                    .into()),
            }
        }
        Request::Fragment(page) | Request::SelectableFragment { page, .. } => {
            let cancel = match &message.request {
                Request::SelectableFragment { cancel, .. } => Some(cancel.clone()),
                _ => None,
            };
            let index = session
                .index
                .as_ref()
                .ok_or("Wait for the document index")?;
            if index.pages.is_empty() {
                if let Some(cancel) = cancel {
                    let revision = index.revision.clone();
                    let prepared = crate::worker::compute(move || {
                        crate::flow_prepare::prepare_projection(
                            Arc::new(Default::default()),
                            revision,
                            cancel,
                            false,
                        )
                    })
                    .await?;
                    return Ok(Reply::SelectableFragment {
                        page: 0,
                        line: 1,
                        prepared: Box::new(prepared),
                    });
                }
                return Ok(Reply::Fragment {
                    page: 0,
                    line: 1,
                    blocks: Vec::new(),
                });
            }
            let (page, targets) = preview_pages(index, *page)?;
            let mut blocks = Vec::new();
            let mut fragments = Vec::new();
            for target in targets {
                let fragment: Fragment = decode(
                    api.json(
                        Method::GET,
                        &format!(
                            "{base}/fragment?target={target}&revision={}",
                            index.revision
                        ),
                        None,
                    )
                    .await?,
                )?;
                if cancel.is_some() {
                    if fragment.revision != index.revision
                        || fragment.document_id != session.document
                    {
                        return Err(
                            "Preview fragment belongs to a different document snapshot".into()
                        );
                    }
                    fragments.push(fragment);
                    continue;
                }
                blocks.extend(
                    crate::worker::compute(move || {
                        studio_markdown_native::parse_readable(&fragment)
                            .map(studio_markdown_native::display_rows)
                    })
                    .await?,
                );
                if blocks.len() > 32768 {
                    return Err("Preview context exceeds its block budget".into());
                }
            }
            if let Some(cancel) = cancel {
                let prepared = crate::worker::compute(move || {
                    crate::flow_prepare::prepare_lazy_context(&fragments, cancel)
                })
                .await?;
                return Ok(Reply::SelectableFragment {
                    page,
                    line: index.pages[page].line,
                    prepared: Box::new(prepared),
                });
            }
            Ok(Reply::Fragment {
                page,
                line: index.pages[page].line,
                blocks,
            })
        }
        Request::Table { id, row, columns } => {
            let columns = columns
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let raw = api
                .json(
                    Method::GET,
                    &format!(
                        "{base}/table-source?id={id}&row={row}&columns={columns}&revision={}",
                        session
                            .index
                            .as_ref()
                            .ok_or("Document index missing")?
                            .revision
                    ),
                    None,
                )
                .await?;
            Ok(Reply::Table(
                crate::worker::compute(move || table_page(raw)).await?,
            ))
        }
        Request::Source {
            line,
            offset,
            previous,
        } => {
            let target = offset.map_or_else(
                || format!("line={line}"),
                |offset| format!("offset={offset}&previous={previous}"),
            );
            let raw = api
                .json(Method::GET, &format!("{base}/source?{target}"), None)
                .await?;
            Ok(Reply::Source(decode(raw["result"].clone())?))
        }
        Request::Edit {
            line,
            offset,
            previous,
        } => {
            let target = offset.map_or_else(
                || format!("line={line}"),
                |offset| format!("offset={offset}&previous={previous}"),
            );
            let raw = api
                .json_document(
                    Method::GET,
                    &format!("{base}/edit?{target}"),
                    None,
                    Some(&session.document),
                )
                .await?;
            let page: EditPage = decode(raw)?;
            let buffer = EditBuffer::new(&page.window.text)?;
            session.editor = Some(Editor {
                page: page.clone(),
                buffer,
            });
            session.poisoned = None;
            Ok(Reply::Edit(page))
        }
        Request::Insert { offset, text } => {
            session
                .editor
                .as_mut()
                .ok_or("Editor has no page")?
                .buffer
                .insert(*offset, text)?;
            Ok(Reply::Ack)
        }
        Request::Delete { from, to } => {
            session
                .editor
                .as_mut()
                .ok_or("Editor has no page")?
                .buffer
                .delete(*from, *to)?;
            Ok(Reply::Ack)
        }
        Request::Save { text } => {
            if let Some(error) = &session.poisoned {
                return Err(format!("Reload the edit page before saving: {error}"));
            }
            let editor = session.editor.as_mut().ok_or("Editor has no page")?;
            if editor.buffer.text() != *text {
                return Err(
                    "The editor tracking does not match its buffer; edits were kept in memory"
                        .into(),
                );
            }
            let changes = editor.buffer.changes()?;
            if changes.is_empty() {
                return Ok(Reply::Saved(editor.page.clone()));
            }
            let body = json!({"revision":editor.page.revision,"start":editor.page.window.start,"end":editor.page.window.end,"changes":changes});
            let raw = api
                .json_document(
                    Method::POST,
                    &format!("{base}/edit"),
                    Some(&body),
                    Some(&editor.page.document_id),
                )
                .await?;
            let page: EditPage = decode(raw)?;
            editor.buffer.rebase(&page.window.text)?;
            session.document = page.document_id.clone();
            editor.page = page.clone();
            Ok(Reply::Saved(page))
        }
        Request::Search(query) => {
            let query: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
            let raw = api
                .json(Method::GET, &format!("{base}/search?query={query}"), None)
                .await?;
            Ok(Reply::Search(decode(raw["matches"].clone())?))
        }
        Request::LocateQuote(query) => {
            query.validate()?;
            let index = session
                .index
                .as_ref()
                .ok_or("Wait for the document index")?;
            let raw = api
                .json(
                    Method::POST,
                    &format!("{base}/locate-quote"),
                    Some(&json!({"revision":index.revision,"query":query})),
                )
                .await?;
            if raw["revision"] != index.revision {
                return Err("Comment snapshot changed".into());
            }
            let located: studio_markdown_native::quote::Located = decode(raw["located"].clone())?;
            if located.position.as_ref().is_some_and(|p| {
                p.line == 0
                    || p.line > p.end_line
                    || p.end_line > index.source_lines
                    || p.start >= p.end
                    || p.end > index.source_bytes
            }) || (located.status == studio_markdown_native::quote::Status::Unique)
                != located.position.is_some()
            {
                return Err("Invalid comment quote location".into());
            }
            Ok(Reply::LocatedQuote(located))
        }
        Request::Heartbeat => {
            api.json(Method::GET, &format!("{base}/heartbeat"), None)
                .await?;
            Ok(Reply::Ack)
        }
        Request::Close => {
            api.json(
                Method::POST,
                "/studio/markdown/close",
                Some(&json!({"id":session.reader})),
            )
            .await?;
            sessions.remove(&message.key);
            Ok(Reply::Ack)
        }
        Request::Open { .. }
        | Request::Resume { .. }
        | Request::Diagram { .. }
        | Request::CancelDiagram { .. }
        | Request::CancelDiagrams => unreachable!(),
    }
}

fn decode<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|error| format!("Invalid document response: {error}"))
}

fn preview_pages(index: &Index, target: usize) -> Result<(usize, Vec<usize>), String> {
    let page = index
        .pages
        .get(target)
        .ok_or("Document page is out of range")?;
    let target = page.table.map_or(target, |id| index.tables[id].first_page);
    let logical = |page: usize| {
        index.pages[page]
            .table
            .map_or(page, |id| index.tables[id].first_page)
    };
    let mut pages = Vec::new();
    if index.source_bytes <= 128 * 1024 && index.pages.len() <= 64 {
        for page in 0..index.pages.len() {
            let page = logical(page);
            if pages.last() != Some(&page) {
                pages.push(page);
            }
        }
    } else {
        if target > 0 {
            pages.push(logical(target - 1));
        }
        pages.push(target);
        let next = index.pages[target]
            .table
            .map_or(target + 1, |id| index.tables[id].end_page);
        if next < index.pages.len() {
            pages.push(next);
        }
    }
    Ok((target, pages))
}

async fn selection_prefix(
    api: &Api,
    reader: &str,
    document: &str,
    revision: &str,
    id: usize,
    characters: usize,
    index: &Index,
) -> Result<studio_markdown_native::flow::TablePrefix, String> {
    if document.is_empty()
        || id >= index.tables.len()
        || characters > studio_markdown_native::flow::BUFFER_BYTES / 4
    {
        return Err("Invalid indexed table prefix".into());
    }
    let mut raw=api.json_document(Method::GET,&format!("/studio/native/markdown/{reader}/table-prefix?id={id}&characters={characters}&revision={revision}"),None,Some(document)).await?;
    if raw["documentId"].as_str() != Some(document) || raw["revision"].as_str() != Some(revision) {
        return Err("Selection document snapshot changed".into());
    }
    let prefix: studio_markdown_native::flow::TablePrefix = decode(raw["prefix"].take())?;
    let count = prefix.text.chars().count();
    if prefix.id != id
        || prefix.text.len() > characters * 4
        || count > characters
        || (!prefix.complete && count != characters)
    {
        return Err("Invalid resolved table prefix".into());
    }
    Ok(prefix)
}
async fn selection_tables(
    api: &Api,
    reader: &str,
    document: &str,
    revision: &str,
    ids: &[usize],
    limit: usize,
    index: &Index,
) -> Result<Vec<studio_markdown_native::flow::TableText>, String> {
    if document.is_empty()
        || ids.is_empty()
        || ids.len() > 32
        || ids
            .iter()
            .enumerate()
            .any(|(i, id)| *id >= index.tables.len() || ids[..i].contains(id))
        || limit > studio_markdown_native::flow::BUFFER_BYTES
    {
        return Err("Invalid indexed table selection".into());
    }
    let query = ids
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let mut raw=api.json_document(Method::GET,&format!("/studio/native/markdown/{reader}/table-text?ids={query}&limit={limit}&revision={revision}"),None,Some(document)).await?;
    if raw["documentId"].as_str() != Some(document) || raw["revision"].as_str() != Some(revision) {
        return Err("Selection document snapshot changed".into());
    }
    let tables: Vec<studio_markdown_native::flow::TableText> = decode(raw["tables"].take())?;
    let mut bytes = 0usize;
    if tables.len() != ids.len()
        || tables.iter().enumerate().any(|(i, t)| {
            bytes = bytes.saturating_add(t.text.len());
            t.id != ids[i]
                || bytes > limit
                || t.source.start > t.source.end
                || t.source.end > index.source_bytes
                || t.source.line == 0
                || t.source.end_line < t.source.line
                || t.source.end_line > index.source_lines
        })
    {
        return Err("Invalid resolved table selection".into());
    }
    Ok(tables)
}

fn table_page(raw: Value) -> Result<TablePage, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Data {
        table: usize,
        columns: Vec<usize>,
        row_start: usize,
        row_end: usize,
        page_size: usize,
        total_rows: usize,
        fragments: Vec<Fragment>,
    }
    let data: Data = decode(raw)?;
    if data.columns.is_empty()
        || data.columns.len() > 512
        || data.row_end < data.row_start
        || data.row_end - data.row_start > 50
        || data.fragments.len() > 50
    {
        return Err("Table response exceeds its display budget".into());
    }
    let mut header = Vec::new();
    let mut rows = Vec::new();
    for fragment in data.fragments {
        for block in studio_markdown_native::parse(&fragment)? {
            if let BlockKind::Table {
                header: cells,
                rows: records,
                row_start,
                ..
            } = block.kind
            {
                if header.is_empty() {
                    header = select_cells(&cells, &data.columns)?;
                }
                for (index, row) in records.into_iter().enumerate() {
                    if (data.row_start..data.row_end).contains(&(row_start + index)) {
                        rows.push(select_cells(&row.cells, &data.columns)?);
                    }
                }
            }
        }
    }
    if rows.len() != data.row_end - data.row_start {
        return Err("Table response is missing source rows".into());
    }
    Ok(TablePage {
        id: data.table,
        columns: data.columns,
        start: data.row_start,
        end: data.row_end,
        page_size: data.page_size,
        total: data.total_rows,
        header,
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn selection_reads_leave_controls_free_and_cancel_when_the_offer_closes() {
        selection_read_case(false).await;
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn prefix_reads_leave_controls_free_and_cancel_when_the_offer_closes() {
        selection_read_case(true).await;
    }
    async fn selection_read_case(prefix: bool) {
        use std::time::Duration;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (seen, seen_rx) = tokio::sync::oneshot::channel();
        let (closed, closed_rx) = tokio::sync::oneshot::channel();
        let seen = Arc::new(tokio::sync::Mutex::new(Some(seen)));
        let closed = Arc::new(tokio::sync::Mutex::new(Some(closed)));
        let revision = "a".repeat(64);
        let revision_server = revision.clone();
        let server = tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let (seen, closed, revision) =
                    (seen.clone(), closed.clone(), revision_server.clone());
                tokio::spawn(async move {
                    let mut bytes = Vec::new();
                    let mut part = [0u8; 2048];
                    let end = loop {
                        let n = socket.read(&mut part).await.unwrap();
                        if n == 0 {
                            return;
                        }
                        bytes.extend_from_slice(&part[..n]);
                        assert!(bytes.len() < 65536);
                        if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                            break end + 4;
                        }
                    };
                    let head = String::from_utf8_lossy(&bytes[..end]);
                    let path = head
                        .lines()
                        .next()
                        .unwrap()
                        .split_whitespace()
                        .nth(1)
                        .unwrap()
                        .to_string();
                    let length = head
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|n| n.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    while bytes.len() < end + length {
                        let n = socket.read(&mut part).await.unwrap();
                        if n == 0 {
                            return;
                        }
                        bytes.extend_from_slice(&part[..n]);
                    }
                    if path.contains("/table-text?") || path.contains("/table-prefix?") {
                        if let Some(seen) = seen.lock().await.take() {
                            let _ = seen.send(());
                        }
                        let ended = matches!(
                            tokio::time::timeout(Duration::from_secs(2), socket.read(&mut part))
                                .await,
                            Ok(Ok(0))
                        );
                        if let Some(closed) = closed.lock().await.take() {
                            let _ = closed.send(ended);
                        }
                        return;
                    }
                    let body = if path.starts_with("/studio/native/markdown/open") {
                        json!({"readerId":"selection-reader","readOnly":true,"root":"/private","path":"/private/file.md"})
                    } else if path.ends_with("/document") {
                        json!({"documentId":"selection-document","state":"ready","index":{
                        "name":"private.md","revision":revision,"sourceBytes":100,"sourceLines":10,"toc":[],
                        "pages":[{"start":0,"end":100,"line":1,"rows":2,"rowStart":0,"table":0}],
                        "tables":[{"firstPage":0,"endPage":1,"rows":2,"columns":["H"]}]}})
                    } else if path.contains("/source?") {
                        json!({"result":{"start":0,"end":6,"line":1,"endLine":1,"text":"hello\n"}})
                    } else {
                        panic!("Unexpected private API route {path}")
                    };
                    let body = body.to_string();
                    let response=format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body);
                    socket.write_all(response.as_bytes()).await.unwrap();
                });
            }
        });
        let api = Api::from_access_url(&format!(
            "http://{address}/#token=0123456789abcdef0123456789abcdef"
        ))
        .unwrap();
        let (input, requests) = mpsc::channel(8);
        let (output, responses) = async_channel::bounded(8);
        let runner = tokio::spawn(run(api, requests, output));
        let message = |serial, request| Message {
            key: 1,
            generation: 1,
            serial,
            request,
        };
        input
            .send(message(
                1,
                Request::Open {
                    root: "/private".into(),
                    path: "/private/file.md".into(),
                    read_only: true,
                },
            ))
            .await
            .unwrap();
        assert!(matches!(
            responses.recv().await.unwrap(),
            Event::Document(Response {
                result: Ok(Reply::Open { .. }),
                ..
            })
        ));
        input.send(message(2, Request::Index)).await.unwrap();
        assert!(matches!(
            responses.recv().await.unwrap(),
            Event::Document(Response {
                result: Ok(Reply::Index(Some(_))),
                ..
            })
        ));
        let (request, receive): (Request, Box<dyn std::any::Any + Send>) = if prefix {
            let (reply, receive) = tokio::sync::oneshot::channel();
            (
                Request::SelectionPrefix {
                    reader: "selection-reader".into(),
                    revision,
                    id: 0,
                    characters: 1024,
                    reply,
                },
                Box::new(receive),
            )
        } else {
            let (reply, receive) = tokio::sync::oneshot::channel();
            (
                Request::SelectionTables {
                    reader: "selection-reader".into(),
                    revision,
                    ids: vec![0],
                    limit: 1024,
                    reply,
                },
                Box::new(receive),
            )
        };
        input.send(message(3, request)).await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), seen_rx)
            .await
            .unwrap()
            .unwrap();
        input
            .send(message(
                4,
                Request::Source {
                    line: 1,
                    offset: None,
                    previous: false,
                },
            ))
            .await
            .unwrap();
        let response = tokio::time::timeout(Duration::from_secs(1), responses.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(
            matches!(response,Event::Document(Response{result:Ok(Reply::Source(source)),..}) if source.text=="hello\n")
        );
        drop(receive);
        assert!(tokio::time::timeout(Duration::from_secs(2), closed_rx)
            .await
            .unwrap()
            .unwrap());
        drop(input);
        runner.await.unwrap();
        server.abort();
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn document_controls_are_processed_while_a_diagram_engine_is_busy() {
        use std::{
            sync::atomic::{AtomicBool, Ordering},
            time::{Duration, Instant},
        };
        let api =
            Api::from_access_url("http://127.0.0.1:1/#token=0123456789abcdef0123456789abcdef")
                .unwrap();
        let (output, responses) = async_channel::bounded(8);
        let started = Arc::new(AtomicBool::new(false));
        let rendering = started.clone();
        let queue = crate::diagram_queue::Queue::with_renderer(
            output.clone(),
            Arc::new(move |_, _, _, _, cancel| {
                rendering.store(true, Ordering::Relaxed);
                let begin = Instant::now();
                while !cancel.load(Ordering::Relaxed) && begin.elapsed() < Duration::from_secs(3) {
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err("cancelled".into())
            }),
        );
        let (input, commands) = mpsc::channel(8);
        let task = tokio::spawn(run_with_queue(api, commands, output, queue));
        input
            .send(Message {
                key: 1,
                generation: 1,
                serial: 1,
                request: Request::Diagram {
                    id: 0,
                    kind: "test".into(),
                    source: "slow".into(),
                    dark: false,
                    style: Default::default(),
                },
            })
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while !started.load(Ordering::Relaxed) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        input
            .send(Message {
                key: 1,
                generation: 1,
                serial: 2,
                request: Request::Heartbeat,
            })
            .await
            .unwrap();
        let response = tokio::time::timeout(Duration::from_secs(1), responses.recv())
            .await
            .expect("diagram engine blocked document controls")
            .unwrap();
        assert!(matches!(
            response,
            Event::Document(Response {
                serial: 2,
                kind: "heartbeat",
                ..
            })
        ));
        input
            .send(Message {
                key: 1,
                generation: 1,
                serial: 3,
                request: Request::Close,
            })
            .await
            .unwrap();
        let response = tokio::time::timeout(Duration::from_secs(1), responses.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            response,
            Event::Document(Response {
                serial: 3,
                kind: "close",
                ..
            })
        ));
        drop(input);
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap();
    }
}

fn select_cells(cells: &[Cell], columns: &[usize]) -> Result<Vec<Cell>, String> {
    columns
        .iter()
        .map(|column| {
            cells
                .get(*column)
                .cloned()
                .ok_or_else(|| "Table column is missing".into())
        })
        .collect()
}
