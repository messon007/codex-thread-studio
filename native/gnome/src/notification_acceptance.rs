//! Private Wayland fixture; actual GIO transport is captured on a private session bus.
use super::*;
use std::path::Path;
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !test() {
        if start.elapsed() > Duration::from_secs(12) {
            return Err(format!("Notification acceptance timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(25)).await;
    }
    Ok(())
}
async fn file(path: std::path::PathBuf, value: Vec<u8>) -> Result<(), String> {
    gio::spawn_blocking(move || std::fs::write(path, value).map_err(|e| e.to_string()))
        .await
        .map_err(|_| "Notification fixture worker stopped")?
}
fn wire(ui: &Rc<Ui>, method: &str, params: Value, stale: bool) {
    let backend = ui.state.borrow().backend.clone();
    let generation = ui.state.borrow().socket_generation;
    ui.event(Event::Wire {
        backend,
        generation: if stale {
            generation.saturating_sub(1)
        } else {
            generation
        },
        payload: json!({"method":method,"params":params}),
        prepared: protocol::Prepared::default(),
    });
}
fn completion(ui: &Rc<Ui>, thread: &str, turn: &str) {
    wire(
        ui,
        "turn/started",
        json!({"threadId":thread,"turn":{"id":turn}}),
        false,
    );
    wire(
        ui,
        "turn/completed",
        json!({"threadId":thread,"turn":{"id":turn,"status":"completed"}}),
        false,
    );
}
async fn background(ui: &Rc<Ui>) -> Result<gtk::Window, String> {
    let other = gtk::Window::builder()
        .title("Private notification focus fixture")
        .default_width(220)
        .default_height(90)
        .child(&gtk::Label::new(Some("Notification fixture")))
        .build();
    other.present();
    until("background window", || !ui.notifications.foreground()).await?;
    Ok(other)
}
#[allow(deprecated)] // Exercise the existing approval dialog, including its response.
pub async fn run(ui: &Rc<Ui>, root: &Path, restore: bool) -> Result<Value, String> {
    until("preference and catalog", || {
        ui.notifications.loaded()
            && ui.state.borrow().ready
            && !ui.state.borrow().threads.is_empty()
    })
    .await?;
    if restore {
        until("D-Bus activation", || {
            ui.notifications.activations.get() > 0
        })
        .await?;
        if !ui.notifications.enabled() {
            return Err("Saved notification preference did not survive activation".into());
        }
        ui.open_settings();
        let settings = ui.settings.borrow().clone().unwrap();
        let controls = settings.smoke_notifications();
        controls.row.set_active(false);
        until("disable restored alerts", || !ui.notifications.busy()).await?;
        if ui.notifications.enabled() {
            return Err("Restored notifications did not disable".into());
        }
        crate::window_capture::capture_dialog(
            &ui.window,
            &settings.dialog,
            &root.join(if restore {
                "notifications-restore.png"
            } else {
                "notifications-emit.png"
            }),
        )
        .await?;
        settings.dialog.close();
        ui.notifications.flush();
        return Ok(
            json!({"ok":true,"activatedAfterExit":true,"restoredPreference":true,"disabledAfterRestart":true}),
        );
    }
    if ui.notifications.enabled() {
        return Err("Notifications must begin opt-out".into());
    }
    let other = background(ui).await?;
    completion(ui, "target", "disabled");
    ui.open_settings();
    let settings = ui.settings.borrow().clone().unwrap();
    let controls = settings.smoke_notifications();
    controls.row.set_active(true);
    if settings.may_close() || controls.row.is_sensitive() {
        return Err("Notification save lost its busy guard".into());
    }
    until("enable", || !ui.notifications.busy()).await?;
    if !ui.notifications.enabled() {
        return Err(format!("Enable failed: {}", controls.error_text()));
    }
    crate::window_capture::capture_dialog(
        &ui.window,
        &settings.dialog,
        &root.join(if restore {
            "notifications-restore.png"
        } else {
            "notifications-emit.png"
        }),
    )
    .await?;
    settings.dialog.close();
    other.close();
    let other = background(ui).await?;
    wire(
        ui,
        "turn/completed",
        json!({"threadId":"target","turn":{"id":"disabled"}}),
        false,
    );
    wire(ui, "turn/completed", json!({"threadId":"idle-only"}), false);
    completion(ui, "target", "notify-once");
    wire(
        ui,
        "turn/completed",
        json!({"threadId":"target","turn":{"id":"notify-once"}}),
        false,
    );
    wire(
        ui,
        "turn/completed",
        json!({"threadId":"target","turn":{"id":"stale"}}),
        true,
    );
    wire(
        ui,
        "turn/started",
        json!({"threadId":"idle-style","turn":{"id":"opencode-style"}}),
        false,
    );
    wire(
        ui,
        "turn/completed",
        json!({"threadId":"idle-style"}),
        false,
    );
    wire(
        ui,
        "turn/completed",
        json!({"threadId":"idle-style"}),
        false,
    );
    until("Router alert registry", || {
        ui.router.borrow().registry.is_some()
    })
    .await?;
    let router_notices = ui.smoke_router_notices()?;
    other.close();
    ui.window.present();
    // GNOME may deny unattended activation without a real input serial. Do not
    // override focus or claim that physical focus acceptance ran in that case.
    glib::timeout_future(Duration::from_millis(250)).await;
    let foreground_observed = ui.notifications.foreground();
    if foreground_observed {
        completion(ui, "target", "foreground-suppressed");
    }
    let other = background(ui).await?;
    let payload = json!({"id":"notice-approval","method":"item/commandExecution/requestApproval","params":{"threadId":"target","command":"private test details"}});
    let backend = ui.state.borrow().backend.clone();
    let generation = ui.state.borrow().socket_generation;
    for _ in 0..2 {
        let mut payload = payload.clone();
        let prepared = protocol::prepare_presentation(&mut payload);
        ui.event(Event::Wire {
            backend: backend.clone(),
            generation,
            payload,
            prepared,
        });
    }
    let dialogs = gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|w| w.downcast::<adw::MessageDialog>().ok())
        .filter(|d| d.heading().as_deref() == Some("Agent needs your input"))
        .collect::<Vec<_>>();
    if dialogs.len() != 1 {
        return Err("Replayed request created duplicate approval dialogs".into());
    }
    file(
        root.join("activate-notification.flag"),
        b"activate".to_vec(),
    )
    .await?;
    until("click activation", || {
        ui.notifications.activations.get() > 0
    })
    .await?;
    if !dialogs[0].is_visible() {
        return Err("Notification activation resolved an approval".into());
    }
    dialogs[0].response("deny");
    other.close();
    ui.open_settings();
    let settings = ui.settings.borrow().clone().unwrap();
    let controls = settings.smoke_notifications();
    controls.row.set_active(false);
    until("disable", || !ui.notifications.busy()).await?;
    if ui.notifications.enabled() {
        return Err("Notifications did not disable".into());
    }
    completion(ui, "target", "disabled-again");
    let path = root.join("config/codex-thread-studio/settings.json");
    gio::spawn_blocking(move || -> Result<(), String> {
        let mut raw: Value =
            serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        raw["desktopNotifications"] = json!(true);
        raw["futureNotification"] = json!({"preserve":true});
        std::fs::write(path, serde_json::to_vec(&raw).unwrap()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "Notification conflict worker stopped")??;
    controls.row.set_active(true);
    until("conflict", || !ui.notifications.busy()).await?;
    if ui.notifications.enabled()
        || controls.row.is_active()
        || !controls.error_text().contains("Reload")
    {
        return Err("Conflicting notification change did not roll back".into());
    }
    controls.refresh();
    until("reload", || !ui.notifications.busy()).await?;
    if !ui.notifications.enabled() {
        return Err("Reload did not apply the other client's setting".into());
    }
    settings.dialog.close();
    let other = background(ui).await?;
    completion(ui, "persistent", "persisted");
    // A startup ledger can finish loading after live events have issued an
    // alert. Preserve the live ID and bound the merged historical list to 32.
    let historical = (0..32)
        .map(|i| crate::notification_ids::tag("codex", &format!("historical-{i}")).unwrap())
        .collect::<Vec<_>>();
    ui.notifications.restore_ids(&json!(historical));
    ui.notifications.flush();
    other.close();
    Ok(
        json!({"ok":true,"optIn":true,"foregroundObserved":foreground_observed,"physicalFocusAcceptance":"manual","completionDedup":true,"idleDedup":true,"staleIgnored":true,"approvalDedup":true,"activationDidNotApprove":true,"conflictRollback":true,"ledgerFlushed":true,"router":router_notices}),
    )
}
