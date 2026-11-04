//! Actual native Comments → Source dispatch against moved private documents.
use super::*;
pub(super) async fn run(
    ui: &Rc<Ui>,
    root: &std::path::Path,
    capture: impl Fn(&str),
) -> Result<Value, String> {
    async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
        let start = std::time::Instant::now();
        while !test() {
            if start.elapsed() > Duration::from_secs(25) {
                return Err(format!("Comment location timed out: {step}"));
            }
            glib::timeout_future(Duration::from_millis(20)).await;
        }
        Ok(())
    }
    until("history", || ui.state.borrow().history_verified).await?;
    ui.composer
        .buffer()
        .set_text("Unsent relocation draft 中文😀");
    let mut results = Vec::new();
    let mut cases = vec![
        ("moved-source", 8, false, true),
        ("moved-preview", 7, false, false),
        ("ambiguous", 3, true, true),
        ("missing", 4, true, true),
        ("unchanged", 5, false, true),
        ("legacy", 7, false, false),
        ("legacy-source", 3, false, true),
    ];
    let reference = gtk::gio::spawn_blocking({
        let path = root.join("reference-line.json");
        move || {
            std::fs::read_to_string(path)
                .ok()
                .and_then(|s| serde_json::from_str::<usize>(&s).ok())
        }
    })
    .await
    .map_err(|_| "Comment fixture worker stopped")?;
    if let Some(line) = reference {
        cases.push(("reference", line, false, false));
    }
    for (id, expected_line, warning, source) in cases {
        let mut pulse = super::conversation_pages_acceptance::Probe::new();
        let started = std::time::Instant::now();
        ui.comment_panel(None);
        let panel = ui
            .annotations
            .borrow()
            .clone()
            .ok_or("Comments did not open")?;
        until("comment library", || {
            panel.smoke_open_record(id).unwrap_or(false)
        })
        .await?;
        if let Err(error) = until("located document", || {
            ui.reader.is_ready() && !panel.dialog.is_mapped() && ui.annotations.borrow().is_none()
        })
        .await
        {
            capture("comment-location-failure.png");
            return Err(format!(
                "{id}: {error}; dialog={} registered={} {}",
                panel.dialog.is_visible(),
                ui.annotations.borrow().is_some(),
                ui.reader.smoke_comment_debug()
            ));
        }
        if ui.current_tool() != Some(crate::workspace_navigation::Pane::Document) {
            return Err("Comments source did not present the native reader".into());
        }
        let actual = ui.reader.smoke_comment_position();
        if actual != (expected_line, warning, source) {
            return Err(format!(
                "{id}: expected {:?}, got {actual:?}",
                (expected_line, warning, source)
            ));
        }
        results.push(json!({"id":id,"line":actual.0,"unverified":actual.1,"source":actual.2,"openMs":started.elapsed().as_millis(),"heartbeat":pulse.stop()}));
        if id == "moved-source" {
            capture("comment-relocated-source.png");
        }
        if id == "moved-preview" {
            capture("comment-relocated-preview.png");
        }
        if id == "ambiguous" {
            capture("comment-ambiguous.png");
        }
        if !source && !warning {
            ui.reader.smoke_comment_view_switch(expected_line).await?;
        }
        ui.reader.smoke_close();
    }
    if ui.composer.buffer().text(
        &ui.composer.buffer().start_iter(),
        &ui.composer.buffer().end_iter(),
        true,
    ) != "Unsent relocation draft 中文😀"
    {
        return Err("Source relocation changed the conversation draft".into());
    }
    let guard = ui
        .reader
        .smoke_comment_open_guard(&root.join("project"))
        .await?;
    Ok(
        json!({"ok":true,"records":results,"nativeSourceAction":true,"draftRetained":true,"editGuard":guard}),
    )
}
