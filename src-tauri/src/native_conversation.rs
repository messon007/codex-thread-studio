//! Authenticated read-only conversation RPC for native paging/search clients.
use crate::{json_error, json_response, GatewayState};
use axum::{
    body::Body,
    extract::{rejection::JsonRejection, State},
    http::{Response, StatusCode},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Read {
    backend: String,
    #[serde(default)]
    thread_id: String,
    operation: Operation,
    #[serde(default)]
    search_term: Option<String>,
    #[serde(default)]
    cursor: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Operation {
    Catalog,
    Search,
    Turns,
    Items,
    States,
    Read,
    Summary,
    Opening,
}
impl Read {
    fn rpc(self) -> Result<(String, &'static str, Value), String> {
        if !matches!(self.operation, Operation::Catalog)
            && (self.thread_id.is_empty()
                || self.thread_id.len() > 256
                || self.thread_id.chars().any(char::is_control))
        {
            return Err("Invalid conversation identifier".into());
        }
        let summary = matches!(self.operation, Operation::Summary);
        let (method, params) = match self.operation {
            Operation::Catalog => {
                if !self.thread_id.is_empty()
                    || self.search_term.is_some()
                    || self
                        .cursor
                        .as_ref()
                        .is_some_and(|cursor| cursor.len() > 4096)
                {
                    return Err("Invalid Router catalog parameters".into());
                }
                (
                    "thread/list",
                    json!({"limit":100,"cursor":self.cursor,"useStateDbOnly":true,"archived":false}),
                )
            }
            Operation::Search => {
                if self.cursor.is_some() {
                    return Err("Search does not accept a cursor".into());
                }
                let search_term = self.search_term.ok_or("Search text is required")?;
                if search_term.is_empty() || search_term.len() > 4096 {
                    return Err("Search text must contain 1–4096 bytes".into());
                }
                (
                    "thread/searchOccurrences",
                    json!({"threadId":self.thread_id,"searchTerm":search_term,"limit":100}),
                )
            }
            Operation::Turns | Operation::Items | Operation::States | Operation::Opening => {
                let items = matches!(self.operation, Operation::Items);
                let states = matches!(self.operation, Operation::States);
                let opening = matches!(self.operation, Operation::Opening);
                if self.search_term.is_some() {
                    return Err("Turn paging does not accept search text".into());
                }
                let cursor = self.cursor;
                if cursor.as_ref().is_some_and(|cursor| cursor.len() > 4096) {
                    return Err("Conversation cursor exceeds its limit".into());
                }
                let mut params = if items {
                    json!({"threadId":self.thread_id,"limit":120,"sortDirection":"desc"})
                } else {
                    json!({"threadId":self.thread_id,"limit":4,"sortDirection":if opening {"asc"}else{"desc"},"itemsView":"full"})
                };
                if states {
                    params["itemsView"] = "summary".into();
                    if cursor.is_some() {
                        return Err("Turn state reads do not accept a cursor".into());
                    }
                }
                if let Some(cursor) = cursor {
                    params["cursor"] = cursor.into();
                }
                (
                    if items {
                        "thread/items/list"
                    } else {
                        "thread/turns/list"
                    },
                    params,
                )
            }
            Operation::Read | Operation::Summary => {
                if self.search_term.is_some() || self.cursor.is_some() {
                    return Err("History read does not accept paging/search parameters".into());
                }
                (
                    "thread/read",
                    json!({"threadId":self.thread_id,"includeTurns":!summary}),
                )
            }
        };
        Ok((self.backend, method, params))
    }
}
pub async fn read(
    State(state): State<GatewayState>,
    body: Result<Json<Read>, JsonRejection>,
) -> Response<Body> {
    let request = match body {
        Ok(Json(value)) => value,
        Err(_) => return json_error(StatusCode::BAD_REQUEST, "Invalid native conversation read"),
    };
    let (backend, method, params) = match request.rpc() {
        Ok(value) => value,
        Err(error) => return json_error(StatusCode::BAD_REQUEST, &error),
    };
    let Some(instance) = state.codex_backends.get(&backend) else {
        return json_error(
            StatusCode::BAD_REQUEST,
            "Choose a configured Codex-compatible backend for this read",
        );
    };
    let result = if method == "thread/read" && params["includeTurns"] == false {
        crate::native_router_backend::read(&instance.server, method, params).await
    } else {
        instance.server.request_private_read(method, params).await
    };
    // JSON serialization of a history page belongs off the async delivery lane.
    match tokio::task::spawn_blocking(move || match result {
        Ok(value) => json_response(StatusCode::OK, &value),
        Err(error) => json_error(StatusCode::BAD_GATEWAY, &error),
    })
    .await
    {
        Ok(response) => response,
        Err(_) => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Native read worker stopped",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn read_operations_are_whitelisted_and_preserve_literal_search_text() {
        let catalog: Read = serde_json::from_value(
            json!({"backend":"codex","operation":"catalog","cursor":"opaque:中文/+?"}),
        )
        .unwrap();
        let (_, method, params) = catalog.rpc().unwrap();
        assert_eq!(method, "thread/list");
        assert_eq!(params["cursor"], "opaque:中文/+?");
        assert_eq!(params["useStateDbOnly"], true);
        let request:Read=serde_json::from_value(json!({"backend":"codex","threadId":"thread","operation":"opening","cursor":"first-page"})).unwrap();
        let (_, method, params) = request.rpc().unwrap();
        assert_eq!(method, "thread/turns/list");
        assert_eq!(params["sortDirection"], "asc");
        assert_eq!(params["itemsView"], "full");
        assert_eq!(params["limit"], 4);
        assert_eq!(params["cursor"], "first-page");
        let request: Read = serde_json::from_value(json!({"backend":"codex","threadId":"thread","operation":"search","searchTerm":"space "})).unwrap();
        let (_, method, params) = request.rpc().unwrap();
        assert_eq!(method, "thread/searchOccurrences");
        assert_eq!(params["searchTerm"], "space ");
        let request: Read = serde_json::from_value(
            json!({"backend":"codex","threadId":"thread","operation":"turns","cursor":"next"}),
        )
        .unwrap();
        let (_, method, params) = request.rpc().unwrap();
        assert_eq!(method, "thread/turns/list");
        assert_eq!(params["limit"], 4);
        assert_eq!(params["itemsView"], "full");
        let request:Read=serde_json::from_value(json!({"backend":"codex","threadId":"thread","operation":"items","cursor":"opaque:中文/+?"})).unwrap();
        let (_, method, params) = request.rpc().unwrap();
        assert_eq!(method, "thread/items/list");
        assert_eq!(params["limit"], 120);
        assert_eq!(params["sortDirection"], "desc");
        assert_eq!(params["cursor"], "opaque:中文/+?");
        assert!(params.get("itemsView").is_none());
        let request: Read = serde_json::from_value(
            json!({"backend":"codex","threadId":"thread","operation":"states"}),
        )
        .unwrap();
        let (_, method, params) = request.rpc().unwrap();
        assert_eq!(method, "thread/turns/list");
        assert_eq!(
            params,
            json!({"threadId":"thread","limit":4,"sortDirection":"desc","itemsView":"summary"})
        );
        let request: Read = serde_json::from_value(
            json!({"backend":"codex","threadId":"thread","operation":"states","cursor":"extra"}),
        )
        .unwrap();
        assert!(request.rpc().is_err());
        let request: Read = serde_json::from_value(
            json!({"backend":"codex","threadId":"thread","operation":"summary"}),
        )
        .unwrap();
        let (_, method, params) = request.rpc().unwrap();
        assert_eq!(method, "thread/read");
        assert_eq!(params["includeTurns"], false);
        assert!(serde_json::from_value::<Read>(
            json!({"backend":"codex","threadId":"thread","operation":"turn/start"})
        )
        .is_err());
    }
}
