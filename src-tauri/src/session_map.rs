use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::backend_config::valid_backend_id;

pub const MAX_MAP_BODY_BYTES: usize = 512 * 1024;
const MAX_ITEMS: usize = 1_000;
const MAX_RELATIONS: usize = 2_000;
const MAX_OPERATIONS: usize = 100;
const MAX_DEPTH: usize = 16;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMap {
    pub id: String,
    pub backend: String,
    pub thread_id: String,
    pub goal: String,
    pub definition_of_done: String,
    pub structure: String,
    pub revision: u64,
    pub current_item_id: Option<String>,
    pub last_synced_turn_id: Option<String>,
    pub items: Vec<MapItem>,
    pub relations: Vec<MapRelation>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapItem {
    pub id: String,
    pub parent_id: Option<String>,
    #[serde(default = "default_item_kind")]
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default = "default_item_state")]
    pub state: String,
    #[serde(default)]
    pub position: i64,
    #[serde(default)]
    pub archived: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapRelation {
    pub id: String,
    pub from_item_id: String,
    pub to_item_id: String,
    #[serde(default = "default_relation_kind")]
    pub kind: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMapRequest {
    pub backend: String,
    pub thread_id: String,
    pub goal: String,
    #[serde(default)]
    pub definition_of_done: String,
    #[serde(default = "default_structure")]
    pub structure: String,
    #[serde(default)]
    pub items: Vec<CreateMapItem>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMapItem {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default = "default_item_kind")]
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default = "default_item_state")]
    pub state: String,
    #[serde(default)]
    pub position: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOperationsRequest {
    pub base_revision: u64,
    #[serde(default = "default_actor")]
    pub actor: String,
    #[serde(default)]
    pub source_turn_id: Option<String>,
    pub operations: Vec<MapOperation>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum MapOperation {
    SetGoal {
        goal: String,
        #[serde(default)]
        definition_of_done: Option<String>,
    },
    AddItem {
        #[serde(default)]
        item_id: Option<String>,
        #[serde(default)]
        parent_id: Option<String>,
        #[serde(default)]
        after_item_id: Option<String>,
        title: String,
        #[serde(default = "default_item_kind")]
        kind: String,
        #[serde(default)]
        summary: String,
        #[serde(default = "default_item_state")]
        state: String,
    },
    UpdateItem {
        item_id: String,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        kind: Option<String>,
        #[serde(default)]
        summary: Option<String>,
    },
    MoveItem {
        item_id: String,
        #[serde(default)]
        parent_id: Option<String>,
        #[serde(default)]
        after_item_id: Option<String>,
    },
    ReorderItem {
        item_id: String,
        #[serde(default)]
        after_item_id: Option<String>,
    },
    SetState {
        item_id: String,
        state: String,
    },
    SetCurrent {
        #[serde(default)]
        item_id: Option<String>,
    },
    ArchiveItem {
        item_id: String,
    },
    AddRelation {
        #[serde(default)]
        relation_id: Option<String>,
        from_item_id: String,
        to_item_id: String,
        #[serde(default = "default_relation_kind")]
        kind: String,
    },
    RemoveRelation {
        relation_id: String,
    },
}

fn default_structure() -> String {
    "hierarchy".to_string()
}

fn default_item_kind() -> String {
    "item".to_string()
}

fn default_item_state() -> String {
    "notStarted".to_string()
}

fn default_relation_kind() -> String {
    "related".to_string()
}

fn default_actor() -> String {
    "user".to_string()
}

pub fn initialize(path: &Path) -> Result<(), String> {
    connection(path).map(|_| ())
}

pub fn create(path: &Path, request: CreateMapRequest) -> Result<SessionMap, String> {
    let now = now_ms();
    let map_id = Uuid::new_v4().to_string();
    let mut map = SessionMap {
        id: map_id,
        backend: request.backend,
        thread_id: request.thread_id,
        goal: request.goal,
        definition_of_done: request.definition_of_done,
        structure: request.structure,
        revision: 1,
        current_item_id: None,
        last_synced_turn_id: None,
        items: request
            .items
            .into_iter()
            .map(|item| MapItem {
                id: item.id.unwrap_or_else(|| Uuid::new_v4().to_string()),
                parent_id: item.parent_id,
                kind: item.kind,
                title: item.title,
                summary: item.summary,
                state: item.state,
                position: item.position,
                archived: false,
                created_at: now,
                updated_at: now,
            })
            .collect(),
        relations: Vec::new(),
        created_at: now,
        updated_at: now,
    };
    normalize_positions(&mut map.items);
    validate_map(&map)?;

    let mut connection = connection(path)?;
    if find_with_connection(&connection, &map.backend, &map.thread_id)?.is_some() {
        return Err("this session already has a map".to_string());
    }
    let transaction = connection.transaction().map_err(sql_error)?;
    insert_snapshot(&transaction, &map)?;
    transaction.commit().map_err(sql_error)?;
    Ok(map)
}

pub fn find(path: &Path, backend: &str, thread_id: &str) -> Result<Option<SessionMap>, String> {
    validate_anchor(backend, thread_id)?;
    find_with_connection(&connection(path)?, backend, thread_id)
}

pub fn remove(path: &Path, backend: &str, thread_id: &str) -> Result<Option<SessionMap>, String> {
    validate_anchor(backend, thread_id)?;
    let mut connection = connection(path)?;
    let transaction = connection.transaction().map_err(sql_error)?;
    let existing = find_with_connection(&transaction, backend, thread_id)?;
    if let Some(map) = &existing {
        transaction
            .execute("DELETE FROM session_maps WHERE id = ?1", [&map.id])
            .map_err(sql_error)?;
    }
    transaction.commit().map_err(sql_error)?;
    Ok(existing)
}

pub fn apply_operations(
    path: &Path,
    backend: &str,
    thread_id: &str,
    request: ApplyOperationsRequest,
) -> Result<Option<SessionMap>, String> {
    validate_anchor(backend, thread_id)?;
    validate_operations_request(&request)?;
    let mut connection = connection(path)?;
    let transaction = connection.transaction().map_err(sql_error)?;
    let Some(mut map) = find_with_connection(&transaction, backend, thread_id)? else {
        return Ok(None);
    };
    if map.revision != request.base_revision {
        return Err(format!(
            "map revision conflict: expected {}, current {}",
            request.base_revision, map.revision
        ));
    }
    let before = map.clone();
    for operation in &request.operations {
        apply_operation(&mut map, operation, &request.actor)?;
    }
    map.revision += 1;
    map.updated_at = now_ms();
    if let Some(turn_id) = request
        .source_turn_id
        .as_ref()
        .filter(|turn_id| !turn_id.trim().is_empty())
    {
        map.last_synced_turn_id = Some(turn_id.clone());
    }
    normalize_positions(&mut map.items);
    validate_map(&map)?;
    replace_snapshot(&transaction, &map)?;
    insert_change(&transaction, &before, &map, &request)?;
    transaction.commit().map_err(sql_error)?;
    Ok(Some(map))
}

pub fn undo(path: &Path, backend: &str, thread_id: &str) -> Result<Option<SessionMap>, String> {
    validate_anchor(backend, thread_id)?;
    let mut connection = connection(path)?;
    let transaction = connection.transaction().map_err(sql_error)?;
    let Some(current) = find_with_connection(&transaction, backend, thread_id)? else {
        return Ok(None);
    };
    let change = transaction
        .query_row(
            "SELECT id, before_json FROM session_map_changes
             WHERE map_id = ?1 AND undone = 0 ORDER BY id DESC LIMIT 1",
            [&current.id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(sql_error)?;
    let Some((change_id, before_json)) = change else {
        return Err("there is no map change to undo".to_string());
    };
    let mut restored = serde_json::from_str::<SessionMap>(&before_json)
        .map_err(|error| format!("stored map history is invalid: {error}"))?;
    restored.revision = current.revision + 1;
    restored.updated_at = now_ms();
    validate_map(&restored)?;
    replace_snapshot(&transaction, &restored)?;
    transaction
        .execute(
            "UPDATE session_map_changes SET undone = 1 WHERE id = ?1",
            [change_id],
        )
        .map_err(sql_error)?;
    transaction.commit().map_err(sql_error)?;
    Ok(Some(restored))
}

fn apply_operation(
    map: &mut SessionMap,
    operation: &MapOperation,
    actor: &str,
) -> Result<(), String> {
    if actor == "assistant" && !assistant_operation_allowed(operation) {
        return Err("the assistant cannot apply this map operation automatically".to_string());
    }
    let now = now_ms();
    match operation {
        MapOperation::SetGoal {
            goal,
            definition_of_done,
        } => {
            map.goal = goal.trim().to_string();
            if let Some(value) = definition_of_done {
                map.definition_of_done = value.trim().to_string();
            }
        }
        MapOperation::AddItem {
            item_id,
            parent_id,
            after_item_id,
            title,
            kind,
            summary,
            state,
        } => {
            if let Some(parent_id) = parent_id {
                require_item(map, parent_id)?;
            }
            let id = item_id
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            if map.items.iter().any(|item| item.id == id) {
                return Err(format!("map item {id} already exists"));
            }
            map.items.push(MapItem {
                id: id.clone(),
                parent_id: parent_id.clone(),
                kind: kind.trim().to_string(),
                title: title.trim().to_string(),
                summary: summary.trim().to_string(),
                state: state.clone(),
                position: i64::MAX,
                archived: false,
                created_at: now,
                updated_at: now,
            });
            place_after(
                &mut map.items,
                &id,
                parent_id.as_deref(),
                after_item_id.as_deref(),
            )?;
        }
        MapOperation::UpdateItem {
            item_id,
            title,
            kind,
            summary,
        } => {
            let item = require_item_mut(map, item_id)?;
            if let Some(value) = title {
                item.title = value.trim().to_string();
            }
            if let Some(value) = kind {
                item.kind = value.trim().to_string();
            }
            if let Some(value) = summary {
                item.summary = value.trim().to_string();
            }
            item.updated_at = now;
        }
        MapOperation::MoveItem {
            item_id,
            parent_id,
            after_item_id,
        } => {
            if let Some(parent_id) = parent_id {
                require_item(map, parent_id)?;
                if is_descendant(map, parent_id, item_id) {
                    return Err("a map item cannot be moved into its own subtree".to_string());
                }
            }
            let item = require_item_mut(map, item_id)?;
            item.parent_id = parent_id.clone();
            item.updated_at = now;
            place_after(
                &mut map.items,
                item_id,
                parent_id.as_deref(),
                after_item_id.as_deref(),
            )?;
        }
        MapOperation::ReorderItem {
            item_id,
            after_item_id,
        } => {
            let parent_id = require_item(map, item_id)?.parent_id.clone();
            place_after(
                &mut map.items,
                item_id,
                parent_id.as_deref(),
                after_item_id.as_deref(),
            )?;
        }
        MapOperation::SetState { item_id, state } => {
            let item = require_item_mut(map, item_id)?;
            item.state = state.clone();
            item.updated_at = now;
        }
        MapOperation::SetCurrent { item_id } => {
            if let Some(previous) = map.current_item_id.as_ref() {
                if item_id.as_ref() != Some(previous) {
                    if let Some(item) = map.items.iter_mut().find(|item| item.id == *previous) {
                        if item.state == "active" {
                            item.state = "visited".to_string();
                            item.updated_at = now;
                        }
                    }
                }
            }
            if let Some(item_id) = item_id {
                let item = require_item_mut(map, item_id)?;
                if !matches!(item.state.as_str(), "done" | "paused") {
                    item.state = "active".to_string();
                }
                item.updated_at = now;
            }
            map.current_item_id = item_id.clone();
        }
        MapOperation::ArchiveItem { item_id } => {
            require_item(map, item_id)?;
            let mut pending = vec![item_id.clone()];
            let mut archived = HashSet::new();
            while let Some(id) = pending.pop() {
                if !archived.insert(id.clone()) {
                    continue;
                }
                pending.extend(
                    map.items
                        .iter()
                        .filter(|item| item.parent_id.as_ref() == Some(&id))
                        .map(|item| item.id.clone()),
                );
            }
            for item in &mut map.items {
                if archived.contains(&item.id) {
                    item.archived = true;
                    item.updated_at = now;
                }
            }
            if map
                .current_item_id
                .as_ref()
                .is_some_and(|id| archived.contains(id))
            {
                map.current_item_id = None;
            }
        }
        MapOperation::AddRelation {
            relation_id,
            from_item_id,
            to_item_id,
            kind,
        } => {
            require_item(map, from_item_id)?;
            require_item(map, to_item_id)?;
            let id = relation_id
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            if map.relations.iter().any(|relation| relation.id == id) {
                return Err(format!("map relation {id} already exists"));
            }
            map.relations.push(MapRelation {
                id,
                from_item_id: from_item_id.clone(),
                to_item_id: to_item_id.clone(),
                kind: kind.trim().to_string(),
            });
        }
        MapOperation::RemoveRelation { relation_id } => {
            let before = map.relations.len();
            map.relations.retain(|relation| relation.id != *relation_id);
            if map.relations.len() == before {
                return Err(format!("map relation {relation_id} does not exist"));
            }
        }
    }
    Ok(())
}

fn assistant_operation_allowed(operation: &MapOperation) -> bool {
    match operation {
        MapOperation::AddItem { state, .. } => {
            matches!(state.as_str(), "notStarted" | "active" | "visited")
        }
        MapOperation::UpdateItem { .. } | MapOperation::SetCurrent { .. } => true,
        MapOperation::SetState { state, .. } => {
            matches!(state.as_str(), "notStarted" | "active" | "visited")
        }
        _ => false,
    }
}

fn require_item<'a>(map: &'a SessionMap, item_id: &str) -> Result<&'a MapItem, String> {
    map.items
        .iter()
        .find(|item| item.id == item_id && !item.archived)
        .ok_or_else(|| format!("map item {item_id} does not exist"))
}

fn require_item_mut<'a>(map: &'a mut SessionMap, item_id: &str) -> Result<&'a mut MapItem, String> {
    map.items
        .iter_mut()
        .find(|item| item.id == item_id && !item.archived)
        .ok_or_else(|| format!("map item {item_id} does not exist"))
}

