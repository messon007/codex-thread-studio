//! Protocol-neutral bounded agent discovery; parsing, membership and tree layout
//! data belong on Rust workers, never GTK or the app-server's optional hints.
use crate::model::Thread;
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
pub const MAX_LINEAGE: usize = 32;
pub const MAX_AGENTS: usize = 10_000;
pub const MAX_PAGES: usize = 100;
const MAX_BYTES: usize = 32 * 1024 * 1024;
const SOURCE_KINDS: [&str; 5] = [
    "subAgent",
    "subAgentReview",
    "subAgentCompact",
    "subAgentThreadSpawn",
    "subAgentOther",
];

#[derive(Clone, Debug)]
pub struct Agent {
    pub thread: Thread,
    pub role: String,
    pub created: i64,
}
impl Agent {
    pub fn parse(value: &Value) -> Result<Arc<Self>, String> {
        if value
            .get("parentThreadId")
            .is_some_and(|parent| !parent.is_null() && !parent.is_string())
        {
            return Err("Invalid agent parent identity".into());
        }
        let thread =
            Thread::from_value(value).ok_or("Invalid agent session identity or project")?;
        if thread
            .parent
            .as_ref()
            .is_some_and(|parent| parent.len() > 256)
        {
            return Err("Invalid agent parent identity".into());
        }
        let role = value["agentRole"]
            .as_str()
            .unwrap_or("")
            .chars()
            .filter(|c| !c.is_control())
            .take(256)
            .collect();
        Ok(Arc::new(Self {
            thread,
            role,
            created: value["createdAt"].as_i64().unwrap_or(0),
        }))
    }
    fn weight(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.thread.id.len()
            + self.thread.parent.as_ref().map_or(0, String::len)
            + self.thread.title.len()
            + self.thread.cwd.len()
            + self.thread.activity.len()
            + self.role.len()
    }
}
#[derive(Debug)]
pub struct Page {
    pub agents: Vec<Arc<Agent>>,
    pub cursor: Option<String>,
}
impl Page {
    pub fn parse(value: &Value) -> Result<Self, String> {
        let rows = value["data"]
            .as_array()
            .ok_or("Invalid agent catalog response")?;
        if rows.len() > 100 {
            return Err("Agent catalog page exceeds 100 sessions".into());
        }
        let agents = rows
            .iter()
            .map(Agent::parse)
            .collect::<Result<Vec<_>, _>>()?;
        let cursor = match value.get("nextCursor") {
            None | Some(Value::Null) => None,
            Some(Value::String(cursor)) if cursor.is_empty() => None,
            Some(Value::String(cursor)) if cursor.len() <= 4096 => Some(cursor.clone()),
            _ => return Err("Invalid agent catalog cursor".into()),
        };
        Ok(Self { agents, cursor })
    }
}
/// Recognize thread catalog responses without consuming model/skill/MCP data.
/// Empty arrays remain intact for their original consumer.
pub fn prepare_page(payload: &mut Value) -> Option<Result<Page, String>> {
    if payload.get("id").is_none() || payload.get("method").is_some() {
        return None;
    }
    let rows = payload.pointer("/result/data")?.as_array()?;
    if rows.is_empty() {
        return Some(Page::parse(&payload["result"]));
    }
    if !rows
        .iter()
        .any(|row| row["id"].is_string() && row["cwd"].is_string())
    {
        return None;
    }
    let page = Page::parse(&payload["result"]);
    payload["result"] = Value::Null;
    Some(page)
}
#[derive(Clone, Debug)]
pub enum Step {
    Read(String),
    List(Option<String>),
}
impl Step {
    pub fn rpc(&self) -> (&'static str, Value) {
        match self {
            Self::Read(id) => ("thread/read", json!({"threadId":id,"includeTurns":false})),
            Self::List(cursor) => (
                "thread/list",
                json!({"cursor":cursor,"limit":100,"sourceKinds":SOURCE_KINDS,"useStateDbOnly":true}),
            ),
        }
    }
}
#[derive(Debug, Default)]
pub struct Discovery {
    pub lineage: Vec<Arc<Agent>>,
    candidates: Vec<Arc<Agent>>,
    cursors: HashSet<String>,
    pages: usize,
    bytes: usize,
    pub partial: Option<String>,
}
impl Discovery {
    pub fn summary(&mut self, expected: &str, agent: Arc<Agent>) -> Result<Step, String> {
        if agent.thread.id != expected {
            return Err("The backend returned a different agent session".into());
        }
        if self.lineage.len() >= MAX_LINEAGE
            || self.lineage.iter().any(|a| a.thread.id == agent.thread.id)
        {
            return Err("Agent ancestry contains a cycle or exceeds 32 levels".into());
        }
        let next = agent.thread.parent.clone();
        self.lineage.push(agent);
        match next {
            Some(parent) if self.lineage.iter().any(|a| a.thread.id == parent) => {
                Err("Agent ancestry contains a cycle".into())
            }
            Some(_) if self.lineage.len() >= MAX_LINEAGE => {
                Err("Agent ancestry exceeds 32 levels".into())
            }
            Some(parent) => Ok(Step::Read(parent)),
            None => Ok(Step::List(None)),
        }
    }
    pub fn page(&mut self, page: Page) -> Result<Option<Step>, String> {
        if self.pages >= MAX_PAGES {
            self.partial = Some("Partial catalog: reached the 100-page scan budget".into());
            return Ok(None);
        }
        if self
            .lineage
            .last()
            .is_none_or(|agent| agent.thread.parent.is_some())
        {
            return Err("Resolve the main agent before scanning its catalog".into());
        }
        if page
            .cursor
            .as_ref()
            .is_some_and(|cursor| self.cursors.contains(cursor))
        {
            return Err("The backend repeated an agent catalog cursor; refresh the tree".into());
        }
        let bytes = page
            .agents
            .iter()
            .map(|agent| agent.weight())
            .sum::<usize>();
        if self.candidates.len() + page.agents.len() > MAX_AGENTS || self.bytes + bytes > MAX_BYTES
        {
            self.partial =
                Some("Partial catalog: reached the 10,000-session / 32 MiB scan budget".into());
            return Ok(None);
        }
        self.pages += 1;
        self.bytes += bytes;
        self.candidates.extend(page.agents);
        if let Some(cursor) = page.cursor {
            self.cursors.insert(cursor.clone());
            if self.pages >= MAX_PAGES {
                self.partial = Some("Partial catalog: reached the 100-page scan budget".into());
                Ok(None)
            } else {
                Ok(Some(Step::List(Some(cursor))))
            }
        } else {
            Ok(None)
        }
    }
    pub fn scanned(&self) -> usize {
        self.candidates.len()
    }
    /// Called on a CPU worker, moving unused catalog metadata off the UI thread.
    pub fn tree(self) -> Result<Tree, String> {
        let selected = self
            .lineage
            .first()
            .ok_or("No selected agent was resolved")?
            .thread
            .id
            .clone();
        let root = self
            .lineage
            .last()
            .ok_or("No main agent was resolved")?
            .thread
            .id
            .clone();
        if self.lineage.last().unwrap().thread.parent.is_some() {
            return Err("Agent lineage has no main session".into());
        }
        let mut by_id: HashMap<String, Arc<Agent>> = self
            .candidates
            .into_iter()
            .map(|agent| (agent.thread.id.clone(), agent))
            .collect();
        // Direct summary reads override possibly older paged catalog metadata.
        for agent in self.lineage {
            by_id.insert(agent.thread.id.clone(), agent);
        }
        let mut children: HashMap<String, Vec<Arc<Agent>>> = HashMap::new();
        for agent in by_id.values() {
            if agent.thread.id == root {
                continue;
            }
            if let Some(parent) = &agent.thread.parent {
                if parent != &agent.thread.id && by_id.contains_key(parent) {
                    children
                        .entry(parent.clone())
                        .or_default()
                        .push(agent.clone());
                }
            }
        }
        for siblings in children.values_mut() {
            siblings.sort_by(|a, b| {
                a.created
                    .cmp(&b.created)
                    .then_with(|| a.thread.id.cmp(&b.thread.id))
            });
        }
        let mut stack = vec![(by_id.get(&root).unwrap().clone(), 0usize)];
        let mut visited = HashSet::new();
        let mut rows = Vec::new();
        let mut partial = self.partial;
        while let Some((agent, depth)) = stack.pop() {
            if !visited.insert(agent.thread.id.clone()) {
                continue;
            }
            if depth > 64 {
                partial = Some("Partial tree: descendants exceed 64 levels".into());
                continue;
            }
            if let Some(children) = children.get(&agent.thread.id) {
                for child in children.iter().rev() {
                    stack.push((child.clone(), depth + 1));
                }
            }
            rows.push(Row { agent, depth });
        }
        if !rows.iter().any(|row| row.agent.thread.id == selected) {
            return Err("The selected agent is missing from its resolved family".into());
        }
        Ok(Tree {
            selected,
            rows,
            partial,
        })
    }
}
#[derive(Debug)]
pub struct Row {
    pub agent: Arc<Agent>,
    pub depth: usize,
}
#[derive(Debug)]
pub struct Tree {
    pub selected: String,
    pub rows: Vec<Row>,
    pub partial: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn agent(id: &str, parent: Option<&str>, created: i64) -> Arc<Agent> {
        Agent::parse(&json!({"id":id,"parentThreadId":parent,"cwd":"/project","agentNickname":format!("Native {id}"),"createdAt":created})).unwrap()
    }
    #[test]
    fn tree_is_rooted_validated_stable_and_excludes_unrelated_or_cyclic_families() {
        let mut d = Discovery::default();
        assert!(
            matches!(d.summary("leaf",agent("leaf",Some("child"),4)).unwrap(),Step::Read(parent) if parent=="child")
        );
        d.summary("child", agent("child", Some("root"), 2)).unwrap();
        d.summary("root", agent("root", None, 0)).unwrap();
        d.page(Page {
            agents: vec![
                agent("child", Some("root"), 2),
                agent("older", Some("root"), 1),
                agent("leaf", Some("wrong"), 4),
                agent("other", None, 0),
                agent("unrelated", Some("other"), 1),
                agent("cycle-a", Some("cycle-b"), 1),
                agent("cycle-b", Some("cycle-a"), 2),
                agent("orphan", Some("missing"), 1),
            ],
            cursor: None,
        })
        .unwrap();
        let tree = d.tree().unwrap();
        assert_eq!(
            tree.rows
                .iter()
                .map(|row| (row.agent.thread.id.as_str(), row.depth))
                .collect::<Vec<_>>(),
            [("root", 0), ("older", 1), ("child", 1), ("leaf", 2)]
        );
        assert_eq!(tree.selected, "leaf");
        assert!(tree.partial.is_none());
    }
    #[test]
    fn lineage_cursor_and_identity_failures_do_not_construct_a_misleading_tree() {
        let mut d = Discovery::default();
        assert!(d.summary("a", agent("b", None, 0)).is_err());
        d.summary("a", agent("a", Some("b"), 0)).unwrap();
        assert!(d.summary("b", agent("b", Some("a"), 0)).is_err());
        let mut d = Discovery::default();
        d.summary("root", agent("root", None, 0)).unwrap();
        d.page(Page {
            agents: vec![],
            cursor: Some("opaque:中文/+?".into()),
        })
        .unwrap();
        assert!(d
            .page(Page {
                agents: vec![],
                cursor: Some("opaque:中文/+?".into())
            })
            .is_err());
        assert!(!Step::List(None)
            .rpc()
            .1
            .as_object()
            .unwrap()
            .contains_key("ancestorThreadId"));
        assert_eq!(Step::Read("child".into()).rpc().1["includeTurns"], false);
    }
    #[test]
    fn worker_projection_bounds_and_strips_thread_payloads_without_consuming_models() {
        let mut payload = json!({"id":1,"result":{"data":[{"id":"a","parentThreadId":"root","cwd":"/project","preview":"x".repeat(100000),"turns":[{"items":[{"text":"never on GTK"}]}]}],"nextCursor":"opaque"}});
        let page = prepare_page(&mut payload).unwrap().unwrap();
        assert_eq!(page.agents[0].thread.title.len(), 120);
        assert_eq!(page.cursor.as_deref(), Some("opaque"));
        assert!(payload["result"].is_null());
        let mut models = json!({"id":2,"result":{"data":[{"id":"model","model":"model"}]}});
        assert!(prepare_page(&mut models).is_none());
        assert!(models["result"]["data"].is_array());
        assert!(Page::parse(&json!({"data":[],"nextCursor":"x".repeat(4097)})).is_err());
    }
    #[test]
    fn bounded_scan_returns_explicit_partial_tree_with_the_selected_lineage() {
        let mut d = Discovery::default();
        d.summary("root", agent("root", None, 0)).unwrap();
        for index in 0..MAX_PAGES {
            let step = d
                .page(Page {
                    agents: (0..100)
                        .map(|column| {
                            agent(
                                &format!("child-{index}-{column}"),
                                Some(if column % 2 == 0 {
                                    "root"
                                } else {
                                    "other-family"
                                }),
                                (index * 100 + column) as i64,
                            )
                        })
                        .collect(),
                    cursor: Some(index.to_string()),
                })
                .unwrap();
            assert_eq!(step.is_none(), index == MAX_PAGES - 1);
        }
        let scanned = d.scanned();
        assert!(d
            .page(Page {
                agents: vec![agent("overflow", Some("root"), 0)],
                cursor: None
            })
            .unwrap()
            .is_none());
        assert_eq!(d.scanned(), scanned);
        let tree = d.tree().unwrap();
        assert_eq!(tree.rows.len(), MAX_PAGES * 50 + 1);
        assert!(tree.partial.unwrap().contains("100-page"));
    }
    #[test]
    fn ancestry_budget_allows_32_summaries_but_never_requests_a_33rd() {
        let mut complete = Discovery::default();
        let mut incomplete = Discovery::default();
        for index in 0..MAX_LINEAGE {
            let id = format!("agent-{index}");
            let next = format!("agent-{}", index + 1);
            let step = complete
                .summary(
                    &id,
                    agent(
                        &id,
                        (index + 1 < MAX_LINEAGE).then_some(next.as_str()),
                        index as i64,
                    ),
                )
                .unwrap();
            assert_eq!(matches!(step, Step::List(None)), index + 1 == MAX_LINEAGE);
            let step = incomplete.summary(&id, agent(&id, Some(&next), index as i64));
            assert_eq!(step.is_err(), index + 1 == MAX_LINEAGE);
        }
        assert_eq!(complete.tree().unwrap().rows.len(), MAX_LINEAGE);
        assert!(incomplete.tree().is_err());
    }
}
