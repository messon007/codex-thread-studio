//! Private provider deltas through the real gateway, CPU lane and GTK list.
use super::*;
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let at = std::time::Instant::now();
    while !test() {
        if at.elapsed() > Duration::from_secs(20) {
            return Err(format!("Streaming acceptance timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
async fn emit(root: &std::path::Path, values: Value) -> Result<(), String> {
    let root = root.to_owned();
    gio::spawn_blocking(move || {
        let p = root.join("stream-events.tmp");
        std::fs::write(&p, serde_json::to_vec(&values).unwrap()).map_err(|e| e.to_string())?;
        std::fs::rename(p, root.join("stream-events.json")).map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "Streaming fixture writer stopped")?
}
fn item(ui: &Ui) -> Option<Arc<Item>> {
    ui.state
        .borrow()
        .items
        .iter()
        .find(|i| i.id == "stream-assistant")
        .cloned()
}
fn delta(text: &str) -> Value {
    json!({"method":"item/agentMessage/delta","params":{"threadId":"target","turnId":"live-stream","itemId":"stream-assistant","delta":text}})
}
pub async fn run(ui: &Rc<Ui>, root: &std::path::Path) -> Result<Value, String> {
    until("initial history verified", || {
        ui.state.borrow().history_verified && !ui.state.borrow().loading
    })
    .await?;
    ui.composer
        .buffer()
        .set_text("Unsent streaming draft 中文😀 ");
    let started = std::time::Instant::now();
    let ticks = Rc::new(std::cell::Cell::new(0u64));
    let gap = Rc::new(std::cell::Cell::new(0u128));
    let last = Rc::new(std::cell::Cell::new(started));
    let (t, g, l) = (ticks.clone(), gap.clone(), last.clone());
    let timer = glib::timeout_add_local(Duration::from_millis(16), move || {
        t.set(t.get() + 1);
        g.set(
            g.get()
                .max(l.replace(std::time::Instant::now()).elapsed().as_millis()),
        );
        glib::ControlFlow::Continue
    });
    struct Timer(Option<glib::SourceId>);
    impl Drop for Timer {
        fn drop(&mut self) {
            if let Some(t) = self.0.take() {
                t.remove();
            }
        }
    }
    let _timer = Timer(Some(timer));
    let seed = std::fs::read_to_string(root.join("stream-seed.md")).map_err(|e| e.to_string())?;
    emit(root,json!([
        {"method":"turn/started","params":{"threadId":"target","turn":{"id":"live-stream"}}},
        {"method":"item/started","params":{"threadId":"target","turnId":"live-stream","item":{"id":"stream-assistant","type":"agentMessage","text":""}}},delta(&seed)
    ])).await?;
    until("rich reply before completion", || {
        !ui.state.borrow().render_dirty
            && item(ui).is_some_and(|i| {
                i.rich
                    .as_ref()
                    .is_some_and(|r| r.source_bytes == i.body.len() && r.rows.len() >= 6)
            })
            && ui.state.borrow().active_turn.as_deref() == Some("live-stream")
    })
    .await?;
    let first_ms = started.elapsed().as_millis();
    let before_jobs = ui.streaming.borrow().serial;
    let values = (0..40)
        .map(|i| delta(&format!(" chunk-{i} 中文😀")))
        .collect::<Vec<_>>();
    emit(root, json!(values)).await?;
    until("first burst rendered", || {
        !ui.state.borrow().render_dirty
            && item(ui).is_some_and(|i| {
                i.body.contains("chunk-39")
                    && i.rich
                        .as_ref()
                        .is_some_and(|r| r.source_bytes == i.body.len())
            })
    })
    .await?;
    until("diagrams render before completion", || {
        ui.chat_diagrams.streaming_probe().1
    })
    .await?;
    let diagram_ids = ui.chat_diagrams.streaming_probe().0;
    let probe = crate::chat_ui::stream_probe(&ui.transcript, &ui.transcript_view).await?;
    let values = (40..80)
        .map(|i| delta(&format!(" chunk-{i} 中文😀")))
        .collect::<Vec<_>>();
    emit(root, json!(values)).await?;
    until("second burst rendered", || {
        !ui.state.borrow().render_dirty
            && item(ui).is_some_and(|i| {
                i.body.contains("chunk-79")
                    && i.rich
                        .as_ref()
                        .is_some_and(|r| r.source_bytes == i.body.len())
            })
    })
    .await?;
    let current = item(ui).unwrap();
    probe.verify(&ui.transcript, &current)?;
    if ui.chat_diagrams.streaming_probe().0 != diagram_ids {
        return Err("Streaming restarted unchanged diagram widgets".into());
    }
    let jobs = ui.streaming.borrow().serial - before_jobs;
    if jobs == 0 || jobs >= 80 {
        return Err("Delta parsing did not coalesce".into());
    }
    if !ui.stop.is_sensitive() || ui.composer_text() != "Unsent streaming draft 中文😀 " {
        return Err("Streaming blocked conversation controls or changed the draft".into());
    }
    super::smoke::capture(&ui.window, &root.join("streaming-active.png"));
    let final_text = format!("{} final-tail", current.body);
    emit(root,json!([
        {"method":"item/completed","params":{"threadId":"target","turnId":"live-stream","item":{"id":"stream-assistant","type":"agentMessage","text":final_text}}},
        {"method":"turn/completed","params":{"threadId":"target","turn":{"id":"live-stream","status":"completed"}}},
        {"method":"thread/status/changed","params":{"threadId":"target","status":{"type":"idle"}}}
    ])).await?;
    until("completed final text rendered", || {
        !ui.state.borrow().render_dirty
            && ui.state.borrow().active_turn.is_none()
            && item(ui).is_some_and(|i| {
                i.body.ends_with("final-tail")
                    && i.rich
                        .as_ref()
                        .is_some_and(|r| r.source_bytes == i.body.len())
            })
    })
    .await?;
    probe.verify(&ui.transcript, &item(ui).unwrap())?;
    // Queue real Rust work, change selection synchronously, and let its result
    // arrive for the previous generation/session. It must never replace history.
    let source = item(ui).unwrap();
    let context = ui.streaming_context().unwrap();
    let serial = {
        let mut b = ui.streaming.borrow_mut();
        b.serial += 1;
        let s = b.serial;
        b.pending = Some(s);
        s
    };
    ui.command(Command::Streaming(Some(crate::streaming::Request {
        context,
        serial,
        source,
    })));
    let other = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|t| t.id == "other")
        .cloned()
        .ok_or("Other fixture session missing")?;
    ui.select(other);
    until("other session selected", || {
        ui.state
            .borrow()
            .selected
            .as_ref()
            .is_some_and(|t| t.id == "other")
            && ui.state.borrow().history_verified
            && !ui.state.borrow().loading
    })
    .await?;
    glib::timeout_future(Duration::from_millis(200)).await;
    if item(ui).is_some() {
        return Err("Old streaming work crossed session selection".into());
    }
    Ok(
        json!({"ok":true,"firstRichMs":first_ms,"deltaCount":80,"parseRequests":jobs,"richBeforeCompletion":true,"stableParagraphAndTable":true,"currentActionSnapshot":true,"diagramsBeforeCompletion":true,"diagramWidgetsStable":true,"completionPreserved":true,"lateSessionWorkRejected":true,"draftRetained":true,"eventMaxMs":ui.event_max_ms.get(),"reconcileMaxMs":ui.render_max_ms.get(),"mainLoopTicks":ticks.get(),"maxHeartbeatGapMs":gap.get()}),
    )
}