fn is_descendant(map: &SessionMap, candidate: &str, ancestor: &str) -> bool {
    let mut current = Some(candidate);
    for _ in 0..=MAX_DEPTH {
        let Some(id) = current else { return false };
        if id == ancestor {
            return true;
        }
        current = map
            .items
            .iter()
            .find(|item| item.id == id)
            .and_then(|item| item.parent_id.as_deref());
    }
    true
}

fn place_after(
    items: &mut [MapItem],
    item_id: &str,
    parent_id: Option<&str>,
    after_item_id: Option<&str>,
) -> Result<(), String> {
    let mut siblings = items
        .iter()
        .filter(|item| {
            item.id != item_id && !item.archived && item.parent_id.as_deref() == parent_id
        })
        .map(|item| (item.id.clone(), item.position))
        .collect::<Vec<_>>();
    siblings.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)));
    let insertion = match after_item_id {
        Some(after) => siblings
            .iter()
            .position(|(id, _)| id == after)
            .map(|index| index + 1)
            .ok_or_else(|| "afterItemId must name a sibling item".to_string())?,
        None => siblings.len(),
    };
    siblings.insert(insertion, (item_id.to_string(), i64::MAX));
    let positions = siblings
        .into_iter()
        .enumerate()
        .map(|(position, (id, _))| (id, position as i64))
        .collect::<HashMap<_, _>>();
    for item in items.iter_mut() {
        if let Some(position) = positions.get(&item.id) {
            item.position = *position;
        }
    }
    Ok(())
}

