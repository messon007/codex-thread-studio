//! Real mounted chat text/code/table actions; owned fake providers only.
use super::*;
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let at = std::time::Instant::now();
    while !test() {
        if at.elapsed() > Duration::from_secs(15) {
            return Err(format!("Chat comment acceptance timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
fn find(widget: &gtk::Widget, needle: &str) -> Option<gtk::Widget> {
    if widget.is_mapped() {
        let text = if let Some(label) = widget
            .downcast_ref::<gtk::Label>()
            .filter(|l| l.is_selectable())
        {
            Some(label.text().to_string())
        } else {
            widget
                .downcast_ref::<gtk::TextView>()
                .filter(|v| !v.is_editable())
                .map(|v| {
                    let b = v.buffer();
                    b.text(&b.start_iter(), &b.end_iter(), false).to_string()
                })
        };
        if text.is_some_and(|s| s.contains(needle)) {
            return Some(widget.clone());
        }
    }
    let mut child = widget.first_child();
    while let Some(w) = child {
        child = w.next_sibling();
        if let Some(w) = find(&w, needle) {
            return Some(w);
        }
    }
    None
}
fn select(widget: &gtk::Widget, needle: &str) -> Result<(), String> {
    if let Some(label) = widget.downcast_ref::<gtk::Label>() {
        let text = label.text();
        let start = text.find(needle).ok_or("Label quote missing")?;
        let a = text[..start].chars().count() as i32;
        label.select_region(a, a + needle.chars().count() as i32);
    } else if let Some(view) = widget.downcast_ref::<gtk::TextView>() {
        let b = view.buffer();
        let text = b.text(&b.start_iter(), &b.end_iter(), false);
        let start = text.find(needle).ok_or("Buffer quote missing")?;
        let a = text[..start].chars().count() as i32;
        b.select_range(
            &b.iter_at_offset(a),
            &b.iter_at_offset(a + needle.chars().count() as i32),
        );
    } else {
        return Err("Not a native text selection widget".into());
    }
    Ok(())
}
fn menu(widget: &gtk::Widget, action: &str) -> Option<String> {
    fn scan(model: &gio::MenuModel, action: &str) -> Option<String> {
        for i in 0..model.n_items() {
            if model
                .item_attribute_value(i, "action", None)
                .is_some_and(|v| v.str() == Some(action))
            {
                return model
                    .item_attribute_value(i, "label", None)
                    .and_then(|v| v.str().map(str::to_owned));
            }
            for link in ["section", "submenu"] {
                if let Some(child) = model.item_link(i, link) {
                    if let Some(s) = scan(&child, action) {
                        return Some(s);
                    }
                }
            }
        }
        None
    }
    let model = widget
        .downcast_ref::<gtk::Label>()
        .and_then(|l| l.extra_menu())
        .or_else(|| {
            widget
                .downcast_ref::<gtk::TextView>()
                .and_then(|v| v.extra_menu())
        })?;
    scan(&model, action)
}
async fn leaf(ui: &Rc<Ui>, id: &str, needle: &str) -> Result<gtk::Widget, String> {
    let locate = || {
        (0..ui.transcript.n_items()).find(|index| {
            let value = ui
                .transcript
                .item(*index)
                .and_downcast::<glib::BoxedAnyObject>()
                .unwrap();
            let row = value.borrow::<crate::chat_ui::Row>();
            row.item.id == id
                && match row.part {
                    crate::chat_ui::Part::Plain => row.item.body.contains(needle),
                    crate::chat_ui::Part::Block(index) => {
                        row.item.rich.as_ref().is_some_and(|rich| {
                            match &rich.rows[index].block.kind {
                                studio_markdown_native::BlockKind::Paragraph(text)
                                | studio_markdown_native::BlockKind::Heading {
                                    content: text,
                                    ..
                                } => text.plain.contains(needle),
                                studio_markdown_native::BlockKind::Code { text, .. } => {
                                    text.contains(needle)
                                }
                                studio_markdown_native::BlockKind::Table { .. } => {
                                    needle.starts_with("Cell ")
                                }
                                _ => false,
                            }
                        })
                    }
                    _ => false,
                }
        })
    };
    until("reply blocks rendered", || locate().is_some()).await?;
    let row = locate().ok_or_else(|| format!("Reply block missing: {needle}"))?;
    ui.transcript_view
        .scroll_to(row, gtk::ListScrollFlags::NONE, None);
    until(needle, || {
        find(ui.transcript_view.upcast_ref(), needle).is_some()
    })
    .await?;
    find(ui.transcript_view.upcast_ref(), needle).ok_or("Mapped text vanished".into())
}
async fn capture(
    ui: &Rc<Ui>,
    widget: &gtk::Widget,
    item: &Arc<Item>,
    quote: &str,
) -> Result<Value, String> {
    select(widget, quote)?;
    widget
        .activate_action("chat-comment.capture", None)
        .map_err(|e| e.to_string())?;
    until("captured library", || ui.annotations.borrow().is_some()).await?;
    let panel = ui.annotations.borrow().clone().unwrap();
    let expected =
        crate::annotations::chat_capture(item, "codex:native-smoke-thread", Some(quote.into()))?;
    let record = panel.smoke_save_source_capture(&expected).await?;
    if let Some(directory) = std::env::var_os("STUDIO_GNOME_SMOKE_DIR") {
        smoke::capture_dialog(
            &panel.dialog,
            &std::path::PathBuf::from(directory).join("chat-comment-dialog.png"),
        );
    }
    panel.dialog.close();
    until("saved library closed", || ui.annotations.borrow().is_none()).await?;
    Ok(record)
}
pub(super) async fn run(
    ui: &Rc<Ui>,
    root: &std::path::Path,
    screenshot: impl Fn(&str),
) -> Result<Value, String> {
    until("source history", || {
        ui.state.borrow().ready
            && !ui.state.borrow().loading
            && ui
                .state
                .borrow()
                .items
                .iter()
                .any(|i| i.id == "history-message")
    })
    .await?;
    ui.composer.buffer().set_text("Unsent chat draft 中文😀");
    let original = ui
        .state
        .borrow()
        .items
        .iter()
        .find(|i| i.id == "history-message")
        .cloned()
        .unwrap();
    if std::env::var("STUDIO_GNOME_SMOKE_CHAT_COMMENTS")
        .ok()
        .as_deref()
        == Some("restore")
    {
        ui.comment_panel(None);
        let panel = ui.annotations.borrow().clone().unwrap();
        until("restored records", || panel.smoke_chat_records().is_some()).await?;
        let records = panel.smoke_chat_records().unwrap();
        if records.len() != 5 || records.iter().any(|r| r["source"]["provider"] != "chat") {
            return Err(format!("Chat records not restored: {records:?}"));
        }
        panel.smoke_chat_insert().await?;
        let first = ui
            .composer
            .buffer()
            .text(
                &ui.composer.buffer().start_iter(),
                &ui.composer.buffer().end_iter(),
                false,
            )
            .to_string();
        panel.smoke_chat_insert().await?;
        let b = ui.composer.buffer();
        let second = b.text(&b.start_iter(), &b.end_iter(), false);
        if first != second
            || !first.starts_with("Unsent chat draft 中文😀")
            || !first.contains("session opencode:oc-target / turn actual-turn / item actual-reply")
        {
            return Err("Chat insertion lost draft, anchors or idempotence".into());
        }
        screenshot("chat-comments-restored.png");
        panel.dialog.close();
        until("restore library closed", || {
            ui.annotations.borrow().is_none()
        })
        .await?;
        return Ok(
            json!({"ok":true,"restored":true,"records":records,"draftRetained":true,"idempotentInsert":true}),
        );
    }
    let quote = "Literal Save bold 中文😀";
    let widget = leaf(ui, "history-message", quote).await?;
    select(&widget, quote)?;
    for language in ["zh-CN", "en-US"] {
        crate::language::set(language, "en-US")?;
        crate::i18n_ui::apply();
        for (action, key) in [
            ("chat-comment.capture", "Comment on selected text"),
            ("favorite.save-selection", "Favorite selected text"),
            ("translation.translate", "Translate selected text"),
        ] {
            if menu(&widget, action).as_deref() != Some(crate::language::tr(key).as_str()) {
                return Err(format!("Live context-menu language failed: {action}"));
            }
        }
        if crate::content_widgets::selection(&widget, 16384).as_deref() != Some(quote) {
            return Err("Language changed selected model text".into());
        }
    }
    // Equal history refresh preserves this mounted leaf but replaces its Item
    // Arc. The menu must use the row's current handle, not the older capture.
    let current = Arc::new((*original).clone());
    {
        let mut state = ui.state.borrow_mut();
        let index = state
            .items
            .iter()
            .position(|i| Arc::ptr_eq(i, &original))
            .unwrap();
        state.items[index] = current.clone();
    }
    ui.render_items();
    if !widget.is_mapped() {
        return Err("Equal refresh remounted the selected leaf".into());
    }
    let mut records = vec![capture(ui, &widget, &current, quote).await?];
    let code = "fn literal() { println!(\"中文😀\"); }";
    let widget = leaf(ui, "history-message", code).await?;
    records.push(capture(ui, &widget, &current, code).await?);
    let first_cell = leaf(ui, "history-message", "Cell 0 中文😀").await?;
    select(&first_cell, "Cell 0 中文😀")?;
    fn next(widget: &gtk::Widget) -> Option<gtk::Button> {
        if let Some(b) = widget
            .downcast_ref::<gtk::Button>()
            .filter(|b| b.is_mapped() && b.tooltip_text().as_deref() == Some("Next table page"))
        {
            return Some(b.clone());
        }
        let mut child = widget.first_child();
        while let Some(w) = child {
            child = w.next_sibling();
            if let Some(b) = next(&w) {
                return Some(b);
            }
        }
        None
    }
    next(ui.transcript_view.upcast_ref())
        .ok_or("Table page control missing")?
        .emit_clicked();
    until("recycled first table page", || !first_cell.is_mapped()).await?;
    first_cell
        .activate_action("chat-comment.capture", None)
        .map_err(|e| e.to_string())?;
    if ui.annotations.borrow().is_some() {
        return Err("Recycled table selection captured a comment".into());
    }
    let widget = leaf(ui, "history-message", "Cell 55 中文😀").await?;
    records.push(capture(ui, &widget, &current, "Cell 55 中文😀").await?);
    let oversized = leaf(ui, "history-message", "large-chat-comment-boundary").await?;
    let view = oversized
        .downcast_ref::<gtk::TextView>()
        .ok_or("Oversized code is not a native text view")?;
    let buffer = view.buffer();
    buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
    oversized
        .activate_action("chat-comment.capture", None)
        .map_err(|e| e.to_string())?;
    if ui.annotations.borrow().is_some() {
        return Err("Oversized GTK selection created a comment".into());
    }
    ui.history_pages.open();
    until("history window", || {
        ui.history_pages.active() && !ui.history_pages.smoke_busy()
    })
    .await?;
    if !ui
        .history_pages
        .items()
        .unwrap_or_default()
        .iter()
        .any(|i| i.id == "native-old-response")
    {
        ui.history_pages.smoke_older();
        until("older history window", || {
            !ui.history_pages.smoke_busy()
                && ui
                    .history_pages
                    .items()
                    .is_some_and(|items| items.iter().any(|i| i.id == "native-old-response"))
        })
        .await?;
    }
    let older = ui
        .history_pages
        .items()
        .unwrap_or_default()
        .into_iter()
        .find(|i| i.id == "native-old-response")
        .ok_or("Older reply missing")?;
    let older_quote = "native-old-marker is available from paged history.";
    let widget = leaf(ui, "native-old-response", older_quote).await?;
    records.push(capture(ui, &widget, &older, older_quote).await?);
    ui.history_pages.smoke_live();
    until("live tail restored", || !ui.history_pages.active()).await?;
    ui.render_items();
    // Router display wrappers retain their actual target message identity while
    // the comment batch continues to belong to the selected conversation.
    let mut routed = (*current).clone();
    routed.origin = Some(Arc::new(crate::model::Origin {
        backend: "opencode".into(),
        thread: "oc-target".into(),
        title: "Target".into(),
        root: root.join("project").to_string_lossy().into(),
        turn: "actual-turn".into(),
        item: "actual-reply".into(),
        question: Arc::new("Original question".into()),
        excerpt: true,
    }));
    let routed = Arc::new(routed);
    {
        let mut state = ui.state.borrow_mut();
        let index = state
            .items
            .iter()
            .position(|i| Arc::ptr_eq(i, &current))
            .unwrap();
        state.items[index] = routed.clone();
    }
    ui.render_items();
    until("routed reply rebind", || !widget.is_mapped()).await?;
    let widget = leaf(ui, "history-message", quote).await?;
    records.push(capture(ui, &widget, &routed, quote).await?);
    screenshot("chat-comments-captured.png");
    ui.select(Thread {
        id: "native-cache-thread".into(),
        parent: None,
        title: "Sibling".into(),
        cwd: root.join("project").to_string_lossy().into(),
        pinned: false,
        activity: String::new(),
    });
    until("sibling loaded", || {
        !ui.state.borrow().loading
            && ui
                .state
                .borrow()
                .selected
                .as_ref()
                .is_some_and(|t| t.id == "native-cache-thread")
    })
    .await?;
    ui.comment_reply(routed, Some(quote.into()));
    if ui.annotations.borrow().is_some() {
        return Err("Stale reply wrote to a different session".into());
    }
    Ok(
        json!({"ok":true,"records":records,"literalSelection":true,"codeSelection":true,"pagedTableSelection":true,"historicalSelection":true,"oversizedSelectionRejected":true,"liveMenuLanguage":true,"stableRefreshAction":true,"recycledCellRejected":true,"routedIdentity":true,"staleSessionRejected":true}),
    )
}
