//! Real WebKit pages + gateway metadata, with isolated profiles only.
use super::*;
async fn wait(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !test() {
        if start.elapsed() > std::time::Duration::from_secs(25) {
            return Err(format!("Browser restoration timed out: {step}"));
        }
        glib::timeout_future(std::time::Duration::from_millis(25)).await;
    }
    Ok(())
}
impl Browser {
    pub async fn smoke_tab_save_failure(
        self: &Rc<Self>,
        root: &std::path::Path,
    ) -> Result<(), String> {
        fn button(widget: &gtk::Widget, label: &str) -> Option<gtk::Button> {
            if let Some(b) = widget
                .downcast_ref::<gtk::Button>()
                .filter(|b| b.label().as_deref() == Some(label))
            {
                return Some(b.clone());
            }
            let mut child = widget.first_child();
            while let Some(w) = child {
                child = w.next_sibling();
                if let Some(b) = button(&w, label) {
                    return Some(b);
                }
            }
            None
        }
        let parent = self.parent.upgrade().ok_or("Missing browser window")?;
        // Two loaded pages, stable revision, and no pending save.
        let path = root.join("config/codex-thread-studio/gnome-browser-tabs.json");
        gio::spawn_blocking(move || -> Result<(), String> {
            let mut raw: Value =
                serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            raw["nonce"] = serde_json::json!("private-fixture-foreign-revision");
            std::fs::write(&path, serde_json::to_vec(&raw).unwrap()).map_err(|e| e.to_string())
        })
        .await
        .map_err(|_| "Private revision worker stopped")??;
        self.tabs.set_selected_page(&self.tabs.nth_page(0));
        wait("foreign revision rejected", || self.tab_error.get()).await?;
        // Simulate already-admitted cleanup finishing after Cancel; neither the
        // metadata nor file lane may re-close the cancelled window.
        let release = Arc::new(AtomicBool::new(false));
        let worker = release.clone();
        self.file_io.run(
            move || {
                let start = std::time::Instant::now();
                while !worker.load(Ordering::Acquire) {
                    if start.elapsed() > std::time::Duration::from_secs(20) {
                        return Err("Private cleanup release timed out".into());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Ok(())
            },
            |_| {},
        );
        // Arm the file guard as if it was admitted before the save failed.
        if self.file_io.allow_close() {
            return Err("Missing cleanup close guard".into());
        }
        parent.close();
        wait("save failure dialog", || {
            self.tab_alert
                .borrow()
                .as_ref()
                .is_some_and(|a| a.is_mapped())
        })
        .await?;
        let dialog = self.tab_alert.borrow().clone().unwrap();
        button(dialog.upcast_ref(), &crate::language::tr("Cancel"))
            .ok_or("Missing save failure Cancel button")?
            .emit_clicked();
        wait("save failure cancelled", || {
            self.tab_alert.borrow().is_none() && !self.tab_closing.get()
        })
        .await?;
        release.store(true, Ordering::Release);
        wait("cleanup settled", || self.file_io.idle()).await?;
        glib::timeout_future(std::time::Duration::from_millis(250)).await;
        if !parent.is_mapped()
            || self.tab_alert.borrow().is_some()
            || self.engine.borrow().is_none()
            || self.tabs.n_pages() != 2
        {
            return Err("Cancelled close resumed after cleanup".into());
        }
        self.open(None);
        wait("explicit reopen recovered save", || {
            self.engine.borrow().is_some()
                && !self.loading_preferences.get()
                && self.tab_task.get().is_none()
                && self.tab_pending.borrow().is_none()
        })
        .await?;
        if self.tab_error.get() {
            return Err("Reopening did not recover the tab save".into());
        }
        Ok(())
    }
    pub async fn smoke_prepare_tab_shutdown(self: &Rc<Self>, url: &str) -> Result<(), String> {
        self.exit();
        self.open(Some(url));
        wait("shutdown first page", || {
            self.active()
                .is_some_and(|w| !w.is_loading() && w.title().is_some())
        })
        .await?;
        self.add_tab(Some(url.trim_end_matches("/one").to_owned() + "/two"));
        wait("shutdown second page", || {
            self.active()
                .is_some_and(|w| !w.is_loading() && w.title().is_some())
                && self.tab_task.get().is_none()
                && self.tab_pending.borrow().is_none()
        })
        .await?;
        self.active()
            .unwrap()
            .evaluate_javascript_future(
                "localStorage.setItem('restore-private-test','private')",
                Some(WORLD),
                None,
            )
            .await
            .map_err(|e| e.to_string())?;
        self.tabs.set_selected_page(&self.tabs.nth_page(0));
        // Queue one save and replace its pending selection before delivering any
        // GTK completion. Close must flush the latest selection, not the first.
        self.tabs.set_selected_page(&self.tabs.nth_page(1));
        if self.tab_task.get().is_none() {
            return Err("Missing pending tab save at shutdown".into());
        }
        Ok(())
    }
    pub async fn smoke_tab_restart(self: &Rc<Self>, url: &str) -> Result<Value, String> {
        if self.engine.borrow().is_some() || self.tabs.n_pages() != 0 {
            return Err("Restoring tabs eagerly started a content engine".into());
        }
        self.open(None);
        let two = url.trim_end_matches("/one").to_owned() + "/two";
        wait("real application restart", || {
            self.tabs.n_pages() == 2
                && !self.loading_preferences.get()
                && (0..2).all(|i| {
                    self.tabs
                        .nth_page(i)
                        .child()
                        .downcast::<webkit6::WebView>()
                        .is_ok_and(|w| !w.is_loading() && w.title().is_some())
                })
        })
        .await?;
        let expected = crate::browser_tabs::Tabs {
            tabs: vec![url.to_owned(), two],
            active: 1,
        };
        if self.current_tabs() != expected {
            return Err(format!(
                "Restart lost latest tab selection: {:?}",
                self.current_tabs()
            ));
        }
        let storage = self
            .active()
            .unwrap()
            .evaluate_javascript_future(
                "localStorage.getItem('restore-private-test')===null",
                Some(WORLD),
                None,
            )
            .await
            .map_err(|e| e.to_string())?;
        if !storage.to_boolean() {
            return Err("Restoring addresses persisted private page storage".into());
        }
        Ok(
            serde_json::json!({"ok":true,"realRestart":true,"latestSelection":true,"lazy":true,"privateStorageCleared":true}),
        )
    }
    pub async fn smoke_restoration(
        self: &Rc<Self>,
        panel: &Rc<crate::browser_settings_ui::Panel>,
        url: &str,
    ) -> Result<Value, String> {
        self.exit();
        panel.smoke_restore(true).await?;
        if self.engine.borrow().is_some() {
            return Err("Restoration setting eagerly started Browser".into());
        }
        let two = url.trim_end_matches("/one").to_owned() + "/two";
        self.open(Some(url));
        wait("first page", || {
            self.active()
                .is_some_and(|w| !w.is_loading() && w.title().is_some())
        })
        .await?;
        self.add_tab(Some(two.clone()));
        wait("second page", || {
            self.active()
                .is_some_and(|w| !w.is_loading() && w.uri().as_deref() == Some(two.as_str()))
        })
        .await?;
        self.tabs.set_selected_page(&self.tabs.nth_page(0));
        wait("saved addresses", || {
            self.tab_task.get().is_none() && self.tab_pending.borrow().is_none()
        })
        .await?;
        let fragment = format!("{url}#restored-section");
        self.navigate(&fragment);
        wait("same-document address saved", || {
            self.active()
                .is_some_and(|w| w.uri().as_deref() == Some(fragment.as_str()))
                && self.tab_task.get().is_none()
                && self.tab_pending.borrow().is_none()
        })
        .await?;
        let before = self.current_tabs();
        if before.tabs != vec![fragment, two.clone()] || before.active != 0 {
            return Err(format!("Unexpected saved selection: {before:?}"));
        }
        self.exit();
        wait("exit save", || {
            self.tab_task.get().is_none() && self.tab_pending.borrow().is_none()
        })
        .await?;
        // Forget all native caches. Reopen must use the disk-backed gateway state.
        self.tab_last.borrow_mut().take();
        self.tab_revision.borrow_mut().take();
        self.open(Some("file:///private-unselected-browser-fixture"));
        wait("rejected explicit Open", || {
            !self.loading_preferences.get() && self.tab_task.get().is_none()
        })
        .await?;
        if self.engine.borrow().is_some() || self.tab_pending.borrow().is_some() {
            return Err("Rejected Open changed browser workspace state".into());
        }
        self.open(None);
        wait("restored tabs", || {
            self.tabs.n_pages() == 2
                && !self.loading_preferences.get()
                && (0..2).all(|i| {
                    self.tabs
                        .nth_page(i)
                        .child()
                        .downcast::<webkit6::WebView>()
                        .is_ok_and(|w| !w.is_loading() && w.title().is_some())
                })
        })
        .await?;
        if self.current_tabs() != before {
            return Err("Restored addresses or active tab differ".into());
        }
        self.tabs.reorder_page(&self.tabs.nth_page(0), 1);
        wait("tab order saved", || {
            self.tab_task.get().is_none() && self.tab_pending.borrow().is_none()
        })
        .await?;
        let reordered = self.current_tabs();
        self.exit();
        self.open(None);
        wait("reordered tabs restored", || {
            self.tabs.n_pages() == 2 && !self.loading_preferences.get()
        })
        .await?;
        if self.current_tabs() != reordered {
            return Err("Restored tab order differs".into());
        }
        self.tabs.reorder_page(&self.tabs.nth_page(0), 1);
        // Close a tab through the native widget.
        self.tabs.close_page(&self.tabs.nth_page(0));
        wait("individual close saved", || {
            self.tab_task.get().is_none() && self.tab_pending.borrow().is_none()
        })
        .await?;
        self.exit();
        self.open(None);
        wait("only surviving tab restored", || {
            self.tabs.n_pages() == 1
                && !self.loading_preferences.get()
                && self
                    .active()
                    .is_some_and(|w| !w.is_loading() && w.uri().as_deref() == Some(two.as_str()))
        })
        .await?;
        // Explicit URL opening is intentional navigation, never restores extra tabs.
        self.exit();
        self.open(Some(url));
        wait("explicit open", || {
            self.tabs.n_pages() == 1
                && !self.loading_preferences.get()
                && self
                    .active()
                    .is_some_and(|w| !w.is_loading() && w.uri().as_deref() == Some(url))
        })
        .await?;
        panel.smoke_restore(false).await?;
        wait("disabled save settled", || {
            self.tab_task.get().is_none() && self.tab_pending.borrow().is_none()
        })
        .await?;
        if self.tab_error.get() {
            return Err(format!(
                "Disabled restoration produced an error: {}",
                self.status.text()
            ));
        }
        self.exit();
        panel.smoke_restore(true).await?;
        self.open(None);
        wait("forgotten addresses", || {
            self.tabs.n_pages() == 1 && !self.loading_preferences.get()
        })
        .await?;
        if self
            .active()
            .unwrap()
            .uri()
            .is_some_and(|u| u.as_str() != "about:blank")
        {
            return Err("Off/on resurrected old addresses".into());
        }
        // Last-tab close saves an empty selection, unlike Exit which preserves it.
        self.navigate(url);
        wait("last page", || {
            self.active()
                .is_some_and(|w| !w.is_loading() && w.uri().as_deref() == Some(url))
        })
        .await?;
        self.tabs.close_page(&self.tabs.nth_page(0));
        wait("last close saved", || {
            self.tab_task.get().is_none() && self.tab_pending.borrow().is_none()
        })
        .await?;
        self.open(None);
        wait("empty after last close", || {
            self.tabs.n_pages() == 1 && !self.loading_preferences.get()
        })
        .await?;
        if self
            .active()
            .unwrap()
            .uri()
            .is_some_and(|u| u.as_str() != "about:blank")
        {
            return Err("Last closed tab was restored".into());
        }
        self.exit();
        panel.smoke_restore(false).await?;
        Ok(
            serde_json::json!({"lazy":true,"diskRestoration":true,"activeSelection":true,"sameDocumentAddress":true,"tabOrder":true,"individualClose":true,"explicitUrl":true,"rejectedOpenPreservesTabs":true,"forgetOffOn":true,"lastClose":true}),
        )
    }
}