fn normalize_positions(items: &mut [MapItem]) {
    let parents = items
        .iter()
        .filter(|item| !item.archived)
        .map(|item| item.parent_id.clone())
        .collect::<HashSet<_>>();
    for parent in parents {
        let mut siblings = items
            .iter()
            .filter(|item| !item.archived && item.parent_id == parent)
            .map(|item| (item.id.clone(), item.position))
            .collect::<Vec<_>>();
        siblings.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)));
        let positions = siblings
            .into_iter()
            .enumerate()
            .map(|(position, (id, _))| (id, position as i64))
            .collect::<HashMap<_, _>>();
        for item in items.iter_mut() {
            if let Some(position) = positions.get(&item.id) {
                item.position = *position;
            }
        }
    }
}

fn validate_operations_request(request: &ApplyOperationsRequest) -> Result<(), String> {
    if !matches!(request.actor.as_str(), "user" | "assistant") {
        return Err("map operation actor must be user or assistant".to_string());
    }
    if request.operations.is_empty() || request.operations.len() > MAX_OPERATIONS {
        return Err(format!(
            "a map update must contain between 1 and {MAX_OPERATIONS} operations"
        ));
    }
    if request
        .source_turn_id
        .as_ref()
        .is_some_and(|value| value.len() > 256)
    {
        return Err("source turn id is too large".to_string());
    }
    Ok(())
}

