//! Exercise the real history entry, SQLite API, navigation and stale replies.
use super::*;
struct Pulse(Option<glib::SourceId>);
impl Drop for Pulse {
    fn drop(&mut self) {
        if let Some(source) = self.0.take() {
            source.remove();
        }
    }
}
async fn wait(ui: &Ui, query: &str, count: usize) -> Result<(), String> {
    let at = Instant::now();
    loop {
        if ui.router.borrow().page.is_none()
            && ui
                .router
                .borrow()
                .view
                .as_ref()
                .is_some_and(|(spec, h)| spec.query == query && h.ids.len() == count)
        {
            return Ok(());
        }
        if at.elapsed() > Duration::from_secs(15) {
            return Err(format!(
                "Router search did not settle: {query:?}, {count} results"
            ));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
}
pub(super) async fn run(
    ui: &Rc<Ui>,
    directory: &std::path::Path,
    oldest: &str,
) -> Result<Value, String> {
    let ticks = Rc::new(std::cell::Cell::new(0u64));
    let gap = Rc::new(std::cell::Cell::new(0u128));
    let last = Rc::new(std::cell::Cell::new(Instant::now()));
    let (count, max_gap, previous) = (ticks.clone(), gap.clone(), last.clone());
    let _pulse = Pulse(Some(glib::timeout_add_local(
        Duration::from_millis(16),
        move || {
            let now = Instant::now();
            max_gap.set(
                max_gap
                    .get()
                    .max(now.duration_since(previous.replace(now)).as_millis()),
            );
            count.set(count.get() + 1);
            glib::ControlFlow::Continue
        },
    )));
    let original = ui.composer_text();
    let key = ui.composer_key();
    let draft = "Keep unsent search draft 中文😀 ";
    ui.composer.buffer().set_text(draft);
    let at = Instant::now();
    ui.window
        .lookup_action("find-conversation")
        .ok_or("Find action missing")?
        .activate(None);
    if !ui.router_bar.search_toggle.is_active() || ui.finder.widget.is_visible() {
        return Err("Find action did not select persisted Router search".into());
    }
    ui.router_bar.search.set_text("été");
    wait(ui, "été", 1).await?;
    let first_ms = at.elapsed().as_millis();
    if !ui
        .router
        .borrow()
        .view
        .as_ref()
        .unwrap()
        .1
        .ids
        .contains(&oldest.to_owned())
    {
        return Err("Search did not reach the oldest stored request".into());
    }
    ui.router_bar.case.set_active(true);
    wait(ui, "été", 0).await?;
    ui.router_bar.case.set_active(false);
    wait(ui, "été", 1).await?;
    ui.router_bar.search.set_text("[]%_.* ");
    wait(ui, "[]%_.* ", 1).await?;
    super::super::smoke::capture(&ui.window, &directory.join("router-search-literal.png"));
    ui.router_bar.search.set_text("[]%_.*  ");
    wait(ui, "[]%_.*  ", 0).await?;
    let previous = ui.router.borrow().view.clone().unwrap();
    ui.router_bar.search.set_text("Historical request 1");
    ui.router_search_changed();
    let serial = ui.router.borrow().page.ok_or("No pending Router search")?;
    let mut stale = previous.0.clone();
    stale.query = "Historical request 1".into();
    // The entry changed before its debounced callback. An in-flight response
    // must already be ineligible, even if its request serial is still current.
    ui.router_bar.search.set_text("Historical request 2");
    ui.router_event(
        serial,
        Ok(Reply::Page {
            spec: stale,
            history: previous.1,
            changed: vec![],
            stamps: HashMap::new(),
        }),
    );
    if ui.router.borrow().view.as_ref().unwrap().0.query != "[]%_.*  " {
        return Err("A superseded query replaced the current Router view".into());
    }
    wait(ui, "Historical request 2", 11).await?;
    ui.router_bar.search.set_text("Historical request");
    wait(ui, "Historical request", 20).await?;
    ui.router_bar.older.emit_clicked();
    let at = Instant::now();
    while ui.router.borrow().page.is_some()
        || !ui
            .router
            .borrow()
            .view
            .as_ref()
            .is_some_and(|(s, _)| s.query == "Historical request" && s.before.is_some())
    {
        if at.elapsed() > Duration::from_secs(15) {
            return Err("Filtered older page did not load".into());
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    let page = ui.router.borrow().view.as_ref().unwrap().1.ids.clone();
    glib::timeout_future(Duration::from_millis(2400)).await;
    if ui.router.borrow().view.as_ref().unwrap().1.ids != page {
        return Err("Live polling replaced a filtered history page".into());
    }
    ui.router_bar.newer.emit_clicked();
    let at = Instant::now();
    while ui.router.borrow().page.is_some()
        || ui.router.borrow().view.as_ref().unwrap().1.newer.is_some()
    {
        if at.elapsed() > Duration::from_secs(15) {
            return Err("Filtered newer page did not load".into());
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    if ui.composer_text() != draft || ui.composer_key() != key {
        return Err("History search changed the selected conversation or its draft".into());
    }
    super::super::smoke::capture(&ui.window, &directory.join("router-search-page.png"));
    ui.router_bar.search.emit_stop_search();
    wait(ui, "", 20).await?;
    if ui.router_bar.search_bar.is_search_mode() || ui.router_bar.search_toggle.is_active() {
        return Err("Closing search did not restore normal history".into());
    }
    let source = ui
        .state
        .borrow()
        .selected
        .clone()
        .ok_or("Router selection missing")?;
    ui.router_bar.search_toggle.set_active(true);
    ui.router_bar.search.set_text("Historical request 1");
    ui.router_search_changed();
    let other=Thread::from_value(&json!({"id":"oc-router","name":"OpenCode Router","cwd":directory.join("data/codex-thread-studio/router").to_string_lossy()})).ok_or("No alternate Router")?;
    ui.router_select_reference("opencode", other);
    async fn selected(ui: &Ui, key: &str) -> Result<(), String> {
        let at = Instant::now();
        while ui.composer_key().as_deref() != Some(key)
            || ui.state.borrow().loading
            || !ui.state.borrow().options_loaded
            || ui.router.borrow().page.is_some()
            || !ui
                .router
                .borrow()
                .view
                .as_ref()
                .is_some_and(|(spec, _)| spec.controller == key)
        {
            if at.elapsed() > Duration::from_secs(15) {
                return Err(format!("Router search scope did not switch to {key}"));
            }
            glib::timeout_future(Duration::from_millis(20)).await;
        }
        Ok(())
    }
    selected(ui, "opencode:oc-router").await?;
    if !ui.router_bar.search.text().is_empty()
        || !ui.router.borrow().view.as_ref().unwrap().0.query.is_empty()
    {
        return Err("A prior controller's search leaked into another Router".into());
    }
    ui.router_select_reference("codex", source);
    selected(ui, "codex:router").await?;
    wait(ui, "", 20).await?;
    if ui.composer_text() != draft {
        return Err("Changing Router during search lost the original draft".into());
    }
    ui.composer.buffer().set_text(&original);
    Ok(
        json!({"unicode":true,"case":true,"literalSpaces":true,"olderStoredRequest":true,"filteredPaging":true,"livePollRetention":true,"staleQueryGuard":true,"draftRetention":true,"closeRestoresHistory":true,"controllerSwitchIsolation":true,"firstResultMs":first_ms,"heartbeatTicks":ticks.get(),"maximumGapMs":gap.get()}),
    )
}
