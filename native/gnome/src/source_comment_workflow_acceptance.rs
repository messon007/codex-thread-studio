use super::*;
use std::path::Path;
pub(super) async fn save_flow(
    ui: &Rc<Ui>,
    directory: &Path,
    value: &mut Value,
) -> Result<(), String> {
    let raw = value
        .pointer("/flowComments/_capture")
        .ok_or("Flow capture payload missing")?;
    let selected = crate::annotations::Capture {
        excerpt: raw["excerpt"]
            .as_str()
            .ok_or("Flow quote missing")?
            .to_owned(),
        source: raw["source"].clone(),
    };
    ui.comment_panel(Some(selected.clone()));
    let panel = ui
        .annotations
        .borrow()
        .clone()
        .ok_or("Flow capture did not open Comments")?;
    let saved = panel.smoke_save_source_capture(&selected).await?;
    if saved["source"] != selected.source || saved["source"]["anchor"]["textKind"] != "flow" {
        return Err("Flow comment identity changed while saving".into());
    }
    let path = directory.join("flow-comment-saved.json");
    let record = saved.clone();
    gio::spawn_blocking(move || std::fs::write(path, serde_json::to_vec_pretty(&record).unwrap()))
        .await
        .map_err(|_| "Flow record fixture writer stopped")?
        .map_err(|e| e.to_string())?;
    if !panel.smoke_open_record(saved["id"].as_str().ok_or("Flow comment ID missing")?)? {
        return Err("Saved flow comment could not reopen".into());
    }
    let started = std::time::Instant::now();
    while ui.annotations.borrow().is_some() || !ui.reader.is_ready() {
        if started.elapsed() > Duration::from_secs(20) {
            return Err("Saved flow comment reopen timed out".into());
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    if ui.reader.smoke_comment_position() != (1, false, false) {
        return Err("Saved flow comment was not verified in Preview".into());
    }
    ui.reader.smoke_close();
    let result = value
        .pointer_mut("/flowComments")
        .and_then(Value::as_object_mut)
        .ok_or("Flow result missing")?;
    result.remove("_capture");
    result.insert("explicitSaveAndReopen".into(), json!(true));
    Ok(())
}
pub(super) async fn run(ui: &Rc<Ui>, root: &Path, capture: impl Fn(&str)) -> Result<Value, String> {
    async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
        let start = std::time::Instant::now();
        while !test() {
            if start.elapsed() > Duration::from_secs(20) {
                return Err(format!("Source comments timed out: {step}"));
            }
            glib::timeout_future(Duration::from_millis(20)).await;
        }
        Ok(())
    }
    until("history", || ui.state.borrow().history_verified).await?;
    ui.composer
        .buffer()
        .set_text("Unsent code comment draft 中文😀");
    let project = root.join("project");
    let quote = "let target = \"中文😀\";  \r\nlet next = \"second\";\r\n";
    if std::env::var("STUDIO_GNOME_SMOKE_SOURCE_COMMENTS")
        .ok()
        .as_deref()
        == Some("capture")
    {
        ui.source
            .open_at(project.to_str().unwrap(), "code comments 中文.rs", 201);
        until("source", || {
            ui.source.smoke_ready() && ui.source.widget.is_mapped()
        })
        .await?;
        let selected = ui.source.smoke_capture(quote)?;
        if selected.source["anchor"]["startLine"] != 205
            || selected.source["anchor"]["endLine"] != 206
        {
            return Err(format!(
                "Source selection lost its physical lines: {:?}",
                selected.source
            ));
        }
        let panel = ui
            .annotations
            .borrow()
            .clone()
            .ok_or("Source button did not open Comments")?;
        let saved = panel.smoke_save_source_capture(&selected).await?;
        let id = saved["id"].as_str().ok_or("Saved comment ID missing")?;
        if !panel.smoke_open_record(id)? {
            return Err("Saved comment was not ready to reopen".into());
        }
        until("source reopen", || {
            ui.annotations.borrow().is_none() && ui.source.smoke_ready()
        })
        .await?;
        if ui.source.smoke_comment_location().0 != 205
            || !ui.source.visible_text().starts_with(quote)
        {
            return Err("Saved code comment did not reopen at its exact selection".into());
        }
        until("complete first source line", || {
            ui.source.smoke_first_line_visible()
        })
        .await?;
        capture("source-comment-captured.png");
        return Ok(
            json!({"ok":true,"record":saved,"sourceSelection":true,"exactCrLf":true,"explicitSave":true,"sourceReopen":true}),
        );
    }
    let record = gtk::gio::spawn_blocking({
        let path = root.join("expected-source-record.json");
        move || std::fs::read_to_string(path).map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "Source fixture worker stopped")??;
    let record: Value = serde_json::from_str(&record).map_err(|e| e.to_string())?;
    let id = record["id"].as_str().ok_or("Fixture comment ID missing")?;
    let mut results = Vec::new();
    for (id, line, warning, needle) in [
        (id, 208, false, quote),
        ("code-ambiguous", 1, true, "same"),
        ("code-missing", 3, true, ""),
        ("code-long", 1, false, "needle 中文😀"),
        ("code-patch", 5, false, "+new"),
    ] {
        ui.comment_panel(None);
        let panel = ui
            .annotations
            .borrow()
            .clone()
            .ok_or("Comments did not open")?;
        until("library", || panel.smoke_open_record(id).unwrap_or(false)).await?;
        if let Err(error) = until("source result", || {
            ui.annotations.borrow().is_none() && ui.source.smoke_ready()
        })
        .await
        {
            return Err(format!("{id}: {error}"));
        }
        let actual = ui.source.smoke_comment_location();
        if actual.0 != line
            || actual.2 != warning
            || (!needle.is_empty() && !ui.source.visible_text().starts_with(needle))
        {
            return Err(format!("{id}: unexpected source window {actual:?}"));
        }
        if id == "code-long" && actual.1 < 300_000 {
            return Err("Long-line quote stayed in its first window".into());
        }
        if ui.current_tool() != Some(crate::workspace_navigation::Pane::Source) {
            return Err("Code comment opened the Markdown pane".into());
        }
        results.push(json!({"id":id,"line":actual.0,"offset":actual.1,"unverified":actual.2}));
        if id == record["id"].as_str().unwrap() {
            until("relocated first line", || {
                ui.source.smoke_first_line_visible()
            })
            .await?;
            capture("source-comment-relocated.png");
        }
    }
    let rejected = ui.source.smoke_reject_non_file()?;
    ui.comment_panel(None);
    let panel = ui
        .annotations
        .borrow()
        .clone()
        .ok_or("Comments did not open")?;
    until("UTF-16 record", || {
        panel.smoke_open_record("code-utf16").unwrap_or(false)
    })
    .await?;
    until("unsupported encoding error", || {
        ui.annotations.borrow().is_none() && ui.source.smoke_error("UTF-8")
    })
    .await?;
    if ui.annotations.borrow().is_some() {
        return Err("Rejected source capture created a comment".into());
    }
    if ui.composer.buffer().text(
        &ui.composer.buffer().start_iter(),
        &ui.composer.buffer().end_iter(),
        true,
    ) != "Unsent code comment draft 中文😀"
    {
        return Err("Code comments changed the conversation draft".into());
    }
    Ok(
        json!({"ok":true,"records":results,"restart":true,"literalSource":true,"rejections":rejected,"unsupportedEncoding":true,"draftRetained":true}),
    )
}