fn validate_anchor(backend: &str, thread_id: &str) -> Result<(), String> {
    if !valid_backend_id(backend) {
        return Err("map backend id is invalid".to_string());
    }
    if thread_id.trim().is_empty() || thread_id.len() > 256 {
        return Err("map thread id is invalid".to_string());
    }
    Ok(())
}

fn validate_map(map: &SessionMap) -> Result<(), String> {
    validate_anchor(&map.backend, &map.thread_id)?;
    if map.id.trim().is_empty() || map.id.len() > 128 {
        return Err("map id is invalid".to_string());
    }
    if map.goal.trim().is_empty() || map.goal.len() > 4_096 {
        return Err("map goal must contain between 1 and 4096 bytes".to_string());
    }
    if map.definition_of_done.len() > 8_192 {
        return Err("map completion definition is too large".to_string());
    }
    if !matches!(
        map.structure.as_str(),
        "hierarchy" | "path" | "flow" | "blank"
    ) {
        return Err("map structure must be hierarchy, path, flow, or blank".to_string());
    }
    if map.items.len() > MAX_ITEMS || map.relations.len() > MAX_RELATIONS {
        return Err("map contains too many items or relations".to_string());
    }
    let mut ids = HashSet::new();
    for item in &map.items {
        if item.id.trim().is_empty() || item.id.len() > 128 || !ids.insert(item.id.as_str()) {
            return Err("map item ids must be unique and contain at most 128 bytes".to_string());
        }
        if item.title.trim().is_empty() || item.title.len() > 512 {
            return Err(format!("map item {} has an invalid title", item.id));
        }
        if item.kind.trim().is_empty() || item.kind.len() > 64 || item.summary.len() > 4_096 {
            return Err(format!("map item {} metadata is too large", item.id));
        }
        if !matches!(
            item.state.as_str(),
            "notStarted" | "active" | "visited" | "done" | "paused"
        ) {
            return Err(format!("map item {} has an invalid state", item.id));
        }
        if item.position < 0 {
            return Err(format!("map item {} has an invalid position", item.id));
        }
    }
    for item in &map.items {
        if let Some(parent_id) = &item.parent_id {
            if parent_id == &item.id || !ids.contains(parent_id.as_str()) {
                return Err(format!("map item {} has an invalid parent", item.id));
            }
        }
        let mut current = item.parent_id.as_deref();
        let mut visited = HashSet::new();
        for depth in 0..=MAX_DEPTH {
            let Some(parent_id) = current else { break };
            if depth == MAX_DEPTH || !visited.insert(parent_id) {
                return Err("map hierarchy contains a cycle or is too deep".to_string());
            }
            current = map
                .items
                .iter()
                .find(|candidate| candidate.id == parent_id)
                .and_then(|candidate| candidate.parent_id.as_deref());
        }
    }
    if let Some(current) = &map.current_item_id {
        if !map
            .items
            .iter()
            .any(|item| item.id == *current && !item.archived)
        {
            return Err("map current item does not exist".to_string());
        }
    }
    let mut relation_ids = HashSet::new();
    for relation in &map.relations {
        if relation.id.trim().is_empty()
            || relation.id.len() > 128
            || !relation_ids.insert(relation.id.as_str())
            || !ids.contains(relation.from_item_id.as_str())
            || !ids.contains(relation.to_item_id.as_str())
            || relation.kind.trim().is_empty()
            || relation.kind.len() > 64
        {
            return Err("map contains an invalid relation".to_string());
        }
    }
    Ok(())
}

