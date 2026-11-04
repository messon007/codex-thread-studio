use super::*;
use std::path::Path;
async fn until(test: impl Fn() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !test() {
        if start.elapsed() > Duration::from_secs(15) {
            return Err("Conversation images did not become ready".into());
        }
        glib::timeout_future(Duration::from_millis(25)).await;
    }
    Ok(())
}
fn image_row(ui: &Ui) -> Result<(u32, String), String> {
    for index in 0..ui.transcript.n_items() {
        let value = ui
            .transcript
            .item(index)
            .and_downcast::<glib::BoxedAnyObject>()
            .unwrap();
        let row = value.borrow::<crate::chat_ui::Row>();
        if !ui
            .state
            .borrow()
            .items
            .iter()
            .any(|item| Arc::ptr_eq(item, &row.item))
        {
            continue;
        }
        if let crate::chat_ui::Part::Attachment(image) = row.part {
            if let crate::conversation_images::Source::Inline { token } =
                &row.item.images[image].source
            {
                return Ok((index, token.clone()));
            }
        }
    }
    Err("Inline history attachment row missing".into())
}
fn loaded(ui: &Ui, token: &str, mapped: bool) -> usize {
    fn walk(widget: &gtk::Widget, name: &str, mapped: bool) -> usize {
        if widget.widget_name() == name {
            return crate::images_ui::loaded_count(widget, mapped);
        }
        let mut count = 0;
        let mut child = widget.first_child();
        while let Some(w) = child {
            child = w.next_sibling();
            count += walk(&w, name, mapped);
        }
        count
    }
    walk(
        ui.transcript_view.upcast_ref(),
        &format!("attachment:{token}"),
        mapped,
    )
}
fn visible(ui: &Ui, token: &str) -> bool {
    fn walk(widget: &gtk::Widget, viewport: &gtk::ScrolledWindow, name: &str) -> bool {
        if widget.widget_name() == name {
            return crate::images_ui::loaded_count(widget, true) > 0
                && widget.first_child().is_some_and(|picture| {
                    picture.compute_bounds(viewport).is_some_and(|r| {
                        let width = r.x().min(0.0) + r.width().min(viewport.width() as f32 - r.x());
                        let height =
                            r.y().min(0.0) + r.height().min(viewport.height() as f32 - r.y());
                        width >= 32.0 && height >= r.height().min(160.0)
                    })
                });
        }
        let mut child = widget.first_child();
        while let Some(w) = child {
            child = w.next_sibling();
            if walk(&w, viewport, name) {
                return true;
            }
        }
        false
    }
    walk(
        ui.transcript_view.upcast_ref(),
        &ui.transcript_scroll,
        &format!("attachment:{token}"),
    )
}
async fn reveal(ui: &Ui, index: u32, token: &str) -> Result<(), String> {
    // Use the same explicit-navigation coordinator as conversation search, so
    // an already queued follow-to-tail cannot move this row offscreen while
    // its lazy image loads. Wait for actual viewport intersection rather than
    // treating GTK's mapped overscan rows as visible thumbnails.
    until(|| {
        ui.transcript_navigation
            .jump(&ui.transcript_view, index, gtk::ListScrollFlags::NONE);
        visible(ui, token)
    })
    .await
}
pub(super) async fn run(
    ui: &Rc<Ui>,
    directory: &Path,
    capture: impl Fn(&str),
) -> Result<Value, String> {
    let ticks = Rc::new(std::cell::Cell::new(0u64));
    let gap = Rc::new(std::cell::Cell::new(0u128));
    let last = Rc::new(std::cell::Cell::new(std::time::Instant::now()));
    let phase = Rc::new(std::cell::Cell::new("startup"));
    let slow_phase = Rc::new(std::cell::Cell::new("startup"));
    let active = phase.clone();
    let slow = slow_phase.clone();
    let phase_gaps = Rc::new(RefCell::new(std::collections::BTreeMap::new()));
    let observed = phase_gaps.clone();
    let (count, maximum, previous) = (ticks.clone(), gap.clone(), last.clone());
    let timer = glib::timeout_add_local(Duration::from_millis(16), move || {
        let elapsed = previous.get().elapsed().as_millis();
        let mut gaps = observed.borrow_mut();
        let entry = gaps.entry(active.get()).or_insert(0u128);
        *entry = (*entry).max(elapsed);
        drop(gaps);
        if elapsed > maximum.get() {
            maximum.set(elapsed);
            slow.set(active.get());
        }
        previous.set(std::time::Instant::now());
        count.set(count.get() + 1);
        glib::ControlFlow::Continue
    });
    let snapshot_time = std::cell::Cell::new(Duration::ZERO);
    let result = run_inner(ui, directory, &phase, |name| {
        let elapsed = last.get().elapsed().as_millis();
        let mut gaps = phase_gaps.borrow_mut();
        let entry = gaps.entry(phase.get()).or_insert(0u128);
        *entry = (*entry).max(elapsed);
        drop(gaps);
        let start = std::time::Instant::now();
        capture(name);
        snapshot_time.set(snapshot_time.get() + start.elapsed());
        // PNG readback/encoding belongs only to this debug fixture. Do not
        // attribute that deliberate blocking operation to the production image lane.
        last.set(std::time::Instant::now());
    })
    .await;
    timer.remove();
    result.map(|mut v| {
        v["mainLoopTicks"] = json!(ticks.get());
        v["maximumMainLoopGapOutsideSnapshotsMs"] = json!(gap.get());
        v["snapshotWallMs"] = json!(snapshot_time.get().as_millis());
        v["maximumImageCallbackMs"] = json!(ui.images.maximum_callback_ms.get());
        v["slowestGapPhase"] = json!(slow_phase.get());
        v["maximumUiEventMs"] = json!(ui.event_max_ms.get());
        v["maximumUiReconcileMs"] = json!(ui.render_max_ms.get());
        v["mainLoopGapsByPhaseMs"] = json!(*phase_gaps.borrow());
        v
    })
}
async fn run_inner(
    ui: &Rc<Ui>,
    _directory: &Path,
    phase: &std::cell::Cell<&'static str>,
    capture: impl Fn(&str),
) -> Result<Value, String> {
    until(|| {
        ui.state.borrow().ready
            && !ui.state.borrow().catalog_loading
            && ui
                .state
                .borrow()
                .threads
                .iter()
                .any(|t| t.id == "native-smoke-thread")
    })
    .await?;
    let original = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|t| t.id == "native-smoke-thread")
        .cloned()
        .unwrap();
    let opened = std::time::Instant::now();
    phase.set("codex-history");
    ui.select(original.clone());
    until(|| {
        !ui.state.borrow().loading && ui.state.borrow().items.iter().any(|i| i.images.len() == 3)
    })
    .await?;
    until(|| image_row(ui).is_ok()).await?;
    let (index, token) = image_row(ui)?;
    ui.transcript_view
        .scroll_to(index, gtk::ListScrollFlags::NONE, None);
    capture("history-codex-inline.png");
    reveal(ui, index, &token).await?;
    let first_thumbnail_ms = opened.elapsed().as_millis();
    capture("history-codex-inline.png");
    if !visible(ui, &token) {
        return Err("Codex thumbnail is outside the captured viewport".into());
    }
    let media = ui
        .state
        .borrow()
        .items
        .iter()
        .find(|i| !i.images.is_empty())
        .unwrap()
        .clone();
    if media.body.contains("base64")
        || media.images.iter().map(|i| i.weight()).sum::<usize>() > 4096
    {
        return Err("Encoded image payload reached the GTK presentation model".into());
    }
    phase.set("inline-viewer");
    ui.activate_reference_from(&format!("studio-attachment:{token}"), "");
    let viewer = ui
        .image_viewer
        .smoke_inline(&token, |name| capture(name))
        .await?;
    ui.show_tool("image");
    capture("history-inline-viewer.png");
    ui.image_viewer.close();
    ui.tools.set_visible(false);
    ui.update_workspace();
    phase.set("cache-switch");
    let sibling = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|t| t.id == "native-cache-thread")
        .cloned()
        .unwrap();
    ui.select(sibling);
    until(|| !ui.state.borrow().loading).await?;
    capture("history-sibling.png");
    if loaded(ui, &token, false) != 0 {
        return Err("An old image texture survived the conversation switch".into());
    }
    ui.select(original);
    until(|| !ui.state.borrow().loading && image_row(ui).is_ok()).await?;
    until(|| image_row(ui).is_ok()).await?;
    let (index, _) = image_row(ui)?;
    ui.transcript_view
        .scroll_to(index, gtk::ListScrollFlags::NONE, None);
    capture("history-restored.png");
    reveal(ui, index, &token).await?;
    capture("history-restored.png");
    if !visible(ui, &token) {
        return Err("Restored thumbnail is outside the captured viewport".into());
    }
    phase.set("opencode-startup");
    let backends = ui.state.borrow().backends.clone();
    let index = backends
        .iter()
        .position(|b| b.kind == "opencode")
        .ok_or("OpenCode fixture backend missing")?;
    ui.backend.set_selected(index as u32);
    until(|| {
        ui.state.borrow().ready
            && !ui.state.borrow().catalog_loading
            && ui
                .state
                .borrow()
                .threads
                .iter()
                .any(|t| t.id == "native-oc-thread")
    })
    .await?;
    let thread = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|t| t.id == "native-oc-thread")
        .cloned()
        .unwrap();
    phase.set("opencode-history");
    ui.select(thread);
    until(|| {
        !ui.state.borrow().loading && ui.state.borrow().items.iter().any(|i| i.images.len() == 2)
    })
    .await?;
    until(|| image_row(ui).is_ok()).await?;
    let (index, oc_token) = image_row(ui)?;
    ui.transcript_view
        .scroll_to(index, gtk::ListScrollFlags::NONE, None);
    capture("history-opencode.png");
    reveal(ui, index, &oc_token).await?;
    capture("history-opencode.png");
    if !visible(ui, &oc_token) {
        return Err("OpenCode thumbnail is outside the captured viewport".into());
    }
    Ok(
        json!({"ok":true,"codexHistory":true,"openCodeHistory":true,"flatAttachmentRows":true,"nativeViewer":viewer,"switchReleasesTextures":true,"cacheRestore":true,"boundedMetadata":true,"visibleThumbnails":true,"firstThumbnailIncludingCaptureMs":first_thumbnail_ms}),
    )
}
