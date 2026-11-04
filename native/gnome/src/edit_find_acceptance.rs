use super::*;
use std::{
    path::{Path, PathBuf},
    time::Instant,
};
impl Source {
    pub(crate) async fn smoke_edit_find(
        &self,
        root: &Path,
        capture: impl Fn(&str),
    ) -> Result<serde_json::Value, String> {
        async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
            let start = Instant::now();
            while !test() {
                if start.elapsed() > Duration::from_secs(20) {
                    return Err(format!("Editor Find timed out: {step}"));
                }
                glib::timeout_future(Duration::from_millis(10)).await;
            }
            Ok(())
        }
        async fn read(path: PathBuf) -> Result<Vec<u8>, String> {
            gtk::gio::spawn_blocking(move || std::fs::read(path).map_err(|e| e.to_string()))
                .await
                .map_err(|_| "Find fixture read stopped")?
        }
        let project = root.join("project");
        let path = project.join("find.rs");
        let original = read(path.clone()).await?;
        self.open(project.to_str().unwrap(), "find.rs");
        until("source", || {
            self.text.borrow().is_some() && self.widget.is_mapped()
        })
        .await?;
        self.smoke_edit_mode(true);
        until("editor", || self.edit_find_input().is_some()).await?;
        self.search.toggle.emit_clicked();
        let query = "needle 中文😀  ";
        self.search.entry.set_text(query);
        until("saved matches", || self.search.index.get().is_some()).await?;
        if self.search.matches.borrow().hits.len() != 3 || self.find_selection() != query {
            return Err("Editor Find lost original occurrences or whitespace".into());
        }
        self.edit_insert("// draft needle 中文😀  \n");
        let ticks = Rc::new(Cell::new(0));
        let max_gap = Rc::new(Cell::new(0u128));
        let clock = Rc::new(Cell::new(Instant::now()));
        let timer = glib::timeout_add_local(Duration::from_millis(10), {
            let ticks = ticks.clone();
            let gap = max_gap.clone();
            let clock = clock.clone();
            move || {
                let now = Instant::now();
                gap.set(
                    gap.get()
                        .max(now.duration_since(clock.replace(now)).as_millis()),
                );
                ticks.set(ticks.get() + 1);
                glib::ControlFlow::Continue
            }
        });
        let started = Instant::now();
        let matched = until("dirty full-document matches", || {
            self.search.index.get().is_some()
        })
        .await;
        let find_ms = started.elapsed().as_millis();
        timer.remove();
        matched?;
        if self.search.matches.borrow().hits.len() != 4
            || self.find_selection() != query
            || read(path.clone()).await? != original
        {
            return Err("Finding a draft changed the file or omitted its text".into());
        }
        capture("edit-find-draft.png");
        self.search.entry.set_text("été");
        until("Unicode matches", || self.search.index.get().is_some()).await?;
        if self.search.matches.borrow().hits.len() != 3 {
            return Err("Editor Unicode folding differs from Source".into());
        }
        self.search.case.set_active(true);
        until("case-sensitive match", || self.search.index.get().is_some()).await?;
        if self.search.matches.borrow().hits.len() != 1 || self.find_selection() != "été" {
            return Err("Editor case toggle did not apply".into());
        }
        self.search.case.set_active(false);
        self.search.entry.set_text(query);
        until("query restored", || self.search.index.get().is_some()).await?;
        self.smoke_edit_history(false);
        until("undo requery", || self.search.index.get().is_some()).await?;
        if self.search.matches.borrow().hits.len() != 3 || self.editor.dirty.get() {
            return Err("Find replaced the undo history or kept deleted draft matches".into());
        }
        self.smoke_edit_history(true);
        until("redo requery", || self.search.index.get().is_some()).await?;
        if self.search.matches.borrow().hits.len() != 4 || !self.editor.dirty.get() {
            return Err("Redo did not restore searchable draft text".into());
        }
        self.search.next.emit_clicked();
        if self.search.index.get() != Some(1) || self.find_selection() != query {
            return Err("Local Next reloaded the edit page".into());
        }
        let before = self.edit_text();
        self.search.next.emit_clicked();
        self.edit_response("cancel").await?;
        if self.search.index.get() != Some(1)
            || self.edit_text() != before
            || read(path.clone()).await? != original
        {
            return Err("Cancelling a cross-page find changed editor state".into());
        }
        self.search.next.emit_clicked();
        self.edit_response("save").await?;
        until("saved cross-page jump", || {
            self.search.index.get() == Some(2)
                && self
                    .edit_find_input()
                    .is_some_and(|(page, _)| page.window.line == 306)
        })
        .await?;
        let mut saved = "// draft needle 中文😀  \r\n".as_bytes().to_vec();
        saved.extend_from_slice(&original);
        if self.find_selection() != query
            || read(path.clone()).await? != saved
            || self.editor.dirty.get()
        {
            return Err("Save-and-find opened an incorrect occurrence".into());
        }
        self.edit_insert("// second draft removed\n");
        until("second draft", || self.search.index.get().is_some()).await?;
        self.search.next.emit_clicked();
        self.edit_response("discard").await?;
        until("discarded cross-page jump", || {
            self.search.index.get() == Some(3)
                && self
                    .edit_find_input()
                    .is_some_and(|(page, _)| page.window.start > 4 * 1024 * 1024)
        })
        .await?;
        if self.find_selection() != query
            || self.edit_text().contains("second draft")
            || read(path.clone()).await? != saved
        {
            return Err("Discard-and-find used stale draft coordinates or wrote the file".into());
        }
        self.search.entry.set_text("unique transient 中文😀  ");
        until("absent draft phrase", || {
            self.search.count.text() == crate::language::tr("No matches found")
        })
        .await?;
        self.edit_insert("// unique transient 中文😀  \n");
        until("new draft phrase", || self.search.index.get().is_some()).await?;
        let serial = self.search.serial.get();
        let generation = self.generation.get();
        self.edit_insert("// newer edit\n");
        self.event(Response {
            panel: 3,
            generation,
            serial,
            kind: "source-search",
            result: Ok(Reply::SourceEditedMatches(
                crate::source_search::EditMatches {
                    matches: Matches {
                        hits: vec![Hit {
                            start: 0,
                            end: 1,
                            line: 1,
                        }],
                        limited: false,
                    },
                    targets: vec![EditTarget {
                        local: Some(0..1),
                        original: None,
                    }],
                },
            )),
        });
        if self.search.index.get().is_some() {
            return Err("Obsolete draft search changed the current selection".into());
        }
        until("latest draft phrase", || self.search.index.get().is_some()).await?;
        self.smoke_edit_mode(false);
        until("saved Source excludes draft", || {
            self.search.count.text() == crate::language::tr("No matches found")
        })
        .await?;
        self.smoke_edit_mode(true);
        until("Edit retains searchable draft", || {
            self.search.index.get().is_some()
        })
        .await?;
        if self.find_selection() != "unique transient 中文😀  "
            || !self.edit_text().contains("newer edit")
            || read(path.clone()).await? != saved
        {
            return Err("Mode switching mixed saved and unsaved search projections".into());
        }
        self.search.entry.set_text("x");
        until("limited other-page results", || {
            self.search.matches.borrow().limited && self.search.index.get().is_none()
        })
        .await?;
        if !self
            .search
            .count
            .tooltip_text()
            .as_deref()
            .is_some_and(|text| text.contains("first 1000 matches"))
        {
            return Err("Elided result count hid the search limit".into());
        }
        self.search.entry.set_text("unique transient 中文😀  ");
        until("draft query after limit", || {
            self.search.index.get().is_some()
        })
        .await?;
        crate::language::set("zh-CN", "en-US")?;
        crate::i18n_ui::apply();
        if self.search.toggle.tooltip_text().as_deref() != Some("在编辑内容中查找")
            || self.search.entry.text() != "unique transient 中文😀  "
        {
            return Err("Editor Find language switch changed the query or missed its label".into());
        }
        capture("edit-find-chinese.png");
        crate::language::set("en-US", "en-US")?;
        crate::i18n_ui::apply();
        self.smoke_close();
        self.edit_response("discard").await?;
        self.open(project.to_str().unwrap(), "real-app.rs");
        until("real source", || self.text.borrow().is_some()).await?;
        if self.search.toggle.tooltip_text().as_deref() != Some("Find in source") {
            return Err("New Source file retained editor search controls".into());
        }
        self.smoke_edit_mode(true);
        until("real editor", || self.edit_find_input().is_some()).await?;
        self.search.toggle.emit_clicked();
        self.search.entry.set_text(".max_blocking_threads(2)");
        until("other-page results", || {
            self.search.matches.borrow().hits.len() == 1
        })
        .await?;
        if self.search.index.get().is_some() {
            return Err("Typing a query unexpectedly left the current page".into());
        }
        self.search.next.emit_clicked();
        until("real code match", || self.search.index.get().is_some()).await?;
        if self.find_selection() != ".max_blocking_threads(2)" {
            return Err("Clean editor navigation lost the actual file occurrence".into());
        }
        capture("edit-find-real-file.png");
        self.search_failed("Fixture worker busy");
        self.search.matches.replace(Matches::default());
        self.search.index.set(None);
        self.search.entry.emit_activate();
        until("Enter retries find", || self.search.index.get().is_some()).await?;
        self.search.close.emit_clicked();
        if self.search.bar.is_visible() {
            return Err("Editor Find close did not dismiss the bar".into());
        }
        let diff = "@@ -1 +1 @@\n-before\n+after\n";
        self.show_prepared(workspace::Text {
            path: "prepared turn diff".into(),
            language: "diff".into(),
            document: studio_markdown_native::text::TextDocument::new(diff.into())?,
            preview: None,
            html: None,
            diff: Some(studio_markdown_native::unified_diff::Index::new(diff)),
            binary: false,
            truncated: false,
            file: None,
        });
        if self.editor.editing.get()
            || self.search.toggle.tooltip_text().as_deref() != Some("Find in source")
        {
            return Err("Prepared diff retained the editor Find context".into());
        }
        self.smoke_close();
        Ok(
            serde_json::json!({"ok":true,"bytes":original.len(),"findMs":find_ms,"mainLoopTicks":ticks.get(),"maxHeartbeatGapMs":max_gap.get(),"dirtyCorpus":true,"unicodeWhitespace":true,"caseToggle":true,"undoRedoRetained":true,"crossPageCancelSaveDiscard":true,"longLine":true,"staleDraftResults":true,"separateViewCorpora":true,"liveLanguage":true,"realFileFind":true,"explicitRetry":true}),
        )
    }
    fn find_selection(&self) -> String {
        let buffer = self.active_find_view().buffer();
        buffer
            .selection_bounds()
            .map(|(a, b)| buffer.text(&a, &b, true).to_string())
            .unwrap_or_default()
    }
}
