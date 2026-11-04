//! Private Wayland acceptance through real gateway reads and fake provider I/O.
use super::*;
use std::cell::Cell;
pub(super) struct Probe {
    timer: Option<glib::SourceId>,
    ticks: Rc<Cell<u64>>,
    gap: Rc<Cell<u64>>,
}
impl Probe {
    pub(super) fn new() -> Self {
        let ticks = Rc::new(Cell::new(0));
        let gap = Rc::new(Cell::new(0));
        let t = ticks.clone();
        let g = gap.clone();
        let last = Cell::new(std::time::Instant::now());
        let timer = glib::timeout_add_local(Duration::from_millis(10), move || {
            let now = std::time::Instant::now();
            g.set(
                g.get()
                    .max(now.duration_since(last.replace(now)).as_millis() as u64),
            );
            t.set(t.get() + 1);
            glib::ControlFlow::Continue
        });
        Self {
            timer: Some(timer),
            ticks,
            gap,
        }
    }
    pub(super) fn stop(&mut self) -> Value {
        if let Some(timer) = self.timer.take() {
            timer.remove();
        }
        json!({"ticks":self.ticks.get(),"maximumGapMs":self.gap.get()})
    }
}
impl Drop for Probe {
    fn drop(&mut self) {
        if let Some(timer) = self.timer.take() {
            timer.remove();
        }
    }
}
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !test() {
        if start.elapsed() > Duration::from_secs(12) {
            return Err(format!("History paging acceptance timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(25)).await;
    }
    Ok(())
}
async fn flag(root: &std::path::Path, name: &str) -> Result<(), String> {
    let path = root.join(name);
    gio::spawn_blocking(move || std::fs::write(path, b"private fixture").map_err(|e| e.to_string()))
        .await
        .map_err(|_| "Fixture worker stopped")?
}
async fn remove(root: &std::path::Path, name: &str) -> Result<(), String> {
    let path = root.join(name);
    gio::spawn_blocking(move || std::fs::remove_file(path).map_err(|e| e.to_string()))
        .await
        .map_err(|_| "Fixture worker stopped")?
}
async fn idle(ui: &Rc<Ui>) -> Result<(), String> {
    until("page response", || !ui.history_pages.smoke_busy()).await?;
    if !ui.history_pages.active() {
        return Err(format!(
            "History did not open: {}",
            ui.history_pages.smoke_summary()
        ));
    }
    Ok(())
}
fn ids(ui: &Ui) -> Vec<String> {
    ui.history_pages
        .items()
        .unwrap_or_default()
        .iter()
        .map(|i| i.id.clone())
        .collect()
}
fn transcript_contains(ui: &Ui, id: &str) -> bool {
    (0..ui.transcript.n_items()).any(|i| {
        ui.transcript
            .item(i)
            .and_downcast::<glib::BoxedAnyObject>()
            .is_some_and(|object| object.borrow::<crate::chat_ui::Row>().item.id == id)
    })
}
fn mapped_text(widget: &gtk::Widget) -> Option<gtk::Label> {
    if let Ok(view) = widget.clone().downcast::<gtk::Label>() {
        if view.is_mapped() && view.is_selectable() && view.text().chars().count() > 8 {
            return Some(view);
        }
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if let Some(view) = mapped_text(&widget) {
            return Some(view);
        }
    }
    None
}
async fn language_roundtrip(ui: &Rc<Ui>, root: &std::path::Path, case: &str) -> Result<(), String> {
    until("history reconciliation before language change", || {
        !ui.state.borrow().render_dirty && !ui.history_pages.needs_render()
    })
    .await?;
    let adjustment = ui.transcript_scroll.vadjustment();
    let at = std::time::Instant::now();
    let mut previous = None;
    let mut stable = 0;
    while stable < 4 {
        crate::window_capture::next_frame(&ui.window).await?;
        let geometry = (
            adjustment.value(),
            adjustment.upper(),
            adjustment.page_size(),
        );
        if previous == Some(geometry) {
            stable += 1;
        } else {
            stable = 0;
        }
        previous = Some(geometry);
        if at.elapsed() > Duration::from_secs(5) {
            return Err("History layout did not settle before locale check".into());
        }
    }
    until("mapped history selection for language change", || {
        mapped_text(ui.transcript_view.upcast_ref()).is_some()
    })
    .await?;
    let view = mapped_text(ui.transcript_view.upcast_ref()).unwrap();
    view.select_region(1, 7);
    let text = view.text().to_string();
    let original = ui.history_pages.items().ok_or("History page missing")?;
    let live = ui.state.borrow().items.clone();
    let draft = ui.composer_text();
    let summary = ui.history_pages.smoke_summary();
    let offset = adjustment.value();
    let bottom = adjustment.upper() - adjustment.page_size() - offset;
    let anchored_bottom = bottom.abs() <= 2.0;
    let original_geometry = json!({"value":offset,"upper":adjustment.upper(),"viewport":adjustment.page_size(),"toolbar":ui.history_pages.widget.height(),"bottomAnchored":anchored_bottom});
    for chinese in [true, false] {
        crate::language::set(if chinese { "zh-CN" } else { "en-US" }, "en-US")?;
        crate::i18n_ui::apply();
        if let Some(changed) = ui.language.on_changed.borrow().as_ref() {
            changed();
        }
        crate::window_capture::next_frame(&ui.window).await?;
        let mut buttons = Vec::new();
        let mut child = ui.history_pages.widget.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            if let Ok(button) = widget.downcast::<gtk::Button>() {
                buttons.push((
                    button.label().unwrap_or_default().to_string(),
                    button.tooltip_text().unwrap_or_default().to_string(),
                ));
            }
        }
        let expected = if chinese {
            ["更早", "较新", "实时会话"]
        } else {
            ["Older", "Newer", "Live"]
        };
        if buttons
            .iter()
            .map(|(label, _)| label.as_str())
            .collect::<Vec<_>>()
            != expected
        {
            return Err(format!(
                "History controls did not follow locale: {buttons:?}"
            ));
        }
        if chinese {
            if buttons[0].1 != "读取上一页历史消息"
                || !ui.history_pages.smoke_summary().starts_with("历史 · 第 ")
            {
                return Err("History tips or page/count summary remained English".into());
            }
            if ui.history_pages.smoke_mode() == Some(crate::conversation_pages::Mode::Legacy)
                && !ui.history_pages.smoke_summary().contains("兼容模式读取")
            {
                return Err("Legacy history detail remained English".into());
            }
            crate::window_capture::capture(
                &ui.window,
                &root.join(format!("history-language-{case}.png")),
            )
            .await?;
            let geometry = json!({"before":original_geometry,"after":{"value":adjustment.value(),"upper":adjustment.upper(),"viewport":adjustment.page_size(),"toolbar":ui.history_pages.widget.height()}});
            std::fs::write(
                root.join(format!("history-language-{case}.json")),
                serde_json::to_vec_pretty(&geometry).unwrap(),
            )
            .map_err(|e| e.to_string())?;
        } else if ui.history_pages.smoke_summary() != summary {
            return Err("History page summary did not survive locale roundtrip".into());
        }
        let now = ui
            .history_pages
            .items()
            .ok_or("Language change closed history")?;
        let current_live = ui.state.borrow().items.clone();
        // Native font metrics may change the toolbar/viewport height. Preserve
        // the bottom anchor when reading at the end; otherwise retain the top
        // content offset. An unchanged raw scroll value would reject GTK's
        // correct bottom anchoring after that real viewport resize.
        let reading_preserved = if anchored_bottom {
            (adjustment.upper() - adjustment.page_size() - adjustment.value() - bottom).abs() <= 2.0
        } else {
            (adjustment.value() - offset).abs() <= 2.0
        };
        if now.len() != original.len()
            || !now.iter().zip(&original).all(|(a, b)| Arc::ptr_eq(a, b))
            || current_live.len() != live.len()
            || !current_live
                .iter()
                .zip(&live)
                .all(|(a, b)| Arc::ptr_eq(a, b))
            || ui.composer_text() != draft
            || !view.is_mapped()
            || view.text().as_str() != text
            || view.selection_bounds() != Some((1, 7))
            || !reading_preserved
        {
            return Err(format!("Locale change changed history state: case={case} chinese={chinese} historyPointers={} livePointers={} draft={} mapped={} text={} selection={:?} offset={offset}->{}",
                now.len() == original.len() && now.iter().zip(&original).all(|(a,b)|Arc::ptr_eq(a,b)),
                current_live.len() == live.len() && current_live.iter().zip(&live).all(|(a,b)|Arc::ptr_eq(a,b)),
                ui.composer_text() == draft, view.is_mapped(), view.text().as_str() == text,
                view.selection_bounds(), ui.transcript_scroll.vadjustment().value()));
        }
    }
    Ok(())
}
pub async fn run(
    ui: &Rc<Ui>,
    root: &std::path::Path,
    capture: impl Fn(&str),
) -> Result<Value, String> {
    until("initial history", || {
        let s = ui.state.borrow();
        s.ready
            && !s.loading
            && s.selected.as_ref().is_some_and(|t| t.id == "target")
            && s.items.iter().any(|i| i.id == "h649")
            && s.options_loaded
    })
    .await?;
    ui.composer
        .buffer()
        .set_text("Unsent history draft 中文😀 ");
    let live = ui.state.borrow().items.clone();
    if live.len() != crate::conversation_pages::PAGE_ITEMS {
        return Err("Live tail was not bounded before history opened".into());
    }
    let mut probe = Probe::new();
    let modern = root.join("history-use-items").exists();
    let opening = std::time::Instant::now();
    ui.open_history_pages();
    idle(ui).await?;
    let mapped = until("mapped first history body", || {
        ui.chat_visibility
            .visible_ends(ui.transcript_scroll.upcast_ref())
            .contains(&("h649".to_owned(), "huge-turn".to_owned()))
    })
    .await;
    if let Err(error) = mapped {
        let visible = ui
            .chat_visibility
            .visible_ends(ui.transcript_scroll.upcast_ref());
        let a = ui.transcript_scroll.vadjustment();
        let details=format!("{error}; window mapped={} {}x{}, list {}x{}, scroll={} upper={} page={}, visible={visible:?}",ui.window.is_mapped(),ui.window.width(),ui.window.height(),ui.transcript_view.width(),ui.transcript_view.height(),a.value(),a.upper(),a.page_size());
        capture("history-first-not-mapped.png");
        return Err(details);
    }
    let first_open_ms = opening.elapsed().as_millis();
    if ui.history_pages.smoke_mode()
        != Some(if modern {
            crate::conversation_pages::Mode::Items
        } else {
            crate::conversation_pages::Mode::Paged
        })
    {
        return Err("History did not choose its preferred or compatibility paging mode".into());
    }
    if ids(ui).first().map(String::as_str) != Some("h530")
        || ids(ui).last().map(String::as_str) != Some("h649")
    {
        return Err("Initial history slice is incorrect".into());
    }
    if ui.state.borrow().items.len() != crate::conversation_pages::PAGE_ITEMS
        || !std::sync::Arc::ptr_eq(&live[0], &ui.state.borrow().items[0])
    {
        return Err("Opening history copied or replaced the live tail".into());
    }
    language_roundtrip(ui, root, "codex").await?;
    ui.history_pages.smoke_older();
    idle(ui).await?;
    if ids(ui).first().map(String::as_str) != Some("h410") {
        return Err("Older did not page within a large turn".into());
    }
    // Each click uses a stable source anchor, not an ever-growing GTK model.
    for first in ["h290", "h170", "h50"] {
        ui.history_pages.smoke_older();
        idle(ui).await?;
        if ids(ui).first().map(String::as_str) != Some(first) {
            return Err(format!("History skipped {first}"));
        }
    }
    ui.history_pages.smoke_older();
    idle(ui).await?;
    if !modern {
        if ids(ui).first().map(String::as_str) != Some("h0") {
            return Err("Turn page omitted its earliest slice".into());
        }
        ui.history_pages.smoke_older();
        idle(ui).await?;
    }
    if ids(ui).first().map(String::as_str) != Some("oldest-table") {
        return Err(format!(
            "Provider cursor did not retrieve older turns: {:?}",
            ids(ui)
        ));
    }
    until("historical table rendering", || {
        transcript_contains(ui, "oldest-table")
    })
    .await?;
    let responsiveness = probe.stop();
    capture("history-oldest.png");
    ui.finder
        .smoke_history_page("Older 中文😀 ", "oldest-table")
        .await?;
    ui.finder.close();
    ui.finder.clear();
    let at_oldest = ids(ui);
    let source = ui.history_pages.items().unwrap()[0].clone();
    if source.turn_id != "oldest-turn" || source.rich.as_ref().is_none_or(|c| c.rows.is_empty()) {
        return Err("Historical Markdown/source anchors were lost".into());
    }
    let adjustment = ui.transcript_scroll.vadjustment();
    adjustment.set_value(0.0);
    let before = adjustment.value();
    flag(root, "history-live-event").await?;
    until("offscreen live event", || {
        ui.state
            .borrow()
            .items
            .iter()
            .any(|i| i.id == "live-answer")
            && ui.state.borrow().active_turn.is_none()
    })
    .await?;
    glib::timeout_future(Duration::from_millis(180)).await;
    if ids(ui) != at_oldest
        || !transcript_contains(ui, "oldest-table")
        || transcript_contains(ui, "live-answer")
        || (adjustment.value() - before).abs() > 1.0
    {
        return Err("Live delivery replaced or scrolled historical content".into());
    }
    if ui.composer_text() != "Unsent history draft 中文😀 " {
        return Err("History navigation changed the draft".into());
    }
    ui.history_pages.smoke_newer();
    idle(ui).await?;
    if ids(ui).first().map(String::as_str) != Some(if modern { "h50" } else { "h0" }) {
        return Err("Newer did not restore its previous window".into());
    }
    ui.history_pages.smoke_live();
    until("return to live", || {
        !ui.history_pages.active() && transcript_contains(ui, "live-answer")
    })
    .await?;
    if ui.composer_text() != "Unsent history draft 中文😀 " {
        return Err("Live erased the draft".into());
    }
    // Failed reads leave the previous page in place and can be retried.
    ui.open_history_pages();
    idle(ui).await?;
    let before = ids(ui);
    flag(root, "history-anchor-removed").await?;
    ui.history_pages.smoke_older();
    idle(ui).await?;
    if ids(ui) != before || !ui.history_pages.smoke_summary().contains("retained") {
        return Err("Changed anchors silently shifted the page".into());
    }
    let error = ui.history_pages.smoke_summary();
    let diagnostic = error
        .strip_prefix("History read failed: ")
        .ok_or("Missing literal diagnostic wrapper")?;
    crate::language::set("zh-CN", "en-US")?;
    crate::i18n_ui::apply();
    if ui.history_pages.smoke_summary() != format!("读取历史失败：{diagnostic}") {
        return Err("History language change translated or replaced provider diagnostics".into());
    }
    crate::language::set("en-US", "en-US")?;
    crate::i18n_ui::apply();
    remove(root, "history-anchor-removed").await?;
    ui.history_pages.smoke_older();
    idle(ui).await?;
    if ids(ui) == before {
        return Err("History error was not retryable".into());
    }
    ui.history_pages.smoke_live();
    // A cancellation returns immediately, while a late provider reply is still pending.
    flag(root, "history-delay").await?;
    ui.open_history_pages();
    until("delayed request reached provider", || {
        !root.join("history-delay").exists()
    })
    .await?;
    crate::language::set("zh-CN", "en-US")?;
    crate::i18n_ui::apply();
    if ui.history_pages.smoke_summary() != "正在加载历史… 实时消息继续接收。" {
        return Err("Pending history status did not follow language change".into());
    }
    ui.history_pages.smoke_live();
    crate::language::set("en-US", "en-US")?;
    crate::i18n_ui::apply();
    glib::timeout_future(Duration::from_millis(600)).await;
    if ui.history_pages.active() || !transcript_contains(ui, "live-answer") {
        return Err("Late history response reopened a closed page".into());
    }
    // The legacy read uses the same bounded item windows.
    flag(root, "history-legacy").await?;
    ui.open_history_pages();
    idle(ui).await?;
    if !ui.history_pages.smoke_summary().contains("legacy read") {
        return Err("Legacy history fallback was not disclosed".into());
    }
    language_roundtrip(ui, root, "legacy").await?;
    ui.history_pages.smoke_older();
    idle(ui).await?;
    if !ids(ui).iter().any(|id| id.starts_with('h')) {
        return Err("Legacy history did not page".into());
    }
    ui.history_pages.smoke_live();
    remove(root, "history-legacy").await?;
    // Real backend selection must cancel a read and keep each session's draft.
    flag(root, "history-delay").await?;
    ui.open_history_pages();
    until("second delayed read", || {
        !root.join("history-delay").exists()
    })
    .await?;
    let backend = ui
        .state
        .borrow()
        .backends
        .iter()
        .find(|b| b.kind == "opencode")
        .cloned()
        .ok_or("OpenCode fixture backend missing")?;
    ui.switch_backend(backend);
    until("OpenCode catalog", || {
        ui.state.borrow().ready
            && ui
                .state
                .borrow()
                .threads
                .iter()
                .any(|t| t.id == "oc-target")
    })
    .await?;
    let thread = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|t| t.id == "oc-target")
        .cloned()
        .unwrap();
    ui.select(thread);
    until("OpenCode history", || {
        !ui.state.borrow().loading && ui.state.borrow().items.iter().any(|i| i.id == "p249")
    })
    .await?;
    ui.composer
        .buffer()
        .set_text("OpenCode unsent draft 中文😀 ");
    glib::timeout_future(Duration::from_millis(500)).await;
    if ui.history_pages.active() {
        return Err("A late Codex page leaked into OpenCode".into());
    }
    ui.open_history_pages();
    idle(ui).await?;
    if ids(ui).first().map(String::as_str) != Some("p130") {
        return Err(format!(
            "OpenCode parts did not page: {}",
            ui.history_pages.smoke_summary()
        ));
    }
    ui.history_pages.smoke_older();
    idle(ui).await?;
    if ids(ui).first().map(String::as_str) != Some("p10") {
        return Err("OpenCode dropped parts within a message".into());
    }
    ui.history_pages.smoke_older();
    idle(ui).await?;
    if ids(ui).first().map(String::as_str) != Some("p0") {
        return Err("OpenCode did not reach the beginning of the part list".into());
    }
    ui.history_pages.smoke_older();
    idle(ui).await?;
    if ids(ui) != ["oc-old-part"] {
        return Err("OpenCode cursor seam duplicated or skipped a message".into());
    }
    language_roundtrip(ui, root, "opencode").await?;
    capture("history-opencode.png");
    if ui.composer_text() != "OpenCode unsent draft 中文😀 " {
        return Err("OpenCode draft was changed by paging".into());
    }
    ui.history_pages.smoke_live();
    let backend = ui
        .state
        .borrow()
        .backends
        .iter()
        .find(|b| b.id == "codex")
        .cloned()
        .unwrap();
    ui.switch_backend(backend);
    until("Codex draft restore", || {
        !ui.state.borrow().loading
            && ui
                .state
                .borrow()
                .selected
                .as_ref()
                .is_some_and(|t| t.id == "target")
            && ui.composer_text() == "Unsent history draft 中文😀 "
    })
    .await?;
    ui.open_history_pages();
    idle(ui).await?;
    ui.composer
        .buffer()
        .set_text("Native history manual fake send");
    until("manual send enabled", || ui.send.is_sensitive()).await?;
    ui.send.emit_clicked();
    if ui.history_pages.active() {
        return Err("Manual Send did not return to Live".into());
    }
    until("fake manual reply", || {
        transcript_contains(ui, "manual-answer")
            && ui.composer_text().is_empty()
            && ui.state.borrow().active_turn.is_none()
    })
    .await?;
    Ok(
        json!({"ok":true,"liveHistoryLanguage":true,"localePreservesSelectionAndPosition":true,"largeTurnSlices":true,"cursorPaging":true,"legacyPaging":true,"nativeHistoricalMarkdown":true,"loadedPageSearch":true,"liveDeliveryIsolation":true,"stableReadingPosition":true,"draftRetention":true,"errorRetentionAndRetry":true,"cancelLateReply":true,"backendGenerationIsolation":true,"opencodePartPaging":true,"opencodeCursorPaging":true,"manualSendReturnsLive":true,"firstOpenMs":first_open_ms,"pagingHeartbeat":responsiveness}),
    )
}
