//! Private GTK acceptance waits; never blocks the application main loop.
pub async fn until(label: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !test() {
        if start.elapsed() > std::time::Duration::from_secs(20) {
            return Err(format!("Timed out: {label}"));
        }
        gtk::glib::timeout_future(std::time::Duration::from_millis(20)).await;
    }
    Ok(())
}
