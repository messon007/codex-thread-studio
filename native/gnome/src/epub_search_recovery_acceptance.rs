//! Real dialog state retained across owned gateway replacement.
use super::*;
use serde_json::{json, Value};
impl Panel {
    fn recovery_snapshot(&self) -> Value {
        let rows=(0..self.results.n_items()).map(|i| {
            let row=self.results.item(i).and_downcast::<glib::BoxedAnyObject>().unwrap();
            let row=row.borrow::<Section>();
            json!({"chapter":row.chapter,"href":row.href,"snippet":row.snippet,"count":row.count})
        }).collect::<Vec<_>>();
        json!({"draft":self.query.text().as_str(),"draftCase":self.case.is_active(),"activeQuery":self.active_query.borrow().as_str(),"activeCase":self.active_case.get(),"page":self.page.get(),"cursor":self.cursor.get(),"searched":self.searched.get(),"rows":rows})
    }
    pub async fn smoke_recovery_prepare(&self) -> Result<Value, String> {
        self.smoke_wait().await?;
        self.query.set_text("Needle");
        self.case.set_active(false);
        self.search.emit_clicked();
        let kind = std::env::var("STUDIO_GNOME_SMOKE_BOOK_SEARCH").unwrap_or_default();
        if kind == "inflight" {
            crate::acceptance_wait::until("in-flight book search partial page", || {
                self.results.n_items() == 64
                    && self.cursor.get() == Some(64)
                    && self.serial.get().is_some()
            })
            .await?;
            let marker = std::path::PathBuf::from(
                std::env::var_os("STUDIO_GNOME_SMOKE_DIR").ok_or("Search fixture root missing")?,
            )
            .join("epub-search-held.json");
            let at = std::time::Instant::now();
            loop {
                let path = marker.clone();
                if gtk::gio::spawn_blocking(move || path.is_file())
                    .await
                    .map_err(|_| "Held search marker worker stopped")?
                {
                    break;
                }
                if at.elapsed() > std::time::Duration::from_secs(10) {
                    return Err("Search response was not held by the owned proxy".into());
                }
                glib::timeout_future(std::time::Duration::from_millis(20)).await;
            }
        } else {
            self.smoke_wait().await?;
            if kind == "paged" {
                self.smoke_rows(0, 112, 0, Some(112))?;
                self.next.emit_clicked();
                self.smoke_wait().await?;
                self.smoke_rows(112, 140, 1, None)?;
            }
        }
        if kind != "inflight"
            && (self.results.n_items() != if self.total == 140 { 28 } else { 3 }
                || self.cursor.get().is_some())
        {
            return Err(format!(
                "Recovery search did not find all chapters: {}",
                self.status.text()
            ));
        }
        self.query.set_text("Unsent literal search 中文😀 ");
        self.case.set_active(true);
        Ok(self.recovery_snapshot())
    }
    pub fn smoke_recovery_offline(&self, before: &Value) -> Result<(), String> {
        if self.online()
            || self.serial.get().is_some()
            || self.search.is_sensitive()
            || self.next.is_sensitive()
            || self.previous.is_sensitive()
            || self.view.is_sensitive()
            || self.stop.is_sensitive()
            || !self.reconnect.is_visible()
        {
            return Err("Offline book-search controls retained a live request or action".into());
        }
        if self.recovery_snapshot() != *before {
            return Err("Gateway loss changed book-search draft or results".into());
        }
        self.search.emit_clicked();
        self.next.emit_clicked();
        self.previous.emit_clicked();
        self.view.emit_by_name::<()>("activate", &[&2u32]);
        if self.recovery_snapshot() != *before || !self.alive.get() {
            return Err("Offline book search dispatched or navigated retained results".into());
        }
        Ok(())
    }
    pub fn smoke_reconnect(&self) -> Result<(), String> {
        if !self.reconnect.is_mapped()
            || !self.reconnect.is_sensitive()
            || self.reconnect.action_name().as_deref() != Some("win.reconnect")
        {
            return Err("Book-search Reconnect action is not available in the modal dialog".into());
        }
        self.reconnect.emit_clicked();
        Ok(())
    }
    pub fn smoke_recovery_retained(&self, before: &Value, conflict: bool) -> Result<Value, String> {
        if self.recovery_snapshot() != *before
            || self.serial.get().is_some()
            || self.online() == conflict
            || self.reconnect.is_visible()
            || self.search.is_sensitive() == conflict
            || self.view.is_sensitive() == conflict
        {
            return Err(format!(
                "Reconnected book search changed its snapshot or request controls: {}",
                self.status.text()
            ));
        }
        if conflict && !self.status.text().contains("changed") {
            return Err("Conflicting book search has no persistent diagnostic".into());
        }
        Ok(
            json!({"draftRetained":true,"resultsRetained":true,"noAutomaticSearch":true,"offlineActionsBlocked":true,"snapshotRevalidated":!conflict,"conflictProtected":conflict}),
        )
    }
    pub async fn smoke_retained_query(&self, before: &Value) -> Result<(), String> {
        self.search.emit_clicked();
        self.smoke_wait().await?;
        if self.results.n_items() != 0
            || !self.status.text().contains("Search complete")
            || self.searched.get() != self.total
            || self.active_query.borrow().as_str() != before["draft"].as_str().unwrap()
            || !self.active_case.get()
        {
            return Err(format!(
                "Retained search dialog did not query its recovered book: {}",
                self.status.text()
            ));
        }
        self.smoke_search_current_book().await
    }
    pub fn smoke_recovery_serial(&self) -> Option<u64> {
        self.serial.get()
    }
    fn smoke_rows(
        &self,
        start: usize,
        end: usize,
        page: usize,
        cursor: Option<usize>,
    ) -> Result<(), String> {
        let actual = (0..self.results.n_items())
            .map(|i| {
                self.results
                    .item(i)
                    .and_downcast::<glib::BoxedAnyObject>()
                    .unwrap()
                    .borrow::<Section>()
                    .chapter
            })
            .collect::<Vec<_>>();
        if actual != (start..end).collect::<Vec<_>>()
            || self.page.get() != page
            || self.cursor.get() != cursor
            || self.serial.get().is_some()
        {
            return Err(format!("Search pages skip/duplicate sections or change their boundary: {start}..{end}, page={}, cursor={:?}, actual={actual:?}",self.page.get(),self.cursor.get()));
        }
        Ok(())
    }
    pub async fn smoke_recovery_pages(&self, before: &Value) -> Result<Value, String> {
        let inflight = std::env::var("STUDIO_GNOME_SMOKE_BOOK_SEARCH").as_deref() == Ok("inflight");
        if inflight {
            self.next.emit_clicked();
        } else {
            self.previous.emit_clicked();
        }
        self.smoke_wait().await?;
        self.smoke_rows(0, 112, 0, Some(112))?;
        self.next.emit_clicked();
        self.smoke_wait().await?;
        self.smoke_rows(112, 140, 1, None)?;
        if inflight {
            self.previous.emit_clicked();
            self.smoke_wait().await?;
            self.smoke_rows(0, 112, 0, Some(112))?;
            self.next.emit_clicked();
            self.smoke_wait().await?;
            self.smoke_rows(112, 140, 1, None)?;
        }
        if self.query.text().as_str() != before["draft"].as_str().unwrap()
            || !self.case.is_active()
            || self.active_query.borrow().as_str() != "Needle"
            || self.active_case.get()
        {
            return Err("Paging submitted or changed the retained search draft".into());
        }
        Ok(
            json!({"firstPageRows":112,"lastPageRows":28,"noGapsOrDuplicates":true,"partialPageCompleted":inflight,"previousNext":true,"draftRetained":true}),
        )
    }
    pub fn smoke_activate_retained(&self) {
        self.view.emit_by_name::<()>("activate", &[&2u32]);
    }
    pub fn smoke_expired_search(&self) -> Result<(), String> {
        let before = self.recovery_snapshot();
        self.search.emit_clicked();
        self.view.emit_by_name::<()>("activate", &[&0u32]);
        if self.online()
            || self.recovery_snapshot() != before
            || self.serial.get().is_some()
            || self.reconnect.action_name().is_some()
        {
            return Err("Closed search dialog retained live callbacks".into());
        }
        Ok(())
    }
    pub async fn smoke_search_current_book(&self) -> Result<(), String> {
        self.query.set_text("Needle");
        self.case.set_active(false);
        self.search.emit_clicked();
        self.smoke_wait().await?;
        self.smoke_rows(
            0,
            if self.total == 140 { 112 } else { 3 },
            0,
            if self.total == 140 { Some(112) } else { None },
        )?;
        if !self.status.text().contains(if self.total == 140 {
            "More sections available"
        } else {
            "Search complete"
        }) {
            return Err(format!(
                "Search did not use the recovered book: {}",
                self.status.text()
            ));
        }
        Ok(())
    }
}
