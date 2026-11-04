//! Real panel signals against private provider history and private files.
use super::*;
use std::path::Path;
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let at = std::time::Instant::now();
    while !test() {
        if at.elapsed() > Duration::from_secs(20) {
            return Err(format!("Resource panel timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
fn find<T: IsA<gtk::Widget> + IsA<glib::Object> + Clone + glib::types::StaticType>(
    root: &gtk::Widget,
    test: &impl Fn(&T) -> bool,
) -> Option<T> {
    if let Ok(widget) = root.clone().downcast::<T>() {
        if test(&widget) {
            return Some(widget);
        }
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if let Some(found) = find(&widget, test) {
            return Some(found);
        }
    }
    None
}
fn button(root: &gtk::Widget, name: &str) -> Result<gtk::Button, String> {
    find(root, &|b: &gtk::Button| {
        b.is_mapped() && b.tooltip_text().as_deref() == Some(name)
    })
    .ok_or_else(|| format!("Missing resource control: {name}"))
}
fn label(root: &gtk::Widget, text: &str) -> bool {
    find(root, &|l: &gtk::Label| {
        l.is_mapped() && l.text().contains(text)
    })
    .is_some()
}
fn resource_button(root: &gtk::Widget, address: &str, name: &str) -> Result<gtk::Button, String> {
    let label = find(root, &|l: &gtk::Label| l.is_mapped() && l.text() == address)
        .ok_or("Missing resource card")?;
    button(&label.parent().ok_or("Missing resource card parent")?, name)
}
pub(super) async fn run(
    ui: &Rc<Ui>,
    directory: &Path,
    capture: impl Fn(&str),
) -> Result<Value, String> {
    until("history", || {
        ui.state.borrow().history_verified
            && ui
                .state
                .borrow()
                .items
                .iter()
                .any(|i| i.id == "resource-answer")
    })
    .await?;
    ui.composer
        .buffer()
        .set_text("Unsent resource panel draft 中文😀");
    let before = {
        let state = ui.state.borrow();
        (
            state.selected.clone(),
            state.active_turn.clone(),
            state.items.clone(),
        )
    };
    let mut pulse = super::conversation_pages_acceptance::Probe::new();
    gtk::prelude::WidgetExt::activate_action(&ui.window, "win.resources", None)
        .map_err(|e| e.to_string())?;
    let root = ui.resources.widget.upcast_ref::<gtk::Widget>();
    until("latest-turn index", || {
        label(root, "8 resources · 9 references")
    })
    .await?;
    if label(root, "old-only.md") {
        return Err("Older turn leaked into resources".into());
    }
    let search = find(root, &|e: &gtk::SearchEntry| {
        e.placeholder_text().as_deref() == Some("Search resources…")
    })
    .ok_or("Missing resource search")?;
    let filter = find(root, &|d: &gtk::DropDown| {
        d.model().is_some_and(|m| m.n_items() == 4)
    })
    .ok_or("Missing resource filter")?;
    search.set_text("README");
    until("search", || {
        label(root, "README.md") && ui.resources.smoke_state()["rows"] == json!(["README.md"])
    })
    .await?;
    button(root, "Open resource")?.emit_clicked();
    until("Markdown open", || {
        ui.reader.is_ready()
            && ui.current_tool() == Some(crate::workspace_navigation::Pane::Document)
    })
    .await?;
    ui.reader.smoke_close();
    until("return resources", || {
        ui.current_tool() == Some(crate::workspace_navigation::Pane::Resources)
            && button(root, "Go to message").is_ok()
    })
    .await?;
    if search.text() != "README" {
        return Err("Document return erased resource filter".into());
    }
    button(root, "Go to message")?.emit_clicked();
    until("source message", || {
        ui.history_context
            .borrow()
            .as_ref()
            .is_some_and(|v| v.contains("resource-answer"))
    })
    .await?;
    let dialog = ui.history_context.borrow().as_ref().unwrap().dialog.clone();
    dialog.close();
    until("source close", || ui.history_context.borrow().is_none()).await?;
    button(root, "Save resource")?.emit_clicked();
    until("favorite dialog", || ui.favorites.borrow().is_some()).await?;
    let library = ui.favorites.borrow().clone().unwrap();
    let saved = library.smoke_resource_save().await?;
    if saved.item_id != "resource-answer"
        || saved.turn_id != "resource-turn"
        || saved.content != "`README.md`"
        || saved.question != "Review `README.md`"
    {
        return Err(format!("Wrong saved resource source: {saved:?}"));
    }
    until("favorites close", || ui.favorites.borrow().is_none()).await?;
    filter.set_selected(3);
    search.set_text("source");
    until("code filter", || {
        ui.resources.smoke_state()["rows"] == json!(["source.rs:3"])
            && label(root, "source.rs:3")
            && button(root, "Open resource").is_ok()
    })
    .await?;
    button(root, "Open resource")?.emit_clicked();
    until("code source", || {
        ui.source
            .smoke_file_state()
            .is_some_and(|(p, _)| p.ends_with("source.rs"))
    })
    .await?;
    ui.source.smoke_close();
    until("code return", || {
        ui.current_tool() == Some(crate::workspace_navigation::Pane::Resources)
            && button(root, "Open resource").is_ok()
    })
    .await?;
    filter.set_selected(2);
    search.set_text("docs/");
    until("directory filter", || {
        resource_button(root, "docs/", "Open resource").is_ok()
    })
    .await?;
    resource_button(root, "docs/", "Open resource")?.emit_clicked();
    until("directory within project", || {
        ui.current_tool() == Some(crate::workspace_navigation::Pane::Files)
            && label(ui.files.widget.upcast_ref(), "nested.md")
    })
    .await?;
    button(ui.files.widget.upcast_ref(), "Close files")?.emit_clicked();
    until("directory return", || {
        ui.current_tool() == Some(crate::workspace_navigation::Pane::Resources)
            && button(root, "Open resource").is_ok()
    })
    .await?;
    search.set_text("sample");
    until("image filter", || {
        ui.resources.smoke_state()["rows"] == json!(["sample.png"])
            && label(root, "sample.png")
            && button(root, "Open resource").is_ok()
    })
    .await?;
    button(root, "Open resource")?.emit_clicked();
    until("image", || ui.image_viewer.smoke_ready()).await?;
    button(ui.image_viewer.widget.upcast_ref(), "Close image")?.emit_clicked();
    until("image return", || {
        ui.current_tool() == Some(crate::workspace_navigation::Pane::Resources)
            && button(root, "Open resource").is_ok()
    })
    .await?;
    filter.set_selected(1);
    search.set_text("");
    until("web filter", || {
        ui.resources.smoke_state()["rows"] == json!(["https://example.invalid/docs"])
            && label(root, "https://example.invalid/docs")
            && button(root, "Copy address").is_ok()
    })
    .await?;
    button(root, "Copy address")?.emit_clicked();
    let copied = root
        .clipboard()
        .read_text_future()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("No resource address copied")?;
    if copied != "https://example.invalid/docs" {
        return Err("Copy resource address was changed".into());
    }
    filter.set_selected(2);
    search.set_text("outside");
    until("blocked path", || {
        ui.resources.smoke_state()["rows"] == json!(["/outside/secret.md"])
            && label(root, "/outside/secret.md")
            && button(root, "The path is outside the current session root").is_ok()
    })
    .await?;
    if button(root, "The path is outside the current session root")?.is_sensitive() {
        return Err("Outside resource remained openable".into());
    }
    // An old result cannot replace the active filter, even if its session matches.
    let state = ui.state.borrow();
    let context = crate::session_search::Context {
        backend: state.backend.clone(),
        thread: state.selected.as_ref().unwrap().id.clone(),
        generation: state.socket_generation,
    };
    drop(state);
    ui.resources.event(crate::session_resources::Response {
        context,
        serial: 0,
        result: Ok(crate::session_resources::filtered(
            Arc::new(Default::default()),
            "",
            Default::default(),
        )),
    });
    if !label(root, "/outside/secret.md") {
        return Err("Stale resource response erased current rows".into());
    }
    crate::language::set("zh-CN", "en-US")?;
    crate::i18n_ui::apply();
    if !label(root, "资源") || search.text() != "outside" || !label(root, "/outside/secret.md") {
        return Err("Resource language change rewrote literal content".into());
    }
    crate::window_capture::next_frame(&ui.window).await?;
    capture("resources-chinese.png");
    crate::language::set("en-US", "en-US")?;
    crate::i18n_ui::apply();
    filter.set_selected(0);
    search.set_text("");
    until("all resources restored", || {
        ui.resources.smoke_state()["rows"]
            .as_array()
            .is_some_and(|r| r.len() == 8)
            && label(root, "README.md")
            && label(root, "sample.png")
    })
    .await?;
    button(root, "Refresh resources")?.emit_clicked();
    until("explicit refresh", || {
        ui.resources.smoke_state()["rows"]
            .as_array()
            .is_some_and(|r| r.len() == 8)
    })
    .await?;
    crate::window_capture::next_frame(&ui.window).await?;
    capture("resources-panel.png");
    filter.set_selected(3);
    search.set_text("source");
    until("saved filter", || {
        ui.resources.smoke_state()["rows"] == json!(["source.rs:3"])
    })
    .await?;
    let child = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|t| t.id == "child")
        .cloned()
        .ok_or("Missing alternate session")?;
    ui.select(child);
    until("alternate session", || {
        ui.state.borrow().history_verified
            && ui
                .state
                .borrow()
                .selected
                .as_ref()
                .is_some_and(|t| t.id == "child")
            && label(root, "0 resources")
    })
    .await?;
    ui.select(before.0.clone().ok_or("Missing original session")?);
    until("session filter restoration", || {
        ui.state.borrow().history_verified
            && ui.resources.smoke_state()["rows"] == json!(["source.rs:3"])
            && search.text() == "source"
            && filter.selected() == 3
    })
    .await?;
    filter.set_selected(2);
    search.set_text("中文.md");
    until("decoded filename search", || {
        ui.resources.smoke_state()["rows"] == json!(["docs/%E4%B8%AD%E6%96%87.md"])
    })
    .await?;
    button(root, "Open resource")?.emit_clicked();
    until("encoded document", || {
        ui.reader.is_ready()
            && ui
                .reader
                .location()
                .is_some_and(|(_, p)| p.ends_with("中文.md"))
    })
    .await?;
    ui.reader.smoke_close();
    until("encoded return", || {
        ui.current_tool() == Some(crate::workspace_navigation::Pane::Resources)
            && button(root, "Open resource").is_ok()
    })
    .await?;
    search.set_text("shared-resource");
    let shared_path = directory
        .join("shared/shared-resource.md")
        .to_string_lossy()
        .into_owned();
    until("unconfigured shared file", || {
        ui.resources.smoke_state()["rows"] == json!([shared_path])
            && button(root, "The path is outside the current session root").is_ok()
    })
    .await?;
    ui.open_settings();
    let settings = ui.settings.borrow().clone().ok_or("Missing settings")?;
    let shared = settings.smoke_shared();
    until("shared settings loaded", || shared.may_close()).await?;
    until("shared settings mapped", || {
        button(shared.dialog.upcast_ref(), "Add folder").is_ok()
    })
    .await?;
    let input = find(shared.dialog.upcast_ref(), &|e: &adw::EntryRow| {
        e.title() == "Absolute folder path"
    })
    .ok_or("Missing shared folder input")?;
    input.set_text(&directory.join("shared").to_string_lossy());
    button(shared.dialog.upcast_ref(), "Add folder")?.emit_clicked();
    let save = find(shared.dialog.upcast_ref(), &|b: &gtk::Button| {
        b.is_mapped() && b.label().as_deref() == Some("Save")
    })
    .ok_or("Missing shared Save")?;
    save.emit_clicked();
    until("shared folder saved", || shared.may_close()).await?;
    shared.dialog.close();
    settings.dialog.close();
    until("resource authorization updated after Save", || {
        ui.resources.smoke_state()["rows"] == json!([shared_path])
            && button(root, "Open resource").is_ok_and(|b| b.is_sensitive())
    })
    .await?;
    button(root, "Open resource")?.emit_clicked();
    until("shared document", || {
        ui.reader.is_ready()
            && ui
                .reader
                .location()
                .is_some_and(|(_, p)| p.ends_with("shared-resource.md"))
    })
    .await?;
    ui.reader.smoke_close();
    until("shared return", || {
        ui.current_tool() == Some(crate::workspace_navigation::Pane::Resources)
    })
    .await?;
    let stats = pulse.stop();
    let unchanged = {
        let state = ui.state.borrow();
        state.selected == before.0
            && state.active_turn == before.1
            && state.items == before.2
            && ui.composer_text() == "Unsent resource panel draft 中文😀"
    };
    if !unchanged {
        return Err("Resource operations changed the live conversation or draft".into());
    }
    Ok(
        json!({"ok":true,"latestTurn":true,"searchAndFilters":true,"decodedFilenameSearch":true,"sharedFolderSaveRefreshesResources":true,"refresh":true,"sessionFilterRestoration":true,"markdownImageCodeAndDirectory":true,"sourceContext":true,"favorite":saved,"blockedOutsideRoot":true,"copyAddress":true,"staleReplyRejected":true,"liveLanguage":true,"draftAndConversationUnchanged":true,"responsiveness":stats}),
    )
}
