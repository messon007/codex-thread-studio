use super::*;
use std::path::Path;
async fn until(test: impl Fn() -> bool) -> Result<(), String> {
    let at = std::time::Instant::now();
    while !test() {
        if at.elapsed() > Duration::from_secs(20) {
            return Err("Native language fixture timed out".into());
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
fn find(widget: &gtk::Widget, test: &impl Fn(&gtk::Widget) -> bool) -> Option<gtk::Widget> {
    if test(widget) {
        return Some(widget.clone());
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        if let Some(found) = find(&current, test) {
            return Some(found);
        }
    }
    None
}
pub(super) async fn run(ui: &Rc<Ui>, directory: &Path) -> Result<Value, String> {
    until(|| ui.state.borrow().history_verified && ui.language.smoke_language().is_some()).await?;
    crate::smoke_input::request(directory, json!({"action":"focus"})).await?;
    if std::env::var("STUDIO_GNOME_SMOKE_LANGUAGE").ok().as_deref() == Some("restart") {
        if ui.language.smoke_language().as_deref() != Some("zh-CN")
            || ui.send.label().as_deref() != Some("发送")
        {
            return Err("Restart did not restore the shared Chinese language".into());
        }
        until(|| {
            ui.state
                .borrow()
                .queues
                .entries
                .get("codex:target")
                .is_some_and(|entry| entry.loaded)
        })
        .await?;
        if ui.state.borrow().queues.entries["codex:target"]
            .messages
            .first()
            .is_none_or(|message| message.text != "Send {count} 中文😀")
        {
            return Err("Restart lost the explicitly saved queued message".into());
        }
        return Ok(json!({"ok":true,"restart":true,"sharedLanguage":true,"queueRestored":true}));
    }
    if ui.send.label().as_deref() != Some("Send") {
        return Err("Fixture did not start in English".into());
    }
    let source = directory.join("project/language.md");
    let original = gio::spawn_blocking({
        let source = source.clone();
        move || std::fs::read_to_string(source)
    })
    .await
    .map_err(|_| "Language fixture worker stopped")?
    .map_err(|e| e.to_string())?;
    ui.reader.open_path(&source);
    ui.show_tool("document");
    let mode = || {
        find(ui.reader.widget.upcast_ref(), &|w| {
            w.clone()
                .downcast::<gtk::ToggleButton>()
                .is_ok_and(|b| b.label().as_deref() == Some("Edit"))
        })
        .and_downcast::<gtk::ToggleButton>()
    };
    until(|| mode().is_some_and(|b| b.is_sensitive())).await?;
    mode().unwrap().set_active(true);
    let editor = || {
        find(ui.reader.widget.upcast_ref(), &|w| {
            w.clone().downcast::<gtk::TextView>().is_ok_and(|v| {
                v.is_editable()
                    && v.is_mapped()
                    && v.buffer()
                        .text(&v.buffer().start_iter(), &v.buffer().end_iter(), true)
                        .starts_with("# Language document")
            })
        })
        .and_downcast::<gtk::TextView>()
    };
    until(|| editor().is_some()).await?;
    let editor = editor().unwrap();
    let buffer = editor.buffer();
    let saved_expected = format!("{original}\r\nUnapplied Send Settings 中文😀\r\n");
    buffer.set_text(&saved_expected);
    // Edit already normalizes pasted CRLF to LF; compare language switching to
    // the actual prepared buffer, then check restored on-disk CRLF separately.
    let changed = buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), true)
        .to_string();
    buffer.select_range(&buffer.iter_at_offset(4), &buffer.iter_at_offset(14));
    let selection = buffer
        .selection_bounds()
        .map(|(a, b)| (a.offset(), b.offset()));
    let draft = "Unsent Send Settings 发送 中文😀 ";
    ui.composer.buffer().set_text(draft);
    until(|| {
        ui.state
            .borrow()
            .queues
            .entries
            .get("codex:target")
            .is_some_and(|entry| entry.loaded)
    })
    .await?;
    let queued = ui.state.borrow().queues.entries["codex:target"]
        .messages
        .first()
        .cloned()
        .ok_or("Seeded queue missing")?;
    let edit_queue = find(ui.queue.widget.upcast_ref(), &|widget| {
        widget.tooltip_text().as_deref() == Some("Edit queued message")
    })
    .and_downcast::<gtk::Button>()
    .ok_or("Queue edit control missing")?;
    edit_queue.emit_clicked();
    until(|| {
        ui.queue
            .editor
            .borrow()
            .as_ref()
            .is_some_and(|editor| editor.dialog.is_mapped())
    })
    .await?;
    let queue_editor = ui.queue.editor.borrow().clone().unwrap();
    let queue_text = find(queue_editor.dialog.upcast_ref(), &|widget| {
        widget
            .clone()
            .downcast::<gtk::TextView>()
            .is_ok_and(|text| text.is_editable())
    })
    .and_downcast::<gtk::TextView>()
    .ok_or("Queued message editor missing")?;
    queue_text.buffer().set_text("Send {count} 中文😀");
    // Equal UI words in an unmarked content label must remain literal.
    let literal = gtk::Label::new(Some("Send"));
    let superseded = gtk::Label::new(Some("Send"));
    crate::i18n_ui::bind(&superseded, "label", "Send");
    superseded.set_text("External Settings value");
    let equal_literal = gtk::Label::new(None);
    crate::i18n_ui::bind(&equal_literal, "label", "Send");
    crate::i18n_ui::literal(&equal_literal, "label", "Send");
    ui.open_favorites();
    let favorites = ui
        .favorites
        .borrow()
        .clone()
        .ok_or("Language Favorites did not open")?;
    let favorite_probe = favorites.smoke_language_prepare(directory).await?;
    ui.comment_panel(None);
    let comments = ui
        .annotations
        .borrow()
        .clone()
        .ok_or("Language Comments did not open")?;
    let comment_probe = comments.smoke_language_prepare().await?;
    ui.open_settings();
    let settings = ui
        .settings
        .borrow()
        .clone()
        .ok_or("Settings did not open")?;
    let controls = settings.smoke_language();
    let appearance = settings.smoke_appearance();
    until(|| appearance.may_close() && appearance.dialog.is_mapped()).await?;
    controls.smoke_select(1);
    if settings.may_close() {
        return Err("Unapplied language choice can be discarded without a close guard".into());
    }
    controls.smoke_apply();
    until(|| !controls.saving() && ui.language.smoke_language().as_deref() == Some("zh-CN"))
        .await?;
    if ui.send.label().as_deref() != Some("发送") || settings.dialog.title() != "设置" {
        return Err("Language did not update live native actions/settings".into());
    }
    favorites.smoke_language_check(&favorite_probe, true)?;
    comments.smoke_language_check(&comment_probe, true)?;
    if !ui.reader.smoke_status_text().contains("行 ·")
        || !ui.reader.smoke_status_text().ends_with(" · 未保存")
    {
        return Err("Dirty Markdown reader status did not follow the live language".into());
    }
    if literal.text() != "Send"
        || superseded.text() != "External Settings value"
        || equal_literal.text() != "Send"
    {
        return Err("Language change translated unmarked or superseded data".into());
    }
    if queue_editor.dialog.title() != "编辑排队消息"
        || find(ui.queue.widget.upcast_ref(), &|widget| {
            widget
                .clone()
                .downcast::<gtk::Label>()
                .is_ok_and(|label| label.text() == "待发送消息 · 1/3")
        })
        .is_none()
    {
        return Err("Live queue translation failed to update its editor or counter".into());
    }
    if find(ui.queue.widget.upcast_ref(), &|widget| {
        widget
            .clone()
            .downcast::<gtk::Label>()
            .is_ok_and(|label| label.text() == "Pause")
    })
    .is_none()
    {
        return Err("Language switching translated queued-message content as a UI label".into());
    }
    let has_row = |dialog: &adw::PreferencesDialog, title: &str| {
        find(dialog.upcast_ref(), &|w| {
            w.clone()
                .downcast::<adw::PreferencesRow>()
                .is_ok_and(|row| row.title() == title)
        })
        .is_some()
    };
    if appearance.dialog.title() != "字体与阅读布局" || !has_row(&appearance.dialog, "阅读宽度")
    {
        return Err("Existing appearance controls did not follow the language change".into());
    }
    let markdown = find(appearance.dialog.upcast_ref(), &|w| {
        w.clone()
            .downcast::<adw::ComboRow>()
            .is_ok_and(|row| row.title() == "Markdown 布局")
    })
    .and_downcast::<adw::ComboRow>()
    .ok_or("Markdown layout choice missing")?;
    if markdown
        .selected_item()
        .and_downcast::<gtk::StringObject>()
        .is_none_or(|item| item.string() != "Technical")
        || find(markdown.upcast_ref(), &|w| {
            w.clone()
                .downcast::<gtk::Label>()
                .is_ok_and(|label| label.text() == "技术")
        })
        .is_none()
    {
        return Err(
            "Translated Markdown layout changed its payload or lost its contextual label".into(),
        );
    }
    glib::timeout_future(Duration::from_millis(350)).await;
    crate::window_capture::capture(&ui.window, &directory.join("language-appearance.png")).await?;
    appearance.dialog.close();
    until(|| !appearance.dialog.is_mapped()).await?;
    let browser = settings.smoke_browser();
    until(|| browser.may_close() && browser.dialog.is_mapped()).await?;
    if browser.dialog.title() != "浏览器导航" || !has_row(&browser.dialog, "允许本机地址")
    {
        return Err("New browser permission controls did not use the selected language".into());
    }
    browser.dialog.close();
    until(|| !browser.dialog.is_mapped()).await?;
    let visibility = settings.smoke_visibility();
    until(|| visibility.may_close() && visibility.dialog.is_mapped()).await?;
    if visibility.dialog.title() != "会话可见性" {
        return Err("Session visibility controls did not use the selected language".into());
    }
    visibility.dialog.close();
    until(|| !visibility.dialog.is_mapped()).await?;
    let shared = settings.smoke_shared();
    until(|| shared.may_close() && shared.dialog.is_mapped()).await?;
    if shared.dialog.title() != "共享目录" || !has_row(&shared.dialog, "文件夹绝对路径")
    {
        return Err("Shared directory controls did not use the selected language".into());
    }
    shared.dialog.close();
    until(|| !shared.dialog.is_mapped()).await?;
    let notifications = settings.smoke_notifications();
    if notifications.group.title() != "通知" || notifications.row.title() != "桌面通知" {
        return Err("Notification controls did not follow the selected language".into());
    }
    settings.dialog.set_visible_page_name("appearance");
    until(|| settings.dialog.is_mapped()).await?;
    glib::timeout_future(Duration::from_millis(350)).await;
    crate::window_capture::capture(&ui.window, &directory.join("language-chinese.png")).await?;
    let path = directory.join("config/codex-thread-studio/settings.json");
    gio::spawn_blocking(move || {
        let mut raw: Value =
            serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        raw["language"] = json!("en-US");
        raw["concurrentClient"] = json!({"keep":true});
        std::fs::write(path, serde_json::to_vec(&raw).unwrap()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "Language fixture worker stopped")??;
    controls.smoke_select(0);
    comments.smoke_language_prompt(&comment_probe).await?;
    controls.smoke_apply();
    until(|| !controls.saving() && controls.smoke_status().contains("其他客户端")).await?;
    if controls.smoke_selected() != 0
        || !controls.dirty()
        || ui.send.label().as_deref() != Some("发送")
    {
        return Err(
            "Conflicting language save replaced the local choice or visible language".into(),
        );
    }
    controls.smoke_reload();
    until(|| ui.language.smoke_language().as_deref() == Some("en-US") && !controls.dirty()).await?;
    if ui.send.label().as_deref() != Some("Send") {
        return Err("Language reload did not restore English".into());
    }
    favorites.smoke_language_check(&favorite_probe, false)?;
    comments.smoke_language_check(&comment_probe, false)?;
    if queue_editor.dialog.title() != "Edit queued message"
        || queue_text.buffer().text(
            &queue_text.buffer().start_iter(),
            &queue_text.buffer().end_iter(),
            true,
        ) != "Send {count} 中文😀"
        || ui.state.borrow().queues.entries["codex:target"].messages[0].text != "Pause"
    {
        return Err(
            "Language switching changed or implicitly saved the queued-message draft".into(),
        );
    }
    if appearance.dialog.title() != "Fonts and reading layout"
        || browser.dialog.title() != "Browser navigation"
        || visibility.dialog.title() != "Session visibility"
        || shared.dialog.title() != "Shared folders"
        || notifications.row.title() != "Desktop notifications"
    {
        return Err("Existing settings bindings did not restore English".into());
    }
    controls.smoke_select(1);
    controls.smoke_apply();
    until(|| !controls.saving() && ui.language.smoke_language().as_deref() == Some("zh-CN"))
        .await?;
    if buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), true)
        .as_str()
        != changed
        || buffer
            .selection_bounds()
            .map(|(a, b)| (a.offset(), b.offset()))
            != selection
        || ui.composer_text() != draft
    {
        return Err(format!(
            "Language switching changed edit={}, selection={:?} (expected {:?}), draft={}",
            buffer
                .text(&buffer.start_iter(), &buffer.end_iter(), true)
                .as_str()
                != changed,
            buffer
                .selection_bounds()
                .map(|(a, b)| (a.offset(), b.offset())),
            selection,
            ui.composer_text() != draft
        ));
    }
    let disk = gio::spawn_blocking({
        let source = source.clone();
        move || std::fs::read_to_string(source)
    })
    .await
    .map_err(|_| "Language fixture worker stopped")?
    .map_err(|e| e.to_string())?;
    if disk != original {
        return Err("Language switching implicitly saved the document".into());
    }
    settings.dialog.close();
    until(|| ui.settings.borrow().is_none()).await?;
    comments
        .smoke_language_finish(&comment_probe, directory)
        .await?;
    until(|| ui.annotations.borrow().is_none()).await?;
    favorites
        .smoke_language_finish(&favorite_probe, directory, &ui.window)
        .await?;
    until(|| ui.favorites.borrow().is_none()).await?;
    let queue_save = find(queue_editor.dialog.upcast_ref(), &|widget| {
        widget
            .clone()
            .downcast::<gtk::Button>()
            .is_ok_and(|button| button.label().as_deref() == Some("保存"))
    })
    .and_downcast::<gtk::Button>()
    .ok_or("Translated queue Save button missing")?;
    queue_save.emit_clicked();
    until(|| ui.queue.editor.borrow().is_none()).await?;
    let mut expected = (*queued).clone();
    expected.text = "Send {count} 中文😀".into();
    let saved = ui.state.borrow().queues.entries["codex:target"].messages[0].clone();
    if saved.id != expected.id
        || saved.text != expected.text
        || saved.input != expected.input
        || saved.created_at != expected.created_at
    {
        return Err("Localized queue save changed message identity, text or references".into());
    }
    let save = find(ui.reader.widget.upcast_ref(), &|w| {
        w.tooltip_text().as_deref() == Some(&crate::language::tr("Save current page (Ctrl+S)"))
    })
    .and_downcast::<gtk::Button>()
    .ok_or("Save action missing after language switch")?;
    save.emit_clicked();
    let at = std::time::Instant::now();
    loop {
        let disk = gio::spawn_blocking({
            let source = source.clone();
            move || std::fs::read_to_string(source)
        })
        .await
        .map_err(|_| "Language fixture worker stopped")?
        .map_err(|e| e.to_string())?;
        if disk == saved_expected {
            break;
        }
        if at.elapsed() > Duration::from_secs(10) {
            return Err("Explicit edit save failed after switching language".into());
        }
        glib::timeout_future(Duration::from_millis(30)).await;
    }
    Ok(
        json!({"ok":true,"liveLanguage":true,"settingsPanels":true,"queueLocaleAndSave":true,"favoritesLanguage":true,"favoriteDraftAndPromptPreserved":true,"favoriteExplicitSave":true,"commentsLanguage":true,"commentLocationParts":true,"commentPromptAndDraftPreserved":true,"conflictChoiceRetained":true,"sharedReload":true,"editBufferPreserved":true,"selectionPreserved":true,"draftPreserved":true,"noImplicitSave":true,"unmarkedContentLiteral":true,"supersededBindingIgnored":true,"explicitSave":true}),
    )
}