fn connection(path: &Path) -> Result<Connection, String> {
    let parent = path
        .parent()
        .ok_or_else(|| "session map database path has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let connection = Connection::open(path).map_err(sql_error)?;
    connection
        .busy_timeout(Duration::from_secs(3))
        .map_err(sql_error)?;
    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS session_maps (
               id TEXT PRIMARY KEY,
               backend TEXT NOT NULL,
               thread_id TEXT NOT NULL,
               goal TEXT NOT NULL,
               definition_of_done TEXT NOT NULL,
               structure TEXT NOT NULL,
               revision INTEGER NOT NULL,
               current_item_id TEXT,
               last_synced_turn_id TEXT,
               created_at INTEGER NOT NULL,
               updated_at INTEGER NOT NULL,
               UNIQUE(backend, thread_id)
             );
             CREATE TABLE IF NOT EXISTS session_map_items (
               map_id TEXT NOT NULL,
               id TEXT NOT NULL,
               parent_id TEXT,
               kind TEXT NOT NULL,
               title TEXT NOT NULL,
               summary TEXT NOT NULL,
               state TEXT NOT NULL,
               position INTEGER NOT NULL,
               archived INTEGER NOT NULL,
               created_at INTEGER NOT NULL,
               updated_at INTEGER NOT NULL,
               PRIMARY KEY(map_id, id),
               FOREIGN KEY(map_id) REFERENCES session_maps(id) ON DELETE CASCADE
             );
             CREATE INDEX IF NOT EXISTS session_map_item_parent
               ON session_map_items(map_id, parent_id, position);
             CREATE TABLE IF NOT EXISTS session_map_relations (
               map_id TEXT NOT NULL,
               id TEXT NOT NULL,
               from_item_id TEXT NOT NULL,
               to_item_id TEXT NOT NULL,
               kind TEXT NOT NULL,
               PRIMARY KEY(map_id, id),
               FOREIGN KEY(map_id) REFERENCES session_maps(id) ON DELETE CASCADE
             );
             CREATE TABLE IF NOT EXISTS session_map_changes (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               map_id TEXT NOT NULL,
               before_json TEXT NOT NULL,
               after_json TEXT NOT NULL,
               actor TEXT NOT NULL,
               source_turn_id TEXT,
               created_at INTEGER NOT NULL,
               undone INTEGER NOT NULL DEFAULT 0,
               FOREIGN KEY(map_id) REFERENCES session_maps(id) ON DELETE CASCADE
             );
             CREATE INDEX IF NOT EXISTS session_map_change_order
               ON session_map_changes(map_id, id DESC);",
        )
        .map_err(sql_error)?;
    Ok(connection)
}

