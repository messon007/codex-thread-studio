//! All callers share the runtime's bounded CPU pool; GTK and socket I/O stay free.
pub async fn compute<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| format!("Native worker stopped: {error}"))?
}
