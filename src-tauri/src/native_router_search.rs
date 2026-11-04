//! Literal Unicode search over persisted jobs, using a read-only WAL snapshot.
use super::*;
use regex::RegexBuilder;
use rusqlite::{functions::FunctionFlags, OpenFlags};
use serde_json::{json, Value};
use std::{io, time::Instant};

pub fn validate(query: &str) -> Result<(), String> {
    if query.len() > 2048
        || query.chars().count() > 512
        || query
            .chars()
            .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'))
    {
        return Err(
            "Router search supports at most 512 characters, without control characters".into(),
        );
    }
    Ok(())
}
fn matches(job: &Job, needle: &regex::Regex) -> bool {
    [
        &job.prompt,
        &job.response,
        &job.context_excerpt,
        &job.error,
        &job.status,
    ]
    .into_iter()
    .any(|s| needle.is_match(s))
        || job.reference_labels.iter().any(|s| needle.is_match(s))
        || job
            .response_items
            .iter()
            .any(|item| needle.is_match(&item.text))
        || job.target.as_ref().is_some_and(|target| {
            [&target.title, &target.cwd, &target.session_key]
                .into_iter()
                .any(|s| needle.is_match(s))
        })
        || job.decision.as_ref().is_some_and(|decision| {
            [
                &decision.forwarded_prompt,
                &decision.reason,
                &decision.message,
            ]
            .into_iter()
            .any(|s| needle.is_match(s))
        })
}
pub fn page(
    path: &Path,
    controller: &str,
    query: &str,
    sensitive: bool,
    before: Option<i64>,
    after: Option<i64>,
) -> Result<Value, String> {
    page_with_budget(
        path,
        controller,
        query,
        sensitive,
        before,
        after,
        Duration::from_secs(2),
    )
}
pub(super) fn page_with_budget(
    path: &Path,
    controller: &str,
    query: &str,
    sensitive: bool,
    before: Option<i64>,
    after: Option<i64>,
    budget: Duration,
) -> Result<Value, String> {
    validate(query)?;
    let needle = RegexBuilder::new(&regex::escape(query))
        .case_insensitive(!sensitive)
        .size_limit(256 * 1024)
        .build()
        .map_err(|_| "Cannot prepare Router search")?;
    let mut db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| e.to_string())?;
    db.busy_timeout(Duration::from_millis(250))
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + budget;
    db.progress_handler(1000, Some(move || Instant::now() >= deadline))
        .map_err(|e| e.to_string())?;
    db.create_scalar_function(
        "studio_router_contains",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        move |context| {
            let raw = context.get_raw(0).as_str()?;
            if Instant::now() >= deadline || raw.len() > 2 * 1024 * 1024 {
                return Err(rusqlite::Error::UserFunctionError(Box::new(
                    io::Error::other("Router search budget exceeded"),
                )));
            }
            let job: Job = serde_json::from_str(raw)
                .map_err(|error| rusqlite::Error::UserFunctionError(Box::new(error)))?;
            Ok(matches(&job, &needle))
        },
    )
    .map_err(|e| e.to_string())?;
    // Both directions and boundary checks see the same records even while new
    // replies arrive. No Studio write mutex is held while scanning old jobs.
    let transaction = db.transaction().map_err(|e| e.to_string())?;
    let result = (|| {
        let mut rows = super::list_range_on(&transaction, controller, before, after, true)?;
        rows.truncate(20);
        let first = rows.first().and_then(|row| row["cursor"].as_i64());
        let last = rows.last().and_then(|row| row["cursor"].as_i64());
        let (older, newer) = super::neighbors_on(&transaction, controller, first, last, true)?;
        let summary = super::summary_on(&transaction, controller)?;
        Ok(
            json!({"data":rows,"nextCursor":if older{last}else{None},"newerCursor":if newer{first}else{None},"query":query,"caseSensitive":sensitive,"unreadCount":summary["unreadCount"],"activeIds":summary["activeIds"],"controllerBusy":summary["controllerBusy"]}),
        )
    })();
    result.map_err(|error: String| if Instant::now() >= deadline {
        "Router search exceeded its two-second budget. Refine the query; no partial results were returned.".into()
    } else { error })
}