fn insert_snapshot(transaction: &Transaction<'_>, map: &SessionMap) -> Result<(), String> {
    transaction
        .execute(
            "INSERT INTO session_maps (
               id, backend, thread_id, goal, definition_of_done, structure,
               revision, current_item_id, last_synced_turn_id, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                map.id,
                map.backend,
                map.thread_id,
                map.goal,
                map.definition_of_done,
                map.structure,
                map.revision,
                map.current_item_id,
                map.last_synced_turn_id,
                map.created_at,
                map.updated_at,
            ],
        )
        .map_err(sql_error)?;
    insert_children(transaction, map)
}

fn replace_snapshot(transaction: &Transaction<'_>, map: &SessionMap) -> Result<(), String> {
    transaction
        .execute(
            "UPDATE session_maps SET
               goal = ?2, definition_of_done = ?3, structure = ?4, revision = ?5,
               current_item_id = ?6, last_synced_turn_id = ?7, updated_at = ?8
             WHERE id = ?1",
            params![
                map.id,
                map.goal,
                map.definition_of_done,
                map.structure,
                map.revision,
                map.current_item_id,
                map.last_synced_turn_id,
                map.updated_at,
            ],
        )
        .map_err(sql_error)?;
    transaction
        .execute(
            "DELETE FROM session_map_relations WHERE map_id = ?1",
            [&map.id],
        )
        .map_err(sql_error)?;
    transaction
        .execute("DELETE FROM session_map_items WHERE map_id = ?1", [&map.id])
        .map_err(sql_error)?;
    insert_children(transaction, map)
}

fn insert_children(transaction: &Transaction<'_>, map: &SessionMap) -> Result<(), String> {
    for item in &map.items {
        transaction
            .execute(
                "INSERT INTO session_map_items (
                   map_id, id, parent_id, kind, title, summary, state, position,
                   archived, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    map.id,
                    item.id,
                    item.parent_id,
                    item.kind,
                    item.title,
                    item.summary,
                    item.state,
                    item.position,
                    item.archived,
                    item.created_at,
                    item.updated_at,
                ],
            )
            .map_err(sql_error)?;
    }
    for relation in &map.relations {
        transaction
            .execute(
                "INSERT INTO session_map_relations (
                   map_id, id, from_item_id, to_item_id, kind
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    map.id,
                    relation.id,
                    relation.from_item_id,
                    relation.to_item_id,
                    relation.kind,
                ],
            )
            .map_err(sql_error)?;
    }
    Ok(())
}

