//! Native output buttons, source pages and export through private real I/O.
use super::*;
use std::{cell::Cell, path::Path};
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !test() {
        if start.elapsed() > Duration::from_secs(20) {
            return Err(format!("Conversation output acceptance timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
fn button(widget: &gtk::Widget, key: &str) -> Option<gtk::Button> {
    if widget.widget_name() == key {
        return widget.clone().downcast().ok();
    }
    let mut child = widget.first_child();
    while let Some(w) = child {
        if let Some(found) = button(&w, key) {
            return Some(found);
        }
        child = w.next_sibling();
    }
    None
}
async fn navigation_updates(ui: &Rc<Ui>, root: &Path, phase: &str) -> Result<String, String> {
    let mut events = Vec::new();
    let mut last = String::new();
    for index in 0..3 {
        last = format!("navigation-{phase}-{index}");
        let message = format!("{last}-message");
        events.push(json!({"type":"message.updated","properties":{"info":{
            "id":message,"sessionID":"oc-target","role":"assistant","parentID":"oc-user","time":{"completed":1}
        }}}));
        events.push(json!({"type":"message.part.updated","properties":{"part":{
            "id":last,"messageID":message,"sessionID":"oc-target","type":"text",
            "text":format!("# {last}\n\n{}",format!("{}\n\n","Native navigation live update 中文😀 ".repeat(10)).repeat(18))
        }}}));
    }
    let path = root.join("output-navigation-events.json");
    let temporary = root.join("output-navigation-events.tmp");
    gio::spawn_blocking(move || {
        std::fs::write(&temporary, serde_json::to_vec(&events).unwrap())?;
        std::fs::rename(temporary, path)
    })
    .await
    .map_err(|_| "Navigation fixture worker stopped")?
    .map_err(|e| e.to_string())?;
    until("native live navigation updates rendered", || {
        ui.state
            .borrow()
            .items
            .iter()
            .any(|item| item.id == last && item.rich.is_some())
    })
    .await?;
    ui.render_items();
    Ok(last)
}
async fn open(ui: &Rc<Ui>, id: &str, root: &Path) -> Result<(), String> {
    until("header row", || {
        (0..ui.transcript.n_items()).any(|n| {
            ui.transcript
                .item(n)
                .and_downcast::<glib::BoxedAnyObject>()
                .is_some_and(|v| {
                    let r = v.borrow::<crate::chat_ui::Row>();
                    r.item.id == id && r.part == crate::chat_ui::Part::Header
                })
        })
    })
    .await?;
    let mut header = None;
    let mut token = None;
    for n in 0..ui.transcript.n_items() {
        let v = ui
            .transcript
            .item(n)
            .and_downcast::<glib::BoxedAnyObject>()
            .unwrap();
        let r = v.borrow::<crate::chat_ui::Row>();
        if r.item.id == id && r.part == crate::chat_ui::Part::Header {
            header = Some(n);
            token = r.item.output.as_ref().map(|s| s.token.clone());
            break;
        }
    }
    let token = token.ok_or_else(|| format!("No complete source retained for {id}"))?;
    if id == "oc-user" {
        // A live/history reconciliation can queue a follow-tail idle just
        // before the user chooses an earlier message or search result.
        let adjustment = ui.transcript_scroll.vadjustment();
        adjustment.set_value((adjustment.upper() - adjustment.page_size()).max(0.0));
        ui.render_items();
        let object = ui
            .transcript
            .item(header.unwrap())
            .and_downcast::<glib::BoxedAnyObject>()
            .unwrap();
        let item = object.borrow::<crate::chat_ui::Row>().item.clone();
        let hit = crate::session_search::Occurrence {
            turn_id: item.turn_id.clone(),
            item_id: item.id.clone(),
            kind: crate::session_search::Filter::User,
            snippet: String::new(),
            highlight: 0..0,
            source: None,
            block: None,
            table_row: None,
            paragraph_page: None,
            paragraph_range: None,
        };
        ui.finder
            .on_select
            .borrow()
            .as_ref()
            .ok_or("Search selection handler missing")?(hit);
        // A second update can arrive before GTK allocates the selected row.
        ui.render_items();
        until("search jump beats queued follow-tail", || {
            ui.chat_visibility
                .smoke_visible(id, ui.transcript_scroll.upcast_ref())
        })
        .await?;
        glib::timeout_future(Duration::from_millis(180)).await;
        if !ui
            .chat_visibility
            .smoke_visible(id, ui.transcript_scroll.upcast_ref())
        {
            return Err(
                "Queued follow-tail replaced the selected conversation search result".into(),
            );
        }
        let latest = navigation_updates(ui, root, "reading").await?;
        glib::timeout_future(Duration::from_millis(180)).await;
        if !ui
            .chat_visibility
            .smoke_visible(id, ui.transcript_scroll.upcast_ref())
        {
            return Err("Live provider updates displaced the selected search result".into());
        }
        adjustment.set_value((adjustment.upper() - adjustment.page_size()).max(0.0));
        ui.render_items();
        until("follow-tail resumes after returning to the end", || {
            ui.chat_visibility
                .visible_ends(ui.transcript_scroll.upcast_ref())
                .iter()
                .any(|(item, _)| item == &latest)
        })
        .await?;
        let latest = navigation_updates(ui, root, "resumed").await?;
        until("following newly appended provider messages", || {
            ui.chat_visibility
                .visible_ends(ui.transcript_scroll.upcast_ref())
                .iter()
                .any(|(item, _)| item == &latest)
        })
        .await?;
    }
    ui.transcript_navigation.jump(
        &ui.transcript_view,
        header.unwrap(),
        gtk::ListScrollFlags::FOCUS,
    );
    let key = format!("output:{token}");
    until("mapped full-output button", || {
        button(ui.transcript_view.upcast_ref(), &key)
            .is_some_and(|b| b.is_mapped() && b.is_sensitive())
    })
    .await
    .map_err(|error| {
        format!(
            "{error}; item={id}, header={}, rows={}, listMapped={}, control={:?}",
            header.unwrap(),
            ui.transcript.n_items(),
            ui.transcript_view.is_mapped(),
            button(ui.transcript_view.upcast_ref(), &key).map(|b| (
                b.is_mapped(),
                b.is_sensitive(),
                b.width(),
                b.height()
            ))
        )
    })?;
    let control = button(ui.transcript_view.upcast_ref(), &key).unwrap();
    if std::env::var_os("STUDIO_OWNED_MUTTER_INPUT").is_some() {
        if !control.grab_focus() {
            return Err("Full-output control could not receive keyboard focus".into());
        }
        let path = root.join("output-key-activate");
        gio::spawn_blocking(move || {
            std::fs::write(path, b"activate owned control").map_err(|e| e.to_string())
        })
        .await
        .map_err(|_| "Fixture input worker stopped")??;
    } else {
        control.emit_clicked();
    }
    until("output viewer opened", || ui.output.smoke_open()).await
}
pub async fn run(ui: &Rc<Ui>, root: &Path) -> Result<Value, String> {
    until("bounded initial outputs", || {
        let s = ui.state.borrow();
        s.ready
            && s.history_verified
            && !s.loading
            && s.options_loaded
            && s.items.iter().any(|i| i.id == "large-assistant")
    })
    .await?;
    ui.composer
        .buffer()
        .set_text("Unsent output-viewer draft 中文😀 ");
    let before = ui.state.borrow().items.clone();
    let ticks = Rc::new(Cell::new(0u64));
    let gap = Rc::new(Cell::new(0u64));
    let last = Cell::new(std::time::Instant::now());
    let t = ticks.clone();
    let g = gap.clone();
    let timer = glib::timeout_add_local(Duration::from_millis(10), move || {
        let now = std::time::Instant::now();
        g.set(
            g.get()
                .max(now.duration_since(last.replace(now)).as_millis() as u64),
        );
        t.set(t.get() + 1);
        glib::ControlFlow::Continue
    });
    struct Timer(Option<glib::SourceId>);
    impl Drop for Timer {
        fn drop(&mut self) {
            if let Some(id) = self.0.take() {
                id.remove();
            }
        }
    }
    let mut timer = Timer(Some(timer));
    let mut results = Vec::new();
    for case in ["assistant", "command", "reasoning", "tool", "files", "user"] {
        let case_start = std::time::Instant::now();
        open(ui, &format!("large-{case}"), root).await?;
        let entry_ms = case_start.elapsed().as_millis();
        let mut result = ui.output.smoke(root, case).await?;
        result["headerEntryMs"] = json!(entry_ms);
        result["wholeCaseMs"] = json!(case_start.elapsed().as_millis());
        results.push(result);
        if ui.composer_text() != "Unsent output-viewer draft 中文😀 "
            || ui.state.borrow().items.len() != before.len()
            || !ui
                .state
                .borrow()
                .items
                .iter()
                .zip(&before)
                .all(|(a, b)| Arc::ptr_eq(a, b))
        {
            return Err("Reading a full snapshot modified the live transcript or draft".into());
        }
    }
    let heartbeat = json!({"ticks":ticks.get(),"maximumGapMs":gap.get()});
    timer.0.take().unwrap().remove();
    ui.open_history_pages();
    until("historical output page", || {
        !ui.history_pages.smoke_busy()
            && ui.history_pages.active()
            && ui
                .history_pages
                .items()
                .is_some_and(|items| items.iter().any(|i| i.id == "large-assistant"))
    })
    .await?;
    open(ui, "large-assistant", root).await?;
    results.push(ui.output.smoke(root, "historical").await?);
    ui.history_pages.smoke_live();
    let backend = ui
        .state
        .borrow()
        .backends
        .iter()
        .find(|b| b.kind == "opencode")
        .cloned()
        .ok_or("OpenCode fixture missing")?;
    let index = ui
        .state
        .borrow()
        .backends
        .iter()
        .position(|b| b.id == backend.id)
        .ok_or("Backend selector target missing")?;
    ui.backend.set_selected(index as u32);
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
    let selected = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|t| t.id == "oc-target")
        .cloned()
        .unwrap();
    ui.select(selected);
    until("OpenCode large output", || {
        let s = ui.state.borrow();
        !s.loading && s.history_verified && s.items.iter().any(|i| i.id == "oc-large")
    })
    .await?;
    open(ui, "oc-large", root).await?;
    results.push(ui.output.smoke(root, "opencode").await?);
    open(ui, "oc-user", root).await?;
    results.push(ui.output.smoke(root, "opencode-user").await?);
    Ok(
        json!({"ok":true,"outputs":results,"liveTranscriptIsolation":true,"draftRetention":true,"historyOutput":true,"opencodeOutput":true,"literalSearch":true,"errorRetention":true,"staleDialogIsolation":true,"expiredSnapshotError":true,"searchNavigationPriority":true,"liveUpdatesRetainSearch":true,"followTailResumes":true,"pagingHeartbeatIncludingCapture":heartbeat}),
    )
}
