//! Read-only native receipt search. A bounded miss never means "not submitted".
//! Avoid full turn hydration: large tool outputs are irrelevant to message identity.
use crate::codex_app_server::CodexAppServer;
use serde_json::{json, Value};
use std::{collections::BTreeSet, time::Duration};
type Result<T> = std::result::Result<T, String>;

pub async fn find_native_turn(
    server: &CodexAppServer,
    thread: &str,
    client_id: &str,
    native_turn_id: Option<&str>,
) -> Result<Option<Value>> {
    tokio::time::timeout(
        Duration::from_secs(15),
        search(server, thread, client_id, native_turn_id),
    )
    .await
    .map_err(|_| "Native receipt search timed out; delivery remains uncertain".to_owned())?
}
fn valid_turn(turn: &Value) -> Result<&str> {
    let id = turn["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("Native turn lacks an ID")?;
    if !matches!(
        turn["status"].as_str(),
        Some("inProgress" | "completed" | "interrupted" | "failed")
    ) {
        return Err("Unrecognized native turn status; receipt remains uncertain".into());
    }
    Ok(id)
}
fn matched_item(entry: &Value, turn: &str, client: &str) -> Result<bool> {
    if entry["turnId"].as_str().is_some_and(|id| id != turn) {
        return Err("Native item belongs to a different turn".into());
    }
    let item = &entry["item"];
    Ok(item["type"] == "userMessage" && item["clientId"].as_str() == Some(client))
}
fn keep_match(found: &mut Option<Value>, turn: Value) -> Result<()> {
    valid_turn(&turn)?;
    if found
        .as_ref()
        .is_some_and(|previous| previous["id"] != turn["id"])
    {
        return Err(
            "Multiple native turns match this stable message ID; manual reconciliation required"
                .into(),
        );
    }
    *found = Some(turn);
    Ok(())
}
async fn search(
    server: &CodexAppServer,
    thread: &str,
    client: &str,
    known: Option<&str>,
) -> Result<Option<Value>> {
    if thread.is_empty() || client.is_empty() {
        return Err("Missing native receipt identity".into());
    }
    let mut cursor = Value::Null;
    let mut cursors = BTreeSet::new();
    let mut found = None;
    // 64 recent turns at most. Missing older evidence stays unresolved in the UI.
    for _ in 0..4 {
        let page=server.request("thread/turns/list",json!({"threadId":thread,"limit":16,"cursor":cursor,"sortDirection":"desc","itemsView":"notLoaded"})).await?;
        let turns = page["data"].as_array().ok_or("Invalid native turn page")?;
        for turn in turns {
            let id = valid_turn(turn)?;
            if let Some(known) = known {
                if id == known {
                    return Ok(Some(turn.clone()));
                }
                continue;
            }
            let mut item_cursor = Value::Null;
            let mut item_cursors = BTreeSet::new();
            let mut matched = false;
            for _ in 0..4 {
                let items=server.request("thread/items/list",json!({"threadId":thread,"turnId":id,"limit":64,"cursor":item_cursor,"sortDirection":"asc","deferOutput":true})).await?;
                for item in items["data"].as_array().ok_or("Invalid native item page")? {
                    if matched_item(item, id, client)? {
                        matched = true;
                    }
                }
                if matched {
                    keep_match(&mut found, turn.clone())?;
                    break;
                }
                let Some(next) = items["nextCursor"].as_str() else {
                    break;
                };
                if !item_cursors.insert(next.to_owned()) {
                    return Err("Repeated native item cursor".into());
                }
                item_cursor = json!(next);
            }
        }
        let Some(next) = page["nextCursor"].as_str() else {
            return Ok(found);
        };
        if !cursors.insert(next.to_owned()) {
            return Err("Repeated native turn cursor".into());
        }
        cursor = json!(next);
    }
    Ok(found)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_native_client_identity_counts_as_a_message_receipt() {
        assert!(matched_item(&json!({"turnId":"turn","item":{"type":"userMessage","clientId":"stable","content":[]}}),"turn","stable").unwrap());
        assert!(!matched_item(
            &json!({"item":{"type":"agentMessage","clientId":"stable","text":"same text"}}),
            "turn",
            "stable"
        )
        .unwrap());
        assert!(!matched_item(&json!({"item":{"type":"userMessage","clientId":null,"content":[{"type":"text","text":"stable"}]}}),"turn","stable").unwrap());
        assert!(matched_item(
            &json!({"turnId":"other","item":{"type":"userMessage","clientId":"stable"}}),
            "turn",
            "stable"
        )
        .is_err());
    }
    #[test]
    fn duplicate_stable_ids_and_unknown_statuses_are_not_conclusive() {
        let mut found = None;
        keep_match(&mut found, json!({"id":"one","status":"completed"})).unwrap();
        assert!(keep_match(&mut found, json!({"id":"two","status":"completed"})).is_err());
        assert!(valid_turn(&json!({"id":"one","status":"new-status"})).is_err());
        assert!(found.is_some());
    }
}
