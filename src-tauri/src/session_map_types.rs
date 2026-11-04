//! Presentation-neutral Session Map wire types shared by both frontends.
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const MAX_MAP_BODY_BYTES: usize = 512 * 1024;
pub const MAX_ITEMS: usize = 1_000;
pub const MAX_RELATIONS: usize = 2_000;
pub const MAX_OPERATIONS: usize = 100;
pub const MAX_DEPTH: usize = 16;

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

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOperationsRequest {
    pub base_revision: u64,
    #[serde(default)]
    pub map_id: Option<String>,
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

pub fn validate_snapshot(map: &SessionMap) -> Result<(), String> {
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
    let by_id: std::collections::HashMap<_, _> = map
        .items
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect();
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
            current = by_id
                .get(parent_id)
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
