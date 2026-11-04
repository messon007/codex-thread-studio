use super::*;
use crate::acceptance_wait::until;
impl Pdf {
    pub(crate) async fn smoke_before_recovery(self: &Rc<Self>) -> Result<Value, String> {
        if std::env::var("STUDIO_GNOME_SMOKE_PDF_RECOVERY").as_deref() == Ok("encrypted") {
            self.smoke_initial_unlock().await?;
        }
        until("PDF open", || {
            self.state.borrow().pages == 3 && !self.state.borrow().busy
        })
        .await
        .map_err(|e| format!("{e}: {}", self.status.text()))?;
        self.load(1);
        until("PDF second page", || {
            self.state.borrow().page == 1 && !self.state.borrow().busy
        })
        .await?;
        self.fit.set_active(false);
        self.zoom.set(1.35);
        self.layout();
        self.preview.set_active(true);
        self.search_bar.set_text("Needle");
        self.case.set_active(true);
        // GtkSearchEntry emits its delayed search after the case toggle's
        // immediate request. Let that real signal settle before hiding results.
        glib::timeout_future(std::time::Duration::from_millis(350)).await;
        until("PDF scroll range", || {
            self.canvas.vadjustment().upper() > self.canvas.vadjustment().page_size() + 40.0
        })
        .await?;
        self.canvas.vadjustment().set_value(40.0);
        until("PDF search", || {
            self.state.borrow().query == "Needle" && self.more.is_sensitive()
        })
        .await?;
        self.sidebar.set_reveal_child(false);
        let state = self.state.borrow();
        Ok(
            json!({"id":state.id,"generation":state.generation,"serial":state.view,"page":state.page,"scroll":self.canvas.vadjustment().value()}),
        )
    }
    pub(crate) async fn smoke_after_recovery(
        self: &Rc<Self>,
        before: &Value,
        conflict: bool,
    ) -> Result<Value, String> {
        let unlock =
            if std::env::var("STUDIO_GNOME_SMOKE_PDF_RECOVERY").as_deref() == Ok("encrypted") {
                Some(self.smoke_recovery_unlock().await?)
            } else {
                None
            };
        until("PDF recovery", || {
            let state = self.state.borrow();
            !state.busy
                && if conflict {
                    state.recovering && state.id.is_none()
                } else {
                    !state.recovering && state.id.is_some()
                }
        })
        .await
        .map_err(|e| {
            format!(
                "{e}: {} (busy={}, recovering={}, id={})",
                self.status.text(),
                self.state.borrow().busy,
                self.state.borrow().recovering,
                self.state.borrow().id.is_some()
            )
        })?;
        if self.state.borrow().page != 1
            || self.fit.is_active()
            || (self.zoom.get() - 1.35).abs() > 0.001
            || !self.preview.is_active()
            || self.search_bar.text() != "Needle"
            || !self.case.is_active()
            || self.sidebar.reveals_child()
        {
            return Err(format!("PDF recovery changed state: page={}, fit={}, zoom={}, preview={}, query={:?}, case={}, sidebar={}",self.state.borrow().page,self.fit.is_active(),self.zoom.get(),self.preview.is_active(),self.search_bar.text(),self.case.is_active(),self.sidebar.reveals_child()));
        }
        if conflict {
            if !self.status.text().contains("changed") || self.next.is_sensitive() {
                return Err("PDF conflict left live controls".into());
            }
        } else {
            if self.state.borrow().id.as_deref() == before["id"].as_str() {
                return Err("PDF reused a departed lease".into());
            }
            if (self.canvas.vadjustment().value() - before["scroll"].as_f64().unwrap()).abs() > 1.0
            {
                return Err(format!(
                    "PDF scroll changed: {} -> {}",
                    before["scroll"],
                    self.canvas.vadjustment().value()
                ));
            }
        }
        self.event(Response {
            generation: before["generation"].as_u64().unwrap(),
            serial: before["serial"].as_u64().unwrap(),
            kind: "page",
            result: Err("STALE PDF REPLY".into()),
        });
        if self.status.text().contains("STALE") {
            return Err("Retired PDF reply reached the new view".into());
        }
        Ok(
            json!({"snapshotRevalidated":!conflict,"conflictProtected":conflict,"pageRetained":true,"zoomRetained":true,"modeRetained":true,"searchRetained":true,"scrollRetained":true,"staleReplyRejected":true,"encryptedUnlock":unlock}),
        )
    }
}
