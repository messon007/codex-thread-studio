//! Real owned gateway exit; immutable exports and unsaved edits stay owned.
use super::*;
use std::path::Path;
async fn until(ui: &Ui, label: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !test() {
        if start.elapsed() > Duration::from_secs(20) {
            return Err(format!("Gateway stop {label}: {}", ui.status.text()));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
async fn flag(root: &Path, name: &str, present: bool) -> Result<(), String> {
    let path = root.join(name);
    gio::spawn_blocking(move || {
        if present {
            std::fs::write(path, b"1")
        } else {
            std::fs::remove_file(path)
        }
    })
    .await
    .map_err(|_| "Gateway stop writer stopped")?
    .map_err(|e| e.to_string())
}
pub(super) async fn run(ui: &Rc<Ui>, root: &Path) -> Result<Value, String> {
    let mode = std::env::var("STUDIO_GNOME_SMOKE_DISCOVERY").unwrap_or_default();
    until(ui, "initial history", || {
        let s = ui.state.borrow();
        s.ready && s.history_verified && !s.loading && ui.transcript.n_items() > 0
    })
    .await?;
    let mut viewers = if mode.starts_with("runtime-viewers") {
        let root_text = root.join("project").to_string_lossy().into_owned();
        ui.tables.open(
            &root_text,
            &root
                .join(
                    if std::env::var("STUDIO_GNOME_SMOKE_TABLE_RECOVERY").as_deref() == Ok("xlsx") {
                        "project/recovery.xlsx"
                    } else {
                        "project/recovery.csv"
                    },
                )
                .to_string_lossy(),
        );
        let table = ui.tables.smoke_before_recovery().await?;
        ui.pdf.open(
            &root_text,
            &root.join("project/recovery.pdf").to_string_lossy(),
            0,
        );
        let pdf = ui.pdf.smoke_before_recovery().await?;
        ui.epub.open(
            &root_text,
            &root.join("project/recovery.epub").to_string_lossy(),
        );
        let epub = ui.epub.smoke_before_recovery().await?;
        Some(json!({"table":table,"pdf":pdf,"epub":epub}))
    } else {
        None
    };
    ui.composer
        .buffer()
        .set_text("Never automatically send this queued message");
    ui.enqueue()?;
    // Enqueue intentionally releases an empty queue; pause synchronously before
    // its persistence reply can make that first item eligible for dispatch.
    ui.pause_selected_queue("Private queue stays paused");
    until(ui, "durable paused queue", || {
        ui.state
            .borrow()
            .queues
            .entries
            .get("codex:target")
            .is_some_and(|q| q.paused && q.saving.is_none() && q.messages.len() == 1)
    })
    .await?;
    let draft = "Unsent during gateway exit 中文😀";
    ui.composer.buffer().set_text(draft);
    ui.open_local_file(&root.join("project/discovery.md"));
    until(ui, "document", || ui.reader.is_ready()).await?;
    let stale = ui.reader.smoke_before_gateway_loss().await?;
    let export_text = "Immutable local export after gateway exit 中文😀\n";
    let source =
        gio::spawn_blocking(move || crate::conversation_output::retain(export_text.into()))
            .await
            .map_err(|_| "Export source worker stopped")?;
    flag(root, "hold-gateway-export", true).await?;
    ui.output
        .smoke_gateway_export(source, root.join("gateway-export.txt"));
    let start = std::time::Instant::now();
    loop {
        let path = root.join("export-admitted");
        if gio::spawn_blocking(move || path.exists())
            .await
            .map_err(|_| "Export latch reader stopped")?
        {
            break;
        }
        if start.elapsed() > Duration::from_secs(8) {
            return Err("Local export was not admitted".into());
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    flag(root, "stop-gateway", true).await?;
    until(ui, "owned process exit notice", || ui.gateway_stopped.get()).await?;
    if let Some(before) = viewers.as_mut() {
        before["epub"]["versionAtLoss"] = ui.epub.smoke_position_version().into();
        ui.tables.smoke_offline_worksheet(&before["table"])?;
        ui.epub.smoke_search_offline(&before["epub"])?;
        if std::env::var("STUDIO_GNOME_SMOKE_BOOK_SEARCH").as_deref() == Ok("inflight") {
            flag(root, "hold-epub-search", false).await?;
        }
    }
    if ui.send.is_sensitive()
        || ui.state.borrow().ready
        || !ui.state.borrow().backend_generations.is_empty()
        || ui.recovery.borrow().pending() != 0
    {
        return Err("Gateway loss retained a live connection or retry timer".into());
    }
    if ui.composer_text() != draft || !ui.output.writing() || ui.output.allow_window_close() {
        return Err("Gateway loss abandoned a draft or admitted export".into());
    }
    if !ui
        .state
        .borrow()
        .queues
        .entries
        .get("codex:target")
        .is_some_and(|q| q.paused && !q.connected && q.messages.len() == 1 && q.saving.is_none())
    {
        return Err("Gateway exit changed the retained queue or left it connected".into());
    }
    let editor = ui.reader.smoke_after_gateway_loss(stale)?;
    flag(root, "hold-gateway-export", false).await?;
    until(ui, "confirmed local export receipt", || {
        !ui.output.writing()
    })
    .await?;
    let path = root.join("gateway-export.txt");
    let text = gio::spawn_blocking(move || std::fs::read_to_string(path))
        .await
        .map_err(|_| "Export verifier stopped")?
        .map_err(|e| e.to_string())?;
    if text != export_text || !ui.output.allow_window_close() {
        return Err("Immutable export did not finish exactly after gateway loss".into());
    }
    crate::window_capture::capture(&ui.window, &root.join("gateway-export-finished.png")).await?;
    ui.output.smoke_gateway_export_close();
    until(ui, "output dialog actually closed", || {
        !ui.output.smoke_open()
    })
    .await?;
    // AdwDialog::closed retires the model before its closing sheet finishes
    // painting. Allow that normal animation to settle before the root capture.
    glib::timeout_future(Duration::from_millis(500)).await;
    let generation = ui.state.borrow().socket_generation;
    ui.event(Event::Wire {
        backend: "codex".into(),
        generation,
        payload: json!({"method":"studio/appServer/status","params":{"state":"ready"}}),
        prepared: Default::default(),
    });
    if ui.state.borrow().ready || !ui.state.borrow().backend_ready.is_empty() {
        return Err("Late ready restored a retired gateway".into());
    }
    ui.event(Event::Terminal {
        generation: 0,
        update: crate::terminal::Update::Ready {
            shell: "retired shell".into(),
            cwd: root.display().to_string(),
        },
    });
    if ui.terminal.smoke_ready() {
        return Err("Late terminal readiness revived input".into());
    }
    // A transport retry can already have cleared the catalog when its owning
    // gateway exits. Notification-style navigation must preserve that state too.
    let before = ui.state.borrow().selected.as_ref().map(|t| t.id.clone());
    let other = model::Thread::from_value(&json!({"id":"retired-other-session","name":"Must not open","cwd":root.join("project").display().to_string()})).ok_or("Fixture session invalid")?;
    ui.select(other);
    if ui.state.borrow().selected.as_ref().map(|t| t.id.clone()) != before
        || ui.composer_text() != draft
    {
        return Err("Session switching moved the offline draft".into());
    }
    crate::window_capture::capture(&ui.window, &root.join("gateway-stopped.png")).await?;
    if mode != "runtime-stop" {
        let conflict = mode == "runtime-conflict";
        if conflict {
            let path = root.join("project/discovery.md");
            gio::spawn_blocking(move || {
                std::fs::write(
                    path,
                    "# Changed by another application\n\nExternal change 中文😀\n",
                )
            })
            .await
            .map_err(|_| "Conflict writer stopped")?
            .map_err(|e| e.to_string())?;
        }
        if mode == "runtime-viewers-conflict" {
            let project = root.join("project");
            gio::spawn_blocking(move || {
                use std::io::Write;
                for (name, bytes) in [
                    if project.join("recovery.xlsx").is_file() {
                        ("recovery.xlsx", &b"\nExternal workbook edit\n"[..])
                    } else {
                        ("recovery.csv", &b"150,Changed,External edit\n"[..])
                    },
                    ("recovery.pdf", &b"\n% External edit\n"[..]),
                    ("recovery.epub", &b"\nExternal edit\n"[..]),
                ] {
                    std::fs::OpenOptions::new()
                        .append(true)
                        .open(project.join(name))?
                        .write_all(bytes)?;
                }
                Ok::<(), std::io::Error>(())
            })
            .await
            .map_err(|_| "Viewer conflict writer stopped")?
            .map_err(|e| e.to_string())?;
        }
        flag(root, "stop-gateway", false).await?;
        if viewers.is_some() {
            ui.epub.smoke_search_reconnect()?;
        } else {
            gio::prelude::ActionGroupExt::activate_action(&ui.window, "reconnect", None);
        }
        until(ui, "replacement verified history", || {
            let s = ui.state.borrow();
            !ui.gateway_stopped.get() && s.ready && s.history_verified && !s.loading
        })
        .await?;
        if ui.composer_text() != draft {
            return Err("Gateway replacement changed the unsent draft".into());
        }
        if !ui
            .state
            .borrow()
            .queues
            .entries
            .get("codex:target")
            .is_some_and(|q| q.paused && q.messages.len() == 1)
        {
            return Err("Gateway replacement unpaused the queue".into());
        }
        let recovered = ui.reader.smoke_gateway_resumed(conflict).await?;
        let viewer_results = if let Some(before) = viewers {
            if ui.tools.visible_child_name().as_deref() != Some("document") {
                return Err("Background viewer recovery stole document focus".into());
            }
            let changed = mode == "runtime-viewers-conflict";
            ui.show_tool("table");
            let table = ui
                .tables
                .smoke_after_recovery(&before["table"], changed)
                .await?;
            crate::window_capture::capture_widget(
                &ui.window,
                &ui.tables.widget,
                &root.join("table-recovered.png"),
            )
            .await?;
            ui.show_tool("pdf");
            let pdf = ui.pdf.smoke_after_recovery(&before["pdf"], changed).await?;
            crate::window_capture::capture_widget(
                &ui.window,
                &ui.pdf.widget,
                &root.join("pdf-recovered.png"),
            )
            .await?;
            ui.show_tool("epub");
            let mut epub = ui
                .epub
                .smoke_after_recovery(&before["epub"], changed)
                .await?;
            crate::window_capture::capture_widget(
                &ui.window,
                &ui.epub.widget,
                &root.join("epub-recovered.png"),
            )
            .await?;
            epub["bookSearch"] = ui
                .epub
                .smoke_search_recovered(&before["epub"], changed, root)
                .await?;
            ui.show_tool("document");
            Some(json!({"table":table,"pdf":pdf,"epub":epub,"backgroundFocusRetained":true}))
        } else {
            None
        };
        glib::timeout_future(Duration::from_millis(500)).await;
        crate::window_capture::capture(&ui.window, &root.join("gateway-recovered.png")).await?;
        ui.reader.smoke_discard_gateway_loss_edits();
        return Ok(
            json!({"ok":true,"ownedGatewayExit":true,"draftRetained":true,"queuePaused":true,"backendRetriesStopped":true,"lateReadyRejected":true,"lateTerminalRejected":true,"sessionSwitchGuarded":true,"editor":editor,"admittedExportCloseGuard":true,"admittedExportCompleted":true,"replacementRecovered":true,"recoveredEditor":recovered,"viewers":viewer_results}),
        );
    }
    ui.reader.smoke_discard_gateway_loss_edits();
    Ok(
        json!({"ok":true,"ownedGatewayExit":true,"draftRetained":true,"queuePaused":true,"backendRetriesStopped":true,"lateReadyRejected":true,"lateTerminalRejected":true,"sessionSwitchGuarded":true,"editor":editor,"admittedExportCloseGuard":true,"admittedExportCompleted":true,"sourceUnchanged":true}),
    )
}
