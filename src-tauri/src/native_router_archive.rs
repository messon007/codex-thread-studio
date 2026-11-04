//! Read-only browser Router records. These are never native delivery intents.
use super::*;
use regex::RegexBuilder;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use sha2::{Digest, Sha256};
use std::{path::Path, time::Instant};
#[path = "native_router_archive_source.rs"]
mod source;

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Read {
    controller: String,
    id: String,
    before: Option<i64>,
    after: Option<i64>,
    query: String,
    case_sensitive: bool,
}
#[derive(Clone)]
pub(super) struct Record {
    pub id: String,
    pub controller: String,
    pub turn_key: String,
    pub receipt: String,
    pub saved: Value,
}
fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)[..16]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn identity(row: i64, controller: &str, key: &str) -> String {
    format!(
        "browser-{row:x}-{}",
        digest(format!("{controller}\0{key}").as_bytes())
    )
}
fn row_id(id: &str) -> Result<i64, String> {
    let rest = id
        .strip_prefix("browser-")
        .ok_or("Invalid browser history identity")?;
    let (row, hash) = rest
        .split_once('-')
        .ok_or("Invalid browser history identity")?;
    if id.len() > 64
        || row.is_empty()
        || hash.len() != 32
        || !hash.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("Invalid browser history identity".into());
    }
    i64::from_str_radix(row, 16)
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| "Invalid browser history cursor".into())
}
fn record(row: i64, controller: String, key: String, raw: String) -> Result<Record, String> {
    if !crate::valid_router_session_key(&controller)
        || controller.len() > 320
        || !crate::valid_router_session_key(&key)
        || key.len() > 320
        || raw.len() > 128 * 1024
    {
        return Err("Invalid saved browser Router record".into());
    }
    let saved: Value =
        serde_json::from_str(&raw).map_err(|_| "Invalid saved browser Router JSON")?;
    if !saved.is_object() {
        return Err("Invalid saved browser Router record".into());
    }
    Ok(Record {
        id: identity(row, &controller, &key),
        controller,
        turn_key: key,
        receipt: digest(raw.as_bytes()),
        saved,
    })
}
fn connection(path: &Path) -> Result<Option<Connection>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| e.to_string())?;
    db.busy_timeout(Duration::from_millis(250))
        .map_err(|e| e.to_string())?;
    let exists:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='router_dispatches')",[],|r|r.get(0)).map_err(|e|e.to_string())?;
    Ok(exists.then_some(db))
}
fn native_filter(db: &Connection) -> Result<&'static str, String> {
    let exists:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='native_router_jobs')",[],|r|r.get(0)).map_err(|e|e.to_string())?;
    Ok(if exists {
        " AND NOT EXISTS(SELECT 1 FROM native_router_jobs n WHERE n.controller=d.controller AND d.turn_key=substr(d.controller,1,instr(d.controller,':'))||'native-'||n.id AND n.id=CASE WHEN json_valid(d.dispatch) THEN json_extract(d.dispatch,'$.nativeJobId') END)"
    } else {
        ""
    })
}
fn empty(query: &Read) -> Value {
    json!({"data":[],"nextCursor":null,"newerCursor":null,"unreadCount":0,"activeIds":[],"controllerBusy":false,"browserHistory":true,"query":query.query,"caseSensitive":query.case_sensitive})
}
fn page(path: &Path, query: &Read) -> Result<Value, String> {
    let Some(mut db) = connection(path)? else {
        return Ok(empty(query));
    };
    let deadline = Instant::now() + Duration::from_secs(2);
    db.progress_handler(1000, Some(move || Instant::now() >= deadline))
        .map_err(|e| e.to_string())?;
    let filter = native_filter(&db)?;
    let needle = RegexBuilder::new(&regex::escape(&query.query))
        .case_insensitive(!query.case_sensitive)
        .size_limit(256 * 1024)
        .build()
        .map_err(|e| e.to_string())?;
    db.create_scalar_function(
        "studio_browser_contains",
        1,
        rusqlite::functions::FunctionFlags::SQLITE_UTF8
            | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
        move |context| {
            let raw = context.get_raw(0).as_str()?;
            if raw.len() > 128 * 1024 || Instant::now() >= deadline {
                return Err(rusqlite::Error::UserFunctionError(Box::new(
                    std::io::Error::other("Browser history search budget exceeded"),
                )));
            }
            let value: Value = serde_json::from_str(raw)
                .map_err(|e| rusqlite::Error::UserFunctionError(Box::new(e)))?;
            // Search decoded string values, never JSON escaping or property names.
            fn matches(v: &Value, re: &regex::Regex) -> bool {
                match v {
                    Value::String(s) => re.is_match(s),
                    Value::Array(a) => a.iter().any(|v| matches(v, re)),
                    Value::Object(o) => o.values().any(|v| matches(v, re)),
                    _ => false,
                }
            }
            Ok(matches(&value, &needle))
        },
    )
    .map_err(|e| e.to_string())?;
    let tx = db.transaction().map_err(|e| e.to_string())?;
    let search = if query.query.is_empty() {
        ""
    } else {
        " AND studio_browser_contains(d.dispatch)"
    };
    let direction = if query.after.is_some() { "ASC" } else { "DESC" };
    let sql=format!("SELECT d.rowid,d.controller,d.turn_key,d.dispatch FROM router_dispatches d WHERE d.controller=?1 AND (?2 IS NULL OR d.rowid<?2) AND (?3 IS NULL OR d.rowid>?3){filter}{search} ORDER BY d.rowid {direction} LIMIT 21");
    let mut statement = tx.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = statement
        .query_map(params![query.controller, query.before, query.after], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut rows = rows
        .map(|r| {
            let (cursor, c, k, raw) = r.map_err(|e| e.to_string())?;
            record(cursor, c, k, raw).map(|r| (cursor, r))
        })
        .collect::<Result<Vec<_>, String>>()?;
    rows.truncate(20);
    if query.after.is_some() {
        rows.reverse();
    }
    let first = rows.first().map(|r| r.0);
    let last = rows.last().map(|r| r.0);
    let neighbors=format!("SELECT EXISTS(SELECT 1 FROM router_dispatches d WHERE d.controller=?1 AND d.rowid<?2{filter}{search}),EXISTS(SELECT 1 FROM router_dispatches d WHERE d.controller=?1 AND d.rowid>?3{filter}{search})");
    let (older, newer): (bool, bool) = tx
        .query_row(&neighbors, params![query.controller, last, first], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .map_err(|e| e.to_string())?;
    let values=rows.into_iter().map(|(cursor,r)|json!({"cursor":cursor,"id":r.id,"controller":r.controller,"receipt":r.receipt,"status":r.saved["status"].as_str().unwrap_or("unknown"),"unread":false})).collect::<Vec<_>>();
    Ok(
        json!({"data":values,"nextCursor":if older{last}else{None},"newerCursor":if newer{first}else{None},"unreadCount":0,"activeIds":[],"controllerBusy":false,"browserHistory":true,"query":query.query,"caseSensitive":query.case_sensitive}),
    )
}
fn load(path: &Path, id: &str) -> Result<Record, String> {
    let row = row_id(id)?;
    let db = connection(path)?.ok_or("Saved browser request was not found")?;
    let filter = native_filter(&db)?;
    let (c,k,raw):(String,String,String)=db.query_row(&format!("SELECT d.controller,d.turn_key,d.dispatch FROM router_dispatches d WHERE d.rowid=?1{filter}"),[row],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(|e|e.to_string())?.ok_or("Saved browser request was not found")?;
    let record = record(row, c, k, raw)?;
    if record.id != id {
        return Err("Saved browser request identity changed; refresh history".into());
    }
    Ok(record)
}
pub async fn get_archive(
    State(state): State<GatewayState>,
    Query(query): Query<Read>,
) -> Response<Body> {
    if query.before.is_some() && query.after.is_some()
        || query.before.is_some_and(|n| n <= 0)
        || query.after.is_some_and(|n| n <= 0)
        || query.id.is_empty()
            && (!crate::valid_router_session_key(&query.controller) || query.controller.len() > 320)
        || !query.id.is_empty()
            && (!query.controller.is_empty()
                || query.before.is_some()
                || query.after.is_some()
                || !query.query.is_empty()
                || row_id(&query.id).is_err())
    {
        return json_error(
            StatusCode::BAD_REQUEST,
            "Invalid browser Router history query",
        );
    }
    if let Err(e) = store::search::validate(&query.query) {
        return json_error(StatusCode::BAD_REQUEST, &e);
    }
    let Ok(permit) = state.router_runtime.archives.clone().try_acquire_owned() else {
        return json_error(
            StatusCode::TOO_MANY_REQUESTS,
            "Browser history readers are busy; try again",
        );
    };
    let path = state.studio_path.clone();
    if query.id.is_empty() {
        let value = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            page(&path, &query)
        })
        .await;
        return match value {
            Ok(Ok(v)) => json_response(StatusCode::OK, &v),
            Ok(Err(e)) => json_error(StatusCode::BAD_REQUEST, &e),
            Err(_) => json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Browser history reader stopped",
            ),
        };
    }
    let id = query.id;
    let record = match tokio::task::spawn_blocking(move || load(&path, &id)).await {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => return json_error(StatusCode::CONFLICT, &e),
        Err(_) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Browser history reader stopped",
            )
        }
    };
    let mut value = source::view(&state, &record).await;
    let path = state.studio_path.clone();
    let before = record.clone();
    let current = tokio::task::spawn_blocking(move || {
        load(&path, &before.id).map(|r| r.receipt == before.receipt)
    })
    .await;
    drop(permit);
    if !matches!(current, Ok(Ok(true))) {
        return json_error(
            StatusCode::CONFLICT,
            "Saved browser request changed while loading; refresh history",
        );
    }
    value["receipt"] = record.receipt.into();
    json_response(StatusCode::OK, &value)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn saved(path: &Path, key: &str, value: Value) {
        crate::router_history::save(
            path,
            &crate::router_history::Record {
                controller: "codex:router".into(),
                turn_key: key.into(),
                dispatch: value,
            },
        )
        .unwrap();
    }
    #[test]
    fn browser_pages_search_and_newer_boundaries_are_bounded_and_read_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("studio.sqlite3");
        assert!(page(&path, &Read::default()).unwrap()["data"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(!path.exists());
        for n in 0..53 {
            saved(
                &path,
                &format!("codex:turn-{n}"),
                json!({"status":"completed","future":{"text":"中文😀 quoted \"value\"  "},"directTurn":{"items":[{"type":"userMessage","content":[{"type":"text","text":format!("Question {n}")}]}]}}),
            );
        }
        let original = crate::router_history::load(&path, "codex:router")
            .unwrap()
            .into_iter()
            .map(|r| r.dispatch)
            .collect::<Vec<_>>();
        let mut query = Read {
            controller: "codex:router".into(),
            ..Read::default()
        };
        let first = page(&path, &query).unwrap();
        assert_eq!(first["data"].as_array().unwrap().len(), 20);
        query.before = first["nextCursor"].as_i64();
        let middle = page(&path, &query).unwrap();
        assert_eq!(middle["data"].as_array().unwrap().len(), 20);
        query.before = middle["nextCursor"].as_i64();
        let last = page(&path, &query).unwrap();
        assert_eq!(last["data"].as_array().unwrap().len(), 13);
        assert!(last["nextCursor"].is_null());
        query.before = None;
        query.after = last["newerCursor"].as_i64();
        assert_eq!(page(&path, &query).unwrap()["data"], middle["data"]);
        query.after = None;
        query.query = "quoted \"value\"  ".into();
        assert_eq!(
            page(&path, &query).unwrap()["data"]
                .as_array()
                .unwrap()
                .len(),
            20
        );
        query.query = "quoted \"value\"   ".into();
        assert!(page(&path, &query).unwrap()["data"]
            .as_array()
            .unwrap()
            .is_empty());
        query.query = "question 52".into();
        assert_eq!(
            page(&path, &query).unwrap()["data"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        query.case_sensitive = true;
        assert!(page(&path, &query).unwrap()["data"]
            .as_array()
            .unwrap()
            .is_empty());
        let id = first["data"][0]["id"].as_str().unwrap();
        assert_eq!(load(&path, id).unwrap().turn_key, "codex:turn-52");
        assert_eq!(
            original,
            crate::router_history::load(&path, "codex:router")
                .unwrap()
                .into_iter()
                .map(|r| r.dispatch)
                .collect::<Vec<_>>()
        );
    }
    #[test]
    fn identity_rejects_row_reuse_and_native_mirrors_without_hiding_orphans() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("studio.sqlite3");
        saved(&path, "codex:old", json!({"status":"running"}));
        let q = Read {
            controller: "codex:router".into(),
            ..Read::default()
        };
        let p = page(&path, &q).unwrap();
        let id = p["data"][0]["id"].as_str().unwrap();
        let db = Connection::open(&path).unwrap();
        db.execute("DELETE FROM router_dispatches", []).unwrap();
        saved(&path, "codex:new", json!({"status":"routing"}));
        assert!(load(&path, id).is_err());
        db.execute_batch("CREATE TABLE native_router_jobs(id TEXT PRIMARY KEY,controller TEXT);")
            .unwrap();
        db.execute(
            "INSERT INTO native_router_jobs VALUES('native-id','codex:router')",
            [],
        )
        .unwrap();
        saved(
            &path,
            "codex:native-native-id",
            json!({"nativeJobId":"native-id"}),
        );
        saved(
            &path,
            "codex:not-a-mirror",
            json!({"nativeJobId":"native-id"}),
        );
        saved(
            &path,
            "codex:orphan",
            json!({"nativeJobId":"missing-native-id"}),
        );
        let p = page(&path, &q).unwrap();
        assert_eq!(p["data"].as_array().unwrap().len(), 3);
        assert!(p["activeIds"].as_array().unwrap().is_empty());
        assert_eq!(p["controllerBusy"], false);
        for bad in [
            "browser-0-00000000000000000000000000000000",
            "browser-1-nohash",
            "../1",
            "browser-ffffffffffffffff-00000000000000000000000000000000",
        ] {
            assert!(row_id(bad).is_err());
        }
    }
}
