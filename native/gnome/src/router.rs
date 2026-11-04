//! Durable Router jobs. Network, projection and rich-text parsing stay off GTK.
use crate::{
    gateway::Api,
    model::{Item, Origin},
};
use futures_util::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Registry {
    pub revision: String,
    pub controller_backend: String,
    pub controllers: BTreeMap<String, String>,
}
impl Registry {
    pub fn contains(&self, key: &str) -> bool {
        self.controllers
            .iter()
            .any(|(backend, id)| !id.is_empty() && key == format!("{backend}:{id}"))
    }
    pub fn active(&self) -> Option<String> {
        self.controllers
            .get(&self.controller_backend)
            .filter(|id| !id.is_empty())
            .map(|id| format!("{}:{id}", self.controller_backend))
    }
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub session_key: String,
    pub backend: String,
    pub id: String,
    pub title: String,
    pub cwd: String,
}
#[derive(Debug, Deserialize)]
pub struct Choices {
    pub controller: String,
    pub revision: String,
    pub targets: Vec<Target>,
    pub warnings: Vec<String>,
    #[serde(skip)]
    folded: Vec<String>,
}
impl Choices {
    fn prepare(mut self, controller: &str) -> Result<Self, String> {
        if self.controller != controller
            || self.revision.len() > 64
            || self.targets.len() > 2048
            || self.warnings.len() > 64
        {
            return Err("Invalid Router target inventory".into());
        }
        let mut keys = std::collections::HashSet::new();
        for target in &self.targets {
            if target.session_key != format!("{}:{}", target.backend, target.id)
                || target.session_key.len() > 320
                || target.id.is_empty()
                || target.id.len() > 256
                || target.session_key.chars().any(char::is_control)
                || target.cwd.len() > 4096
                || target.title.chars().count() > 300
                || !keys.insert(target.session_key.clone())
            {
                return Err("Invalid Router target identity".into());
            }
            self.folded.push(
                format!("{}\n{}\n{}", target.title, target.session_key, target.cwd).to_lowercase(),
            );
        }
        Ok(self)
    }
    pub fn page(self: Arc<Self>, query: String, offset: usize) -> Result<ChoicePage, String> {
        if query.len() > 2048 {
            return Err("Search supports at most 512 characters".into());
        }
        let needle = query.to_lowercase();
        let matched = self
            .folded
            .iter()
            .enumerate()
            .filter_map(|(index, text)| text.contains(&needle).then_some(index))
            .collect::<Vec<_>>();
        let total = matched.len();
        let offset = offset.min(total.saturating_sub(1) / 50 * 50);
        let indices = matched.into_iter().skip(offset).take(50).collect();
        Ok(ChoicePage {
            catalog: self,
            query,
            offset,
            total,
            indices,
        })
    }
}
#[derive(Debug)]
pub struct ChoicePage {
    pub catalog: Arc<Choices>,
    pub query: String,
    pub offset: usize,
    pub total: usize,
    pub indices: Vec<usize>,
}
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Decision {
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub forwarded_prompt: String,
}
#[derive(Clone, Debug, Deserialize)]
pub struct ResponseItem {
    pub id: String,
    pub text: String,
    pub excerpt: bool,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    #[serde(default)]
    pub browser_history: bool,
    #[serde(default)]
    pub supervision: Option<Supervision>,
    #[serde(default)]
    pub map_notice: String,
    #[serde(default)]
    pub dispatch_notice: String,
    #[serde(default)]
    pub reference_labels: Vec<String>,
    #[serde(default)]
    pub image_count: usize,
    #[serde(default)]
    pub receipt: String,
    pub id: String,
    pub controller: String,
    pub prompt: String,
    pub status: String,
    pub target: Option<Target>,
    pub target_turn_id: String,
    pub decision: Option<Decision>,
    pub response: String,
    #[serde(default)]
    pub response_items: Vec<ResponseItem>,
    pub error: String,
    pub unread: bool,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Supervision {
    pub previous_turn_id: String,
    pub prompt: String,
}
impl Job {
    pub fn terminal(&self) -> bool {
        self.browser_history
            || matches!(
                self.status.as_str(),
                "completed" | "clarify" | "failed" | "stopped"
            )
    }
}
#[derive(Debug)]
pub struct Display {
    pub job: Job,
    pub items: Vec<Arc<Item>>,
}
#[derive(Clone, Debug)]
pub struct History {
    pub statuses: Vec<Tracked>,
    pub controller: String,
    pub ids: Vec<String>,
    pub next: Option<i64>,
    pub newer: Option<i64>,
    pub unread_count: u64,
    pub active_ids: Vec<String>,
    pub controller_busy: bool,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Tracked {
    pub controller: String,
    pub id: String,
    pub status: Option<String>,
    #[serde(default, rename = "targetTurnId")]
    pub target_turn_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageSpec {
    pub browser: bool,
    pub controller: String,
    pub before: Option<i64>,
    pub after: Option<i64>,
    pub query: String,
    pub case_sensitive: bool,
}
impl PageSpec {
    pub fn latest(controller: String) -> Self {
        Self {
            browser: false,
            controller,
            before: None,
            after: None,
            query: String::new(),
            case_sensitive: false,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Receipt {
    pub id: String,
    pub receipt: String,
}
#[derive(Debug)]
pub enum Request {
    PrepareFiles {
        paths: Vec<std::path::PathBuf>,
        tag: u64,
    },
    Choices(String),
    FilterChoices {
        catalog: Arc<Choices>,
        query: String,
        offset: usize,
    },
    Sync {
        known: HashMap<String, String>,
        focus: Option<PageSpec>,
        tracking: Vec<(String, String)>,
    },
    Page(PageSpec),
    Target {
        kind: String,
        origin: Arc<Origin>,
    },
    Start {
        body: Value,
        images: Vec<Arc<crate::attachments::Image>>,
        references: Vec<Arc<crate::composer_refs::Reference>>,
        queued_references: Option<Arc<Vec<Value>>>,
    },
    Stop(String),
    Seen {
        controller: String,
        seen: Vec<Receipt>,
    },
}
#[derive(Debug)]
pub enum Reply {
    Files(Vec<Arc<crate::composer_refs::Reference>>),
    Choices(Arc<Choices>),
    ChoicePage(ChoicePage),
    Sync {
        registry: Registry,
        histories: Vec<History>,
        changed: Vec<Display>,
        stamps: HashMap<String, String>,
        view: Option<(PageSpec, History)>,
        tracking: Vec<Tracked>,
    },
    Page {
        spec: PageSpec,
        history: History,
        changed: Vec<Display>,
        stamps: HashMap<String, String>,
    },
    Target {
        thread: crate::model::Thread,
        origin: Arc<Origin>,
    },
    Started(Display),
    Stopped,
    Seen {
        controller: String,
        acknowledged: Vec<Receipt>,
    },
    Rejected(String),
}
fn query(key: &str, value: &str) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .append_pair(key, value)
        .finish()
}
async fn decode<T: serde::de::DeserializeOwned + Send + 'static>(
    value: Value,
) -> Result<T, String> {
    crate::worker::compute(move || {
        serde_json::from_value(value).map_err(|_| "Invalid Router response".into())
    })
    .await
}
async fn display(value: Value) -> Result<Display, String> {
    crate::worker::compute(move || {
        project(serde_json::from_value(value).map_err(|_| "Invalid Router job")?)
    })
    .await
}
pub async fn execute(api: &Api, request: Request) -> Result<Reply, String> {
    use reqwest::Method;
    match request {
        Request::PrepareFiles { paths, tag } => {
            crate::worker::compute(move || {
                crate::composer_refs::prepare_files(paths, tag).map(Reply::Files)
            })
            .await
        }
        Request::Choices(controller) => {
            let raw = api
                .json(
                    Method::GET,
                    &format!(
                        "/studio/native/router/targets?{}",
                        query("controller", &controller)
                    ),
                    None,
                )
                .await?;
            crate::worker::compute(move || {
                let choices: Choices =
                    serde_json::from_value(raw).map_err(|_| "Invalid Router target inventory")?;
                Ok(Reply::Choices(Arc::new(choices.prepare(&controller)?)))
            })
            .await
        }
        Request::FilterChoices {
            catalog,
            query,
            offset,
        } => {
            crate::worker::compute(move || catalog.page(query, offset).map(Reply::ChoicePage)).await
        }
        Request::Target { kind, origin } => {
            let value=match kind.as_str(){
                "codex"=>api.json(Method::POST,"/studio/native/conversation/read",Some(&json!({"backend":origin.backend,"threadId":origin.thread,"operation":"summary"}))).await?,
                "opencode"=>{
                    let raw=api.json(Method::GET,&crate::opencode::session_metadata_path(&origin.thread,&origin.root),None).await?;
                    json!({"thread":{"id":raw["id"],"title":raw["title"],"cwd":raw["directory"],"parentThreadId":raw["parentID"],"archived":raw.pointer("/time/archived").and_then(Value::as_i64).is_some_and(|n|n>0)}})
                },_=>return Err("The response backend is unavailable".into())
            };
            crate::worker::compute(move || {
                let thread = target_metadata(&value["thread"], &origin)?;
                Ok(Reply::Target { thread, origin })
            })
            .await
        }
        Request::Start {
            body,
            images,
            references,
            queued_references,
        } => {
            let body = crate::worker::compute(move || {
                let mut body = body;
                if let Some(input) = queued_references.filter(|input| !input.is_empty()) {
                    body["references"] = Value::Array(input.as_ref().clone());
                }
                if !images.is_empty() {
                    body["images"] = Value::Array(
                        images
                            .iter()
                            .map(|image| image.input["url"].clone())
                            .collect(),
                    );
                }
                if !references.is_empty() {
                    body["references"] = Value::Array(
                        references
                            .iter()
                            .map(|reference| reference.input.clone())
                            .collect(),
                    );
                }
                Ok(body)
            })
            .await?;
            let (status, value) = api
                .json_status(Method::POST, "/studio/native/router/jobs", &body)
                .await?;
            if status.is_client_error() {
                return Ok(Reply::Rejected(crate::gateway::response_error(
                    status, &value,
                )));
            }
            if !status.is_success() {
                return Err("Router delivery result is unknown; retry the same delivery to check its identity".into());
            }
            let display = display(value).await?;
            if body["requestId"].as_str() != Some(display.job.id.as_str())
                || body["controller"].as_str() != Some(display.job.controller.as_str())
            {
                return Err("Router acknowledgement does not match the delivery identity".into());
            }
            Ok(Reply::Started(display))
        }
        Request::Seen { controller, seen } => {
            let raw = api
                .json(
                    Method::POST,
                    "/studio/native/router/seen",
                    Some(&json!({"controller":controller,"seen":seen})),
                )
                .await?;
            #[derive(Deserialize)]
            struct Ack {
                acknowledged: Vec<Receipt>,
            }
            let reply: Ack = decode(raw).await?;
            if reply.acknowledged.len() > 20
                || reply.acknowledged.iter().any(|ack| {
                    !seen
                        .iter()
                        .any(|s| s.id == ack.id && s.receipt == ack.receipt)
                })
            {
                return Err("Router read acknowledgement does not match the request".into());
            }
            Ok(Reply::Seen {
                controller,
                acknowledged: reply.acknowledged,
            })
        }
        Request::Stop(id) => {
            api.json(
                Method::POST,
                "/studio/native/router/stop",
                Some(&json!({"id":id})),
            )
            .await?;
            Ok(Reply::Stopped)
        }
        Request::Page(spec) => {
            let (history, stamps) = load_history(api, &spec).await?;
            let changed = load_changes(api, history.ids.clone()).await?;
            Ok(Reply::Page {
                spec,
                history,
                changed,
                stamps,
            })
        }
        Request::Sync {
            known,
            focus,
            tracking,
        } => {
            if tracking.len() > 128 {
                return Err("Too many Router queue gates".into());
            }
            let registry: Registry =
                decode(api.json(Method::GET, "/studio/native/router", None).await?).await?;
            if registry.controllers.len() > 64 {
                return Err("Too many Router controllers to monitor".into());
            }
            let mut histories = Vec::new();
            let mut view = None;
            let mut stamps = HashMap::new();
            for (backend, id) in &registry.controllers {
                if id.is_empty() {
                    continue;
                }
                let spec = PageSpec::latest(format!("{backend}:{id}"));
                let (history, versions) = load_history(api, &spec).await?;
                if focus.as_ref() == Some(&spec) {
                    stamps = versions;
                    view = Some((spec, history.clone()));
                }
                histories.push(history);
            }
            if let Some(spec) = focus.filter(|s| {
                registry.contains(&s.controller)
                    && (s.browser || s.before.is_some() || s.after.is_some() || !s.query.is_empty())
            }) {
                let (history, versions) = load_history(api, &spec).await?;
                stamps = versions;
                view = Some((spec, history));
            }
            // Keep full replies only for the displayed page. Other controllers
            // contribute indexed attention/activity metadata, not hidden rich trees.
            let changed_ids = stamps
                .iter()
                .filter(|(id, stamp)| known.get(*id) != Some(*stamp))
                .map(|(id, _)| id.clone())
                .collect();
            let changed = load_changes(api, changed_ids).await?;
            let tracking = stream::iter(tracking.into_iter().map(|(controller, id)| async move {
                let path = format!(
                    "/studio/native/router/jobs?{}&{}&statusOnly=true",
                    query("controller", &controller),
                    query("id", &id)
                );
                let row: Tracked = decode(api.json(Method::GET, &path, None).await?).await?;
                if row.controller != controller || row.id != id {
                    return Err("Router queue status identity changed".into());
                }
                Ok(row)
            }))
            .buffer_unordered(2)
            .collect::<Vec<Result<Tracked, String>>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
            Ok(Reply::Sync {
                registry,
                histories,
                changed,
                stamps,
                view,
                tracking,
            })
        }
    }
}
async fn load_history(
    api: &Api,
    spec: &PageSpec,
) -> Result<(History, HashMap<String, String>), String> {
    let mut path = format!(
        "{}?{}",
        if spec.browser {
            "/studio/native/router/browser-history"
        } else {
            "/studio/native/router/jobs"
        },
        query("controller", &spec.controller)
    );
    if let Some(cursor) = spec.before {
        path.push_str(&format!("&before={cursor}"));
    }
    if let Some(cursor) = spec.after {
        path.push_str(&format!("&after={cursor}"));
    }
    if !spec.query.is_empty() {
        path.push('&');
        path.push_str(&query("query", &spec.query));
        path.push_str(if spec.case_sensitive {
            "&caseSensitive=true"
        } else {
            "&caseSensitive=false"
        });
    }
    let value = api.json(reqwest::Method::GET, &path, None).await?;
    if spec.browser && value["browserHistory"] != true {
        return Err("Browser history scope changed".into());
    }
    if !spec.query.is_empty()
        && (value["query"].as_str() != Some(spec.query.as_str())
            || value["caseSensitive"].as_bool() != Some(spec.case_sensitive))
    {
        return Err("Router history search scope changed".into());
    }
    let controller = spec.controller.clone();
    crate::worker::compute(move || {
        let rows = value["data"].as_array().ok_or("Invalid Router history")?;
        if rows.len() > 20 {
            return Err("Router history exceeds its page budget".into());
        }
        let mut statuses = Vec::new();
        let mut ids = Vec::new();
        let mut versions = HashMap::new();
        for row in rows {
            let id = row["id"]
                .as_str()
                .filter(|id| !id.is_empty() && id.len() <= 64)
                .ok_or("Invalid Router job identity")?
                .to_owned();
            if versions
                .insert(
                    id.clone(),
                    format!(
                        "{}:{}",
                        row["receipt"]
                            .as_str()
                            .filter(|r| r.len() == 32)
                            .ok_or("Invalid Router receipt")?,
                        row["unread"]
                            .as_bool()
                            .ok_or("Invalid Router unread state")?
                    ),
                )
                .is_some()
            {
                return Err("Repeated Router history identity".into());
            }
            if row["controller"].as_str() != Some(controller.as_str()) {
                return Err("Router summary controller identity changed".into());
            }
            let status = row["status"]
                .as_str()
                .ok_or("Invalid Router summary status")?;
            statuses.push(Tracked {
                controller: controller.clone(),
                id: id.clone(),
                status: Some(status.into()),
                target_turn_id: String::new(),
            });
            ids.push(id);
        }
        let unread_count = value["unreadCount"]
            .as_u64()
            .ok_or("Invalid Router unread count")?;
        let active_ids: Vec<String> = serde_json::from_value(value["activeIds"].clone())
            .map_err(|_| "Invalid Router active identities")?;
        if active_ids.len() > 4 {
            return Err("Too many active Router records".into());
        }
        let controller_busy = value["controllerBusy"]
            .as_bool()
            .ok_or("Invalid Router controller status")?;
        Ok((
            History {
                statuses,
                controller,
                ids,
                next: value["nextCursor"].as_i64(),
                newer: value["newerCursor"].as_i64(),
                unread_count,
                active_ids,
                controller_busy,
            },
            versions,
        ))
    })
    .await
}
async fn load_changes(api: &Api, ids: Vec<String>) -> Result<Vec<Display>, String> {
    stream::iter(ids.into_iter().map(|id| async move {
        display(
            api.json(
                reqwest::Method::GET,
                &format!(
                    "{}?{}",
                    if id.starts_with("browser-") {
                        "/studio/native/router/browser-history"
                    } else {
                        "/studio/native/router/jobs"
                    },
                    query("id", &id)
                ),
                None,
            )
            .await?,
        )
        .await
    }))
    .buffer_unordered(2)
    .collect::<Vec<_>>()
    .await
    .into_iter()
    .collect()
}

fn target_metadata(raw: &Value, origin: &Origin) -> Result<crate::model::Thread, String> {
    let thread = crate::model::Thread::from_value(raw).ok_or("Invalid target session metadata")?;
    if thread.id != origin.thread || thread.cwd != origin.root {
        return Err(
            "The target identity or project directory changed; choose it from the session list"
                .into(),
        );
    }
    if raw["archived"] == true {
        return Err("The target is archived; open it from the archive view".into());
    }
    Ok(thread)
}
fn item(id: String, turn: &str, role: &str, body: String) -> Item {
    let mut item =
        Item::from_value(&json!({"id":id,"turnId":turn,"type":"agentMessage","text":body}));
    item.role = role.into();
    item
}
fn rich(mut item: Item) -> Arc<Item> {
    item.rich = crate::chat_content::Content::parse(&item.body).ok();
    Arc::new(item)
}
fn project(job: Job) -> Result<Display, String> {
    if job.id.len() > 64
        || job.image_count > 4
        || job.reference_labels.len() > 32
        || job.reference_labels.iter().map(String::len).sum::<usize>() > 32 * 1024
        || job.prompt.len() > 64 * 1024
        || job.response.len() > 40 * 1024
        || job.response_items.len() > 128
        || job.dispatch_notice.len() > 1024
        || job.map_notice.len() > 4096
    {
        return Err("Router job exceeds its display budget".into());
    }
    let mut question = job.prompt.clone();
    if !job.reference_labels.is_empty() {
        question.push_str("\n\n");
        question.push_str(&job.reference_labels.join("\n"));
    }
    let mut items = vec![rich(item(
        format!("router:{}:question", job.id),
        &job.id,
        "You",
        if job.image_count == 0 {
            question.clone()
        } else {
            format!(
                "{}\n\n[{} attached image{}]",
                question,
                job.image_count,
                if job.image_count == 1 { "" } else { "s" }
            )
        },
    ))];
    let mut status = job
        .target
        .as_ref()
        .map(|target| format!("{} · {}", target.title, target.backend))
        .unwrap_or_else(|| "Thread Router".into());
    status.push_str(&format!(" · {}", job.status));
    if !job.map_notice.is_empty() {
        status.push('\n');
        status.push_str(&job.map_notice);
    }
    if (job.browser_history || job.status == "dispatching") && !job.dispatch_notice.is_empty() {
        status.push('\n');
        status.push_str(&job.dispatch_notice);
    }
    if let Some(decision) = &job.decision {
        if !decision.reason.is_empty() {
            status.push_str(&format!("\n{}", decision.reason));
        }
    }
    if !job.error.is_empty() {
        status.push_str(&format!("\n{}", job.error));
    }
    if job.status == "clarify" {
        status.push_str(&format!("\n{}", job.response));
    }
    items.push(rich(item(
        format!("router:{}:status", job.id),
        &job.id,
        "Router",
        status,
    )));
    if let Some(target) = &job.target {
        if target.session_key != format!("{}:{}", target.backend, target.id) {
            return Err("Router response source identity disagrees".into());
        }
        let fallback;
        let responses = if job.response_items.is_empty() && !job.response.is_empty() {
            fallback = vec![ResponseItem {
                id: String::new(),
                text: job.response.clone(),
                excerpt: true,
            }];
            &fallback
        } else {
            &job.response_items
        };
        if let Some(s) = &job.supervision {
            if s.previous_turn_id.is_empty()
                || s.previous_turn_id.len() > 256
                || s.previous_turn_id == job.target_turn_id
                || s.previous_turn_id.chars().any(char::is_control)
                || !s.prompt.starts_with("[Studio turn supervisor]\n")
                || s.prompt.len() > 16 * 1024 + 64
            {
                return Err("Invalid supervised Router source provenance".into());
            }
        }
        let question = Arc::new(
            job.supervision
                .as_ref()
                .map(|s| s.prompt.clone())
                .or_else(|| {
                    job.decision
                        .as_ref()
                        .map(|d| d.forwarded_prompt.clone())
                        .filter(|s| !s.is_empty())
                })
                .unwrap_or_else(|| job.prompt.clone()),
        );
        let mut bytes = 0usize;
        for (index, response) in responses.iter().enumerate() {
            bytes = bytes.saturating_add(response.text.len());
            if bytes > 40 * 1024 || response.id.len() > 256 {
                return Err("Router source excerpt exceeds its display budget".into());
            }
            let mut result = item(
                format!("router:{}:reply:{index}", job.id),
                &job.id,
                "Assistant",
                response.text.clone(),
            );
            result.origin = Some(Arc::new(Origin {
                backend: target.backend.clone(),
                thread: target.id.clone(),
                title: target.title.clone(),
                root: target.cwd.clone(),
                turn: job.target_turn_id.clone(),
                item: response.id.clone(),
                question: question.clone(),
                excerpt: response.excerpt,
            }));
            items.push(rich(result));
        }
    }
    Ok(Display { job, items })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn target_search_pages_unicode_paths_and_backend_scopes_without_replacing_identities() {
        let mut targets=(0..121).map(|index|json!({"sessionKey":format!("codex:{index}"),"id":index.to_string(),"backend":"codex","title":format!("文档 {index}"),"cwd":"/项目/阅读"})).collect::<Vec<_>>();
        targets.push(json!({"sessionKey":"opencode:0","id":"0","backend":"opencode","title":"OTHER","cwd":"/other"}));
        let raw = json!({"controller":"codex:router","revision":"current","targets":targets,"warnings":[]});
        let catalog = Arc::new(
            serde_json::from_value::<Choices>(raw.clone())
                .unwrap()
                .prepare("codex:router")
                .unwrap(),
        );
        let first = catalog.clone().page("文档".into(), 0).unwrap();
        assert_eq!(
            (first.total, first.indices.len(), first.offset),
            (121, 50, 0)
        );
        let middle = catalog.clone().page("文档".into(), 50).unwrap();
        assert_eq!(middle.indices[0], 50);
        let last = catalog.clone().page("文档".into(), 1000).unwrap();
        assert_eq!((last.offset, last.indices.len()), (100, 21));
        let scope = catalog.clone().page("OPENCODE:0".into(), 0).unwrap();
        assert_eq!(scope.indices, [121]);
        assert_eq!(
            catalog.clone().page("/项目/阅读".into(), 0).unwrap().total,
            121
        );
        assert_eq!(catalog.clone().page("absent".into(), 50).unwrap().offset, 0);
        assert!(catalog.page("x".repeat(2049), 0).is_err());
        assert!(serde_json::from_value::<Choices>(raw.clone())
            .unwrap()
            .prepare("opencode:router")
            .is_err());
        let mut invalid = raw.clone();
        invalid["targets"][0]["cwd"] = "x".repeat(4097).into();
        assert!(serde_json::from_value::<Choices>(invalid)
            .unwrap()
            .prepare("codex:router")
            .is_err());
        let mut duplicate = raw;
        duplicate["targets"][1] = duplicate["targets"][0].clone();
        assert!(serde_json::from_value::<Choices>(duplicate)
            .unwrap()
            .prepare("codex:router")
            .is_err());
    }
    fn job() -> Job {
        serde_json::from_value(json!({"id":"request-1","controller":"codex:router","prompt":"Original query","status":"completed","target":{"sessionKey":"opencode:target","backend":"opencode","id":"target","title":"Target","cwd":"/target"},"targetTurnId":"user-message","decision":{"forwardedPrompt":"Actual target question"},"response":"answer","responseItems":[{"id":"answer-part","text":"# Answer 中文😀","excerpt":false}],"error":"","unread":true})).unwrap()
    }
    #[test]
    fn routed_projection_keeps_display_group_and_actual_source_distinct() {
        let display = project(job()).unwrap();
        let item = display.items.last().unwrap();
        assert_eq!(item.turn_id, "request-1");
        assert!(item.rich.is_some());
        let origin = item.origin.as_ref().unwrap();
        assert_eq!(
            (&origin.backend, &origin.thread, &origin.root),
            (&"opencode".into(), &"target".into(), &"/target".into())
        );
        assert_eq!(
            (&origin.turn, &origin.item, origin.question.as_ref()),
            (
                &"user-message".into(),
                &"answer-part".into(),
                &"Actual target question".into()
            )
        );
        assert!(!origin.excerpt);
        let mut other = (**item).clone();
        Arc::make_mut(other.origin.as_mut().unwrap()).root = "/elsewhere".into();
        assert_ne!(**item, other);
    }
    #[test]
    fn browser_projection_is_passive_and_keeps_verified_source_anchors() {
        let mut saved = job();
        saved.id = "browser-1-00000000000000000000000000000000".into();
        saved.browser_history = true;
        saved.status = "running".into();
        saved.dispatch_notice = "Read-only browser history".into();
        assert!(saved.terminal());
        let projected = project(saved).unwrap();
        assert!(projected
            .items
            .iter()
            .any(|i| i.body.contains("Read-only browser history")));
        let origin = projected.items.last().unwrap().origin.as_ref().unwrap();
        assert_eq!(origin.thread, "target");
        assert_eq!(origin.turn, "user-message");
        assert_eq!(origin.item, "answer-part");
        assert_eq!(origin.question.as_str(), "Actual target question");
    }
    #[test]
    fn supervised_projection_anchors_only_the_new_reply_to_its_actual_continuation() {
        let original = project(job()).unwrap();
        let mut next = job();
        next.target_turn_id = "supervised-next".into();
        next.supervision = Some(Supervision {
            previous_turn_id: "user-message".into(),
            prompt: "[Studio turn supervisor]\nFinish the remaining work".into(),
        });
        next.response_items[0].id = "next-answer".into();
        next.response_items[0].text = "New response".into();
        let display = project(next).unwrap();
        let reply = display.items.last().unwrap();
        let origin = reply.origin.as_ref().unwrap();
        assert_eq!(origin.turn, "supervised-next");
        assert_eq!(origin.item, "next-answer");
        assert_eq!(
            origin.question.as_str(),
            "[Studio turn supervisor]\nFinish the remaining work"
        );
        assert_eq!(
            original.items.last().unwrap().origin.as_ref().unwrap().turn,
            "user-message"
        );
        assert_eq!(
            display.items.first().unwrap(),
            original.items.first().unwrap()
        );
        let mut invalid = job();
        invalid.supervision = Some(Supervision {
            previous_turn_id: invalid.target_turn_id.clone(),
            prompt: "[Studio turn supervisor]\nDifferent input".into(),
        });
        assert!(project(invalid).is_err());
    }
    #[test]
    fn legacy_excerpts_never_invent_message_anchors_and_budgets_fail_explicitly() {
        let mut old = job();
        old.response_items.clear();
        let display = project(old).unwrap();
        let origin = display.items.last().unwrap().origin.as_ref().unwrap();
        assert!(origin.item.is_empty() && origin.excerpt);
        let mut invalid = job();
        invalid.target.as_mut().unwrap().session_key = "codex:target".into();
        assert!(project(invalid).is_err());
        let mut oversized = job();
        oversized.response_items[0].text = "x".repeat(41 * 1024);
        assert!(project(oversized).is_err());
    }
    #[test]
    fn clarification_has_no_target_source_or_assistant_favorite() {
        let mut value = job();
        value.status = "clarify".into();
        value.target = None;
        value.response_items.clear();
        value.response = "Which session?".into();
        let display = project(value).unwrap();
        assert!(display
            .items
            .iter()
            .all(|item| item.origin.is_none() && item.role != "Assistant"));
        assert!(display.items[1].body.contains("Which session?"));
        assert_eq!(
            query("controller", "opencode:id ?中文"),
            "controller=opencode%3Aid+%3F%E4%B8%AD%E6%96%87"
        );
    }
    #[test]
    fn target_metadata_rejects_identity_directory_and_archive_changes_before_navigation() {
        let display = project(job()).unwrap();
        let origin = display.items.last().unwrap().origin.as_ref().unwrap();
        let valid = json!({"id":"target","cwd":"/target","title":"Renamed target"});
        assert_eq!(
            target_metadata(&valid, origin).unwrap().title,
            "Renamed target"
        );
        assert!(target_metadata(&json!({"id":"different","cwd":"/target"}), origin).is_err());
        assert!(target_metadata(&json!({"id":"target","cwd":"/changed"}), origin).is_err());
        assert!(target_metadata(
            &json!({"id":"target","cwd":"/target","archived":true}),
            origin
        )
        .is_err());
        assert!(
            crate::opencode::session_metadata_path("id /?中文", "/project #?")
                .contains("id%20%2F%3F%E4%B8%AD%E6%96%87")
        );
    }
}
