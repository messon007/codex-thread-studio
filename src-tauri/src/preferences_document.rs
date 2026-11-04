//! Shared bounded JSON document I/O for narrowly scoped settings transactions.
//! Call while holding the shared preferences lock, on a blocking worker.
use axum::http::StatusCode;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::Path,
};
pub type Error = (StatusCode, String);
const MAX_BYTES: usize = 2 * 1024 * 1024;
fn bad(error: impl ToString) -> Error {
    (StatusCode::BAD_REQUEST, error.to_string())
}
pub fn read(path: &Path) -> Result<Value, Error> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(serde_json::json!({})),
        Err(e) => return Err(bad(e)),
    };
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(bad)?;
    if bytes.len() > MAX_BYTES {
        return Err(bad("Studio preferences exceed the 2 MiB settings budget"));
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(bad)?;
    if !value.is_object() {
        return Err(bad("Studio preferences must be an object"));
    }
    Ok(value)
}
pub fn revision(value: &Value) -> Result<String, Error> {
    Ok(Sha256::digest(serde_json::to_vec(value).map_err(bad)?)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
pub fn write(path: &Path, value: &Value) -> Result<(), Error> {
    let bytes = serde_json::to_vec_pretty(value).map_err(bad)?;
    if bytes.len() > MAX_BYTES {
        return Err(bad("Studio preferences exceed the 2 MiB settings budget"));
    }
    write_atomic(path, &bytes)
}
/// Atomically persist already bounded/validated bytes without reformatting them.
/// Callers own their document budget and transaction lock.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    write_atomic_checked(path, bytes, || Ok(()))
}
/// Recheck an optimistic revision after staging and syncing, just before rename.
pub fn write_atomic_checked(
    path: &Path,
    bytes: &[u8],
    check: impl FnOnce() -> Result<(), Error>,
) -> Result<(), Error> {
    let parent = path
        .parent()
        .ok_or_else(|| bad("Missing settings directory"))?;
    std::fs::create_dir_all(parent).map_err(bad)?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(bad)?;
    file.write_all(bytes).map_err(bad)?;
    file.flush().map_err(bad)?;
    file.as_file().sync_all().map_err(bad)?;
    check()?;
    file.persist(path).map_err(bad)?;
    #[cfg(unix)]
    {
        if let Ok(directory) = std::fs::File::open(parent) {
            let _ = directory.sync_all();
        }
    }
    Ok(())
}