fn insert_change(
    transaction: &Transaction<'_>,
    before: &SessionMap,
    after: &SessionMap,
    request: &ApplyOperationsRequest,
) -> Result<(), String> {
    let before_json = serde_json::to_string(before).map_err(|error| error.to_string())?;
    let after_json = serde_json::to_string(after).map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO session_map_changes (
               map_id, before_json, after_json, actor, source_turn_id, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                after.id,
                before_json,
                after_json,
                request.actor,
                request.source_turn_id,
                now_ms(),
            ],
        )
        .map_err(sql_error)?;
    Ok(())
}

fn find_with_connection(
    connection: &Connection,
    backend: &str,
    thread_id: &str,
) -> Result<Option<SessionMap>, String> {
    let map = connection
        .query_row(
            "SELECT id, backend, thread_id, goal, definition_of_done, structure,
                    revision, current_item_id, last_synced_turn_id, created_at, updated_at
             FROM session_maps WHERE backend = ?1 AND thread_id = ?2",
            params![backend, thread_id],
            map_from_row,
        )
        .optional()
        .map_err(sql_error)?;
    let Some(mut map) = map else { return Ok(None) };
    let mut item_statement = connection
        .prepare(
            "SELECT id, parent_id, kind, title, summary, state, position,
                    archived, created_at, updated_at
             FROM session_map_items WHERE map_id = ?1
             ORDER BY parent_id, position, id",
        )
        .map_err(sql_error)?;
    map.items = item_statement
        .query_map([&map.id], item_from_row)
        .map_err(sql_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(sql_error)?;
    let mut relation_statement = connection
        .prepare(
            "SELECT id, from_item_id, to_item_id, kind
             FROM session_map_relations WHERE map_id = ?1 ORDER BY id",
        )
        .map_err(sql_error)?;
    map.relations = relation_statement
        .query_map([&map.id], relation_from_row)
        .map_err(sql_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(sql_error)?;
    Ok(Some(map))
}

fn map_from_row(row: &Row<'_>) -> rusqlite::Result<SessionMap> {
    Ok(SessionMap {
        id: row.get(0)?,
        backend: row.get(1)?,
        thread_id: row.get(2)?,
        goal: row.get(3)?,
        definition_of_done: row.get(4)?,
        structure: row.get(5)?,
        revision: row.get(6)?,
        current_item_id: row.get(7)?,
        last_synced_turn_id: row.get(8)?,
        items: Vec::new(),
        relations: Vec::new(),
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn item_from_row(row: &Row<'_>) -> rusqlite::Result<MapItem> {
    Ok(MapItem {
        id: row.get(0)?,
        parent_id: row.get(1)?,
        kind: row.get(2)?,
        title: row.get(3)?,
        summary: row.get(4)?,
        state: row.get(5)?,
        position: row.get(6)?,
        archived: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn relation_from_row(row: &Row<'_>) -> rusqlite::Result<MapRelation> {
    Ok(MapRelation {
        id: row.get(0)?,
        from_item_id: row.get(1)?,
        to_item_id: row.get(2)?,
        kind: row.get(3)?,
    })
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn sql_error(error: rusqlite::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_unsigned_conversions_are_checked_not_wrapped() {
        let connection = Connection::open_in_memory().expect("database");
        let maximum = i64::MAX as u64;
        let round_trip: u64 = connection
            .query_row("SELECT ?1", [maximum], |row| row.get(0))
            .expect("positive integer");
        assert_eq!(round_trip, maximum);
        assert!(connection
            .query_row("SELECT ?1", [u64::MAX], |row| row.get::<_, i64>(0))
            .is_err());
        assert!(connection
            .query_row("SELECT -1", [], |row| row.get::<_, u64>(0))
            .is_err());
        let count: usize = connection
            .query_row("SELECT COUNT(*) FROM (SELECT 1)", [], |row| row.get(0))
            .expect("count");
        assert_eq!(count, 1);
    }

    fn database(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "codex-thread-studio-map-{name}-{}-{}.sqlite3",
            std::process::id(),
            now_ms()
        ))
    }

    fn create_request(thread_id: &str) -> CreateMapRequest {
        CreateMapRequest {
            backend: "codex".to_string(),
            thread_id: thread_id.to_string(),
            goal: "Understand the upgrade set".to_string(),
            definition_of_done: "Every package has an upgrade decision".to_string(),
            structure: "hierarchy".to_string(),
            items: vec![CreateMapItem {
                id: Some("packages".to_string()),
                parent_id: None,
                kind: "group".to_string(),
                title: "Packages".to_string(),
                summary: String::new(),
                state: "notStarted".to_string(),
                position: 0,
            }],
        }
    }

    #[test]
    fn accepts_configured_backend_map_anchors() {
        assert!(validate_anchor("company-codex", "thread-1").is_ok());
    }

    fn cleanup(path: &Path) {
        for candidate in [
            path.to_path_buf(),
            path.with_extension("sqlite3-wal"),
            path.with_extension("sqlite3-shm"),
        ] {
            fs::remove_file(candidate).ok();
        }
    }

    #[test]
    fn absence_is_normal_and_creation_is_explicit() {
        let path = database("optional");
        assert!(find(&path, "codex", "existing-thread").unwrap().is_none());
        let map = create(&path, create_request("existing-thread")).unwrap();
        assert_eq!(map.revision, 1);
        assert_eq!(find(&path, "codex", "existing-thread").unwrap(), Some(map));
        cleanup(&path);
    }

    #[test]
    fn applies_safe_assistant_updates_and_supports_undo() {
        let path = database("operations");
        let map = create(&path, create_request("thread-1")).unwrap();
        let changed = apply_operations(
            &path,
            "codex",
            "thread-1",
            ApplyOperationsRequest {
                base_revision: map.revision,
                actor: "assistant".to_string(),
                source_turn_id: Some("turn-1".to_string()),
                operations: vec![
                    MapOperation::AddItem {
                        item_id: Some("libc".to_string()),
                        parent_id: Some("packages".to_string()),
                        after_item_id: None,
                        title: "libc".to_string(),
                        kind: "topic".to_string(),
                        summary: "Runtime library".to_string(),
                        state: "visited".to_string(),
                    },
                    MapOperation::SetCurrent {
                        item_id: Some("libc".to_string()),
                    },
                ],
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(changed.current_item_id.as_deref(), Some("libc"));
        assert_eq!(changed.last_synced_turn_id.as_deref(), Some("turn-1"));
        assert!(changed.items.iter().any(|item| item.id == "libc"));

        let restored = undo(&path, "codex", "thread-1").unwrap().unwrap();
        assert!(restored.items.iter().all(|item| item.id != "libc"));
        assert!(restored.revision > changed.revision);
        cleanup(&path);
    }

    #[test]
    fn rejects_unsafe_assistant_operations_and_revision_conflicts() {
        let path = database("guardrails");
        let map = create(&path, create_request("thread-2")).unwrap();
        let unsafe_result = apply_operations(
            &path,
            "codex",
            "thread-2",
            ApplyOperationsRequest {
                base_revision: map.revision,
                actor: "assistant".to_string(),
                source_turn_id: Some("turn-2".to_string()),
                operations: vec![MapOperation::SetGoal {
                    goal: "Different goal".to_string(),
                    definition_of_done: None,
                }],
            },
        );
        assert!(unsafe_result.is_err());

        let conflict = apply_operations(
            &path,
            "codex",
            "thread-2",
            ApplyOperationsRequest {
                base_revision: 0,
                actor: "user".to_string(),
                source_turn_id: None,
                operations: vec![MapOperation::SetCurrent { item_id: None }],
            },
        );
        assert!(conflict.unwrap_err().contains("revision conflict"));
        cleanup(&path);
    }

    #[test]
    fn parses_camel_case_operation_fields_from_the_webview_contract() {
        let operation: MapOperation = serde_json::from_value(serde_json::json!({
            "op": "addItem",
            "itemId": "child",
            "parentId": "root",
            "afterItemId": "sibling",
            "title": "Child",
            "kind": "topic",
            "summary": "Nested topic",
            "state": "notStarted"
        }))
        .expect("camelCase operation");
        match operation {
            MapOperation::AddItem {
                item_id,
                parent_id,
                after_item_id,
                ..
            } => {
                assert_eq!(item_id.as_deref(), Some("child"));
                assert_eq!(parent_id.as_deref(), Some("root"));
                assert_eq!(after_item_id.as_deref(), Some("sibling"));
            }
            _ => panic!("expected addItem operation"),
        }
    }
}
