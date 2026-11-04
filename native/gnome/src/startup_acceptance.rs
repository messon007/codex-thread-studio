//! Startup HTTP faults through an owned gateway proxy; no model prompts.
use super::*;
use std::path::Path;
async fn until(ui: &Ui, label: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !test() {
        if start.elapsed() > Duration::from_secs(20) {
            return Err(format!("Startup discovery {label}: {}", ui.status.text()));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
async fn traces(root: &Path) -> Result<Vec<Value>, String> {
    let path = root.join("discovery-requests.jsonl");
    gio::spawn_blocking(move || {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        text.lines()
            .map(|s| serde_json::from_str(s).map_err(|e| e.to_string()))
            .collect()
    })
    .await
    .map_err(|_| "Discovery fixture reader stopped")?
}
async fn allow(root: &Path) -> Result<(), String> {
    let path = root.join("discovery-fault");
    gio::spawn_blocking(move || std::fs::remove_file(path).map_err(|e| e.to_string()))
        .await
        .map_err(|_| "Discovery fixture writer stopped")?
}
pub(super) async fn run(ui: &Rc<Ui>, root: &Path) -> Result<Value, String> {
    let mode = std::env::var("STUDIO_GNOME_SMOKE_DISCOVERY").map_err(|e| e.to_string())?;
    if mode.starts_with("runtime-") {
        return super::gateway_stop_acceptance::run(ui, root).await;
    }
    if mode.starts_with("launch-") {
        return launch(ui, root, &mode).await;
    }
    let waiting = if mode == "permanent" {
        "Backend discovery failed."
    } else {
        "Loading backends · Retrying"
    };
    until(ui, "visible startup notice", || {
        ui.status.text().starts_with(waiting)
    })
    .await?;
    let draft = "Unsent before discovery 中文😀 <literal>";
    ui.composer.buffer().set_text(draft);
    if ui.send.is_sensitive() || ui.state.borrow().startup_ready {
        return Err("Startup failure enabled dispatch".into());
    }
    crate::window_capture::capture(&ui.window, &root.join("discovery-waiting.png")).await?;
    if mode == "close" {
        let path = root.join("discovery-closing");
        gio::spawn_blocking(move || std::fs::write(path, b"1"))
            .await
            .map_err(|_| "Closing fixture writer stopped")?
            .map_err(|e| e.to_string())?;
        return Ok(json!({"ok":true,"closeDuringBackoff":true,"draft":draft}));
    }
    let rejected;
    if mode == "permanent" {
        glib::timeout_future(Duration::from_millis(700)).await;
        rejected = traces(root).await?;
        if rejected.len() != 1 || rejected[0]["status"] != 401 {
            return Err("Permanent discovery rejection retried automatically".into());
        }
        // The document actor must remain usable after discovery fails.
        ui.open_local_file(&root.join("project/discovery.md"));
        until(ui, "document lane after failed discovery", || {
            ui.reader.is_ready()
        })
        .await?;
        if ui.state.borrow().startup_ready || ui.composer_text() != draft {
            return Err("Opening a document changed startup or the unsent draft".into());
        }
        crate::window_capture::capture(&ui.window, &root.join("discovery-document.png")).await?;
        allow(root).await?;
        gio::prelude::ActionGroupExt::activate_action(&ui.window, "reconnect", None);
        // A queued callback from the old attempt must not initialize controls.
        ui.event(Event::Startup {
            generation: 0,
            update: crate::startup::Update::Loaded(Box::new(Event::Backends(Vec::new()))),
        });
        if ui.state.borrow().startup_ready || ui.state.borrow().startup_generation != 1 {
            return Err("Expired discovery callback initialized the catalog".into());
        }
    } else {
        let start = std::time::Instant::now();
        loop {
            let rows = traces(root).await?;
            if rows.len() >= 2 {
                rejected = rows;
                break;
            }
            if start.elapsed() > Duration::from_secs(8) {
                return Err("Automatic discovery retry did not issue another GET".into());
            }
            glib::timeout_future(Duration::from_millis(20)).await;
        }
        allow(root).await?;
    }
    until(ui, "verified selected history", || {
        let s = ui.state.borrow();
        s.startup_ready
            && s.ready
            && !s.loading
            && s.history_verified
            && s.selected.as_ref().is_some_and(|t| t.id == "target")
            && ui.transcript.n_items() > 0
            && s.items
                .iter()
                .any(|item| item.body.contains("Existing conversation history"))
    })
    .await?;
    if ui.composer_text() != draft {
        return Err("Discovery recovery lost the draft".into());
    }
    crate::window_capture::capture(&ui.window, &root.join("discovery-recovered.png")).await?;
    Ok(
        json!({"ok":true,"mode":mode,"automaticRetry":mode=="automatic","explicitReconnect":mode=="permanent","permanentRejectionNotRetried":mode=="permanent","documentLaneAlive":mode=="permanent","staleGenerationRejected":mode=="permanent","rejected":rejected,"draftRetained":true,"historyVerified":true,"historyRendered":true,"noPromptReplay":true}),
    )
}

async fn launch(ui: &Rc<Ui>, root: &Path, mode: &str) -> Result<Value, String> {
    let waiting = if matches!(mode, "launch-retry" | "launch-timeout") {
        "Gateway could not start."
    } else {
        "Starting gateway…"
    };
    until(ui, "gateway launch notice", || {
        ui.status.text().starts_with(waiting)
    })
    .await?;
    let draft = "Unsent before gateway launch 中文😀 <literal>";
    ui.composer.buffer().set_text(draft);
    let path = root.join("launch-attempts.jsonl");
    let start = std::time::Instant::now();
    loop {
        let p = path.clone();
        let ready = gio::spawn_blocking(move || p.is_file())
            .await
            .map_err(|_| "Launch trace reader stopped")?;
        if ready {
            break;
        }
        if start.elapsed() > Duration::from_secs(5) {
            return Err("Owned launch child did not start".into());
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    if ui.send.is_sensitive() || ui.state.borrow().startup_ready {
        return Err("Launch failure enabled Send".into());
    }
    crate::window_capture::capture(&ui.window, &root.join("discovery-waiting.png")).await?;
    if mode == "launch-close" {
        // Closing must not depend on a slot in the ordinary request queue.
        let mut full = false;
        for i in 0..65 {
            let command = Command::Http {
                tag: format!("queued-before-launch-{i}"),
                method: reqwest::Method::GET,
                path: "/studio/native/info".into(),
                body: None,
                document: None,
            };
            if let Err(error) = ui.sender.try_send(command) {
                full = matches!(error, tokio::sync::mpsc::error::TrySendError::Full(_));
                break;
            }
        }
        if !full {
            return Err("Launch close fixture did not fill the request queue".into());
        }
        let path = root.join("discovery-closing");
        gio::spawn_blocking(move || std::fs::write(path, b"1"))
            .await
            .map_err(|_| "Launch closing writer stopped")?
            .map_err(|e| e.to_string())?;
        return Ok(
            json!({"ok":true,"closeDuringLaunch":true,"requestQueueFull":true,"draft":draft}),
        );
    }
    let path = root.join("launch-fault");
    gio::spawn_blocking(move || std::fs::remove_file(path))
        .await
        .map_err(|_| "Launch gate writer stopped")?
        .map_err(|e| e.to_string())?;
    gio::prelude::ActionGroupExt::activate_action(&ui.window, "reconnect", None);
    ui.event(Event::Startup {
        generation: 0,
        update: crate::startup::Update::Loaded(Box::new(Event::Backends(Vec::new()))),
    });
    if ui.state.borrow().startup_ready || ui.state.borrow().startup_generation != 1 {
        return Err("Old process attempt initialized the UI".into());
    }
    until(ui, "history after gateway retry", || {
        let s = ui.state.borrow();
        s.startup_ready
            && s.ready
            && !s.loading
            && s.history_verified
            && s.selected.as_ref().is_some_and(|t| t.id == "target")
            && ui.transcript.n_items() > 0
            && s.items
                .iter()
                .any(|item| item.body.contains("Existing conversation history"))
    })
    .await?;
    if ui.composer_text() != draft {
        return Err("Gateway launch retry lost the draft".into());
    }
    crate::window_capture::capture(&ui.window, &root.join("discovery-recovered.png")).await?;
    Ok(
        json!({"ok":true,"mode":mode,"explicitLaunchRetry":true,"replacedPendingLaunch":mode=="launch-replace","launchTimeoutRecovered":mode=="launch-timeout","draftRetained":true,"historyVerified":true,"historyRendered":true,"staleGenerationRejected":true}),
    )
}
