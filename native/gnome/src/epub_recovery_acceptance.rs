use super::*;
use crate::acceptance_wait::until;
impl Epub {
    async fn smoke_content_value(&self, script: &str) -> Result<Value, String> {
        let web = self
            .surface
            .borrow()
            .clone()
            .ok_or("EPUB surface missing")?;
        let value = web
            .evaluate_javascript_future(&format!("JSON.stringify({script})"), Some(WORLD), None)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::from_str(&value.to_str()).map_err(|e| e.to_string())
    }
    async fn smoke_supplement_visible(&self) -> Result<Value, String> {
        let started = std::time::Instant::now();
        loop {
            let value = self.smoke_content_value("(()=>{const r=document.getElementById('s70')?.getBoundingClientRect();return {href:location.href,text:document.getElementById('s70')?.textContent,visible:!!r&&r.right>0&&r.left<innerWidth&&r.bottom>0&&r.top<innerHeight}})()").await?;
            if value["visible"] == true {
                return Ok(value);
            }
            if started.elapsed() > std::time::Duration::from_secs(15) {
                return Err(format!("Supplement passage is not visible: {value}"));
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
    }
    async fn smoke_supplement(self: &Rc<Self>) -> Result<(), String> {
        self.smoke_content_value("document.getElementById('supplement-link').click() ?? true")
            .await?;
        until("supplement link", || {
            let state = self.state.borrow();
            state.ready && state.supplement && state.fragment == "s35"
        })
        .await?;
        self.smoke_content_value("document.getElementById('second-fragment').click() ?? true")
            .await?;
        // Let the normal LinkClicked policy and anchor layout settle.
        glib::timeout_future(std::time::Duration::from_millis(300)).await;
        self.smoke_supplement_visible().await?;
        if self.state.borrow().fragment != "s70" {
            return Err("Same-section supplement navigation retained the previous fragment".into());
        }
        Ok(())
    }
    pub(crate) fn smoke_position_version(&self) -> u64 {
        self.state.borrow().version
    }
    pub(crate) async fn smoke_before_recovery(self: &Rc<Self>) -> Result<Value, String> {
        until("EPUB open", || self.state.borrow().ready).await?;
        self.chapter(1, false);
        until("EPUB second chapter", || {
            self.state.borrow().ready && self.state.borrow().current == 1
        })
        .await?;
        self.font.set_value(135.0);
        self.theme.set_selected(1);
        self.flow.set_selected(1);
        self.sidebar.set_reveal_child(true);
        until("EPUB location", || {
            self.state.borrow().reading.as_ref().is_some_and(|r| {
                !r.cfi.is_empty()
                    && r.font_scale == 1.35
                    && r.flow == "paginated"
                    && r.theme == "sepia"
            })
        })
        .await?;
        glib::timeout_future(std::time::Duration::from_millis(500)).await;
        // Configure can reflow/init the surface. Navigate only after that
        // settles, and require a real mid-chapter position rather than accepting
        // the nonempty CFI produced at the chapter's first page.
        self.javascript("StudioReader.anchor('p50')".into());
        until("EPUB mid-chapter location", || {
            let state = self.state.borrow();
            state
                .reading
                .as_ref()
                .zip(state.book.as_ref())
                .is_some_and(|(r, b)| {
                    r.progress * b.spine.len() as f64 - state.current as f64 > 0.20
                })
        })
        .await?;
        glib::timeout_future(std::time::Duration::from_millis(200)).await;
        if std::env::var("STUDIO_GNOME_SMOKE_BOOK_LOCATION").as_deref() == Ok("supplement") {
            self.smoke_supplement().await?;
        }
        self.search_book();
        let panel = self
            .book_search
            .borrow()
            .clone()
            .ok_or("Recovery book-search dialog missing")?;
        let search = panel.smoke_recovery_prepare().await?;
        let pending_search = panel.smoke_recovery_serial();
        let state = self.state.borrow();
        Ok(
            json!({"id":state.id,"generation":state.generation,"reading":state.reading,"search":search,"pendingSearch":pending_search,"supplement":state.supplement,"resource":state.resource,"fragment":state.fragment,"documentEpoch":state.document_epoch}),
        )
    }
    pub(crate) fn smoke_search_offline(&self, before: &Value) -> Result<(), String> {
        self.book_search
            .borrow()
            .as_ref()
            .ok_or("Offline search dialog disappeared")?
            .smoke_recovery_offline(&before["search"])
    }
    pub(crate) fn smoke_search_reconnect(&self) -> Result<(), String> {
        self.book_search
            .borrow()
            .as_ref()
            .ok_or("Reconnect search dialog disappeared")?
            .smoke_reconnect()
    }
    pub(crate) async fn smoke_search_recovered(
        self: &Rc<Self>,
        before: &Value,
        conflict: bool,
        root: &std::path::Path,
    ) -> Result<Value, String> {
        let panel = self
            .book_search
            .borrow()
            .clone()
            .ok_or("Retained book search disappeared")?;
        let mut result = panel.smoke_recovery_retained(&before["search"], conflict)?;
        crate::window_capture::capture_dialog(
            &self.parent.upgrade().ok_or("Search parent missing")?,
            &panel.dialog,
            &root.join("epub-search-recovered.png"),
        )
        .await?;
        if let Some(serial) = before["pendingSearch"].as_u64() {
            let path = root.join("epub-search-held.json");
            let held = gio::spawn_blocking(move || std::fs::read(path))
                .await
                .map_err(|_| "Held search reply reader stopped")?
                .map_err(|e| e.to_string())?;
            let held: Value = serde_json::from_slice(&held).map_err(|e| e.to_string())?;
            let page: crate::epub_search::Page =
                serde_json::from_value(held["response"].clone()).map_err(|e| e.to_string())?;
            page.validate()?;
            if page.start != 64 || page.total != 140 {
                return Err("Held response is not the real interrupted search page".into());
            }
            self.event(Response {
                generation: before["generation"].as_u64().unwrap(),
                serial,
                kind: "search",
                result: Ok(Reply::Search(page)),
            });
            panel.smoke_recovery_retained(&before["search"], conflict)?;
            result["retiredInFlightPageRejected"] = true.into();
        }
        if conflict {
            panel.dialog.force_close();
            return Ok(result);
        }
        if self
            .state
            .borrow()
            .book
            .as_ref()
            .is_some_and(|b| b.spine.len() == 140)
        {
            result["pagedRecovery"] = panel.smoke_recovery_pages(&before["search"]).await?;
        }
        panel.smoke_retained_query(&before["search"]).await?;
        panel.smoke_activate_retained();
        // Reopen before the old close animation completes. Its closed callback
        // must not consume or revive the new dialog.
        self.search_book();
        let fresh = self
            .book_search
            .borrow()
            .clone()
            .ok_or("Recovered book-search dialog did not reopen")?;
        if Rc::ptr_eq(&fresh, &panel) {
            return Err("Reopening revived a retired book-search dialog".into());
        }
        until("retained search result navigates recovered book", || {
            self.state.borrow().ready && self.state.borrow().current == 2
        })
        .await?;
        if self.search.text().as_str() != "Needle" || self.case.is_active() {
            return Err("Retained result used the unsubmitted search draft".into());
        }
        panel.smoke_expired_search()?;
        if !self
            .book_search
            .borrow()
            .as_ref()
            .is_some_and(|p| Rc::ptr_eq(p, &fresh))
        {
            return Err("Old search close removed the new dialog".into());
        }
        fresh.smoke_search_current_book().await?;
        fresh.dialog.force_close();
        result["recoveredDialogNavigation"] = true.into();
        result["retainedDialogQuery"] = true.into();
        result["newDocumentQuery"] = true.into();
        result["closedCallbacksRetired"] = true.into();
        result["immediateReopenIsolated"] = true.into();
        Ok(result)
    }
    pub(crate) async fn smoke_after_recovery(
        self: &Rc<Self>,
        before: &Value,
        conflict: bool,
    ) -> Result<Value, String> {
        until("EPUB recovery", || {
            let state = self.state.borrow();
            !state.resume_pending
                && if conflict {
                    state.recovering && state.id.is_none()
                } else {
                    !state.recovering
                        && state.ready
                        && if before["supplement"] == true {
                            state.document_epoch > before["documentEpoch"].as_u64().unwrap()
                        } else {
                            state.version > before["versionAtLoss"].as_u64().unwrap()
                        }
                        && state.reading.as_ref().is_some_and(|r| {
                            Some(r.cfi.as_str()) == before["reading"]["cfi"].as_str()
                        })
                }
        })
        .await
        .map_err(|e| {
            format!(
                "{e}: actual CFI {:?}, expected {}, version {}",
                self.state.borrow().reading.as_ref().map(|r| &r.cfi),
                before["reading"]["cfi"],
                self.state.borrow().version
            )
        })?;
        let state = self.state.borrow();
        let reading = state
            .reading
            .as_ref()
            .ok_or("Retained EPUB location missing")?;
        if state.current != 1
            || state.supplement != (before["supplement"] == true)
            || (state.supplement
                && (Some(state.resource.as_str()) != before["resource"].as_str()
                    || Some(state.fragment.as_str()) != before["fragment"].as_str()))
            || reading.font_scale != 1.35
            || reading.theme != "sepia"
            || reading.flow != "paginated"
            || !self.sidebar.reveals_child()
        {
            return Err("EPUB recovery reset chapter or reading preferences".into());
        }
        if conflict {
            if !self.status.text().contains("changed") || self.next.is_sensitive() {
                return Err("EPUB conflict left live navigation".into());
            }
        } else {
            if state.id.as_deref() == before["id"].as_str()
                || reading.cfi.is_empty()
                || epub::cfi_chapter(&reading.cfi) != Some(1)
            {
                return Err("EPUB recovery reused the old document or lost its CFI".into());
            }
            if Some(reading.cfi.as_str()) != before["reading"]["cfi"].as_str() {
                return Err(format!(
                    "EPUB recovery CFI changed: {} -> {}",
                    before["reading"]["cfi"], reading.cfi
                ));
            }
        }
        drop(state);
        let supplement = before["supplement"] == true;
        let surface = if supplement && !conflict {
            let surface = self.smoke_supplement_visible().await?;
            let uri = url::Url::parse(surface["href"].as_str().ok_or("Supplement URL missing")?)
                .map_err(|e| e.to_string())?;
            if uri.host_str() != self.state.borrow().id.as_deref()
                || !surface["text"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("Supplement paragraph 70")
            {
                return Err("Supplement content did not come from the new lease".into());
            }
            Some(surface)
        } else {
            None
        };
        self.event(Response {
            generation: before["generation"].as_u64().unwrap(),
            serial: 0,
            kind: "save",
            result: Err("STALE EPUB REPLY".into()),
        });
        if self.status.text().contains("STALE") {
            return Err("Retired EPUB reply reached the new document".into());
        }
        Ok(
            json!({"snapshotRevalidated":!conflict,"conflictProtected":conflict,"chapterRetained":true,"cfiRetained":true,"themeRetained":true,"fontRetained":true,"flowRetained":true,"contentsRetained":true,"staleReplyRejected":true,"supplement":supplement,"supplementSurface":surface}),
        )
    }
}
