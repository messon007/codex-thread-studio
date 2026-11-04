//! The actual generic file actor and GTK controls, using private fixtures only.
use super::*;
use std::{
    path::Path,
    time::{Duration, Instant},
};
impl Source {
    pub async fn smoke_structured(&self, root: &Path) -> Result<serde_json::Value, String> {
        async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
            crate::acceptance_wait::until(step, test).await
        }
        async fn read(path: std::path::PathBuf) -> Result<Vec<u8>, String> {
            gtk::gio::spawn_blocking(move || std::fs::read(path).map_err(|e| e.to_string()))
                .await
                .map_err(|_| "Fixture read stopped")?
        }
        async fn capture(source: &Source, root: &Path, name: &str) -> Result<(), String> {
            let window = source
                .widget
                .root()
                .and_downcast::<gtk::Window>()
                .ok_or("Source window absent")?;
            for _ in 0..2 {
                crate::window_capture::next_frame(&window).await?;
            }
            let view = &source.editor.preview.view;
            let buffer = view.buffer();
            let at = buffer.iter_at_mark(&buffer.get_insert());
            let rect = view.iter_location(&at);
            let visible = view.visible_rect();
            if source.preview_active()
                && view.is_mapped()
                && (rect.y() < visible.y()
                    || rect.y() + rect.height() > visible.y() + visible.height())
            {
                return Err(format!("Preview cursor is outside its painted viewport: offset={} cursorY={} height={} visibleY={} visibleHeight={}",at.offset(),rect.y(),rect.height(),visible.y(),visible.height()));
            }
            crate::window_capture::capture(&window, &root.join(name)).await
        }
        let project = root.join("project");
        let project = project.to_str().unwrap();
        let mut stages = Vec::new();
        let stage_started = Cell::new(Instant::now());
        macro_rules! painted {
            ($name:expr) => {{
                crate::window_capture::next_frame(&self.widget).await?;
                let value=serde_json::json!({"stage":$name,"elapsedMs":stage_started.replace(Instant::now()).elapsed().as_millis(),
                    "previewBytes":self.editor.preview.window.borrow().as_ref().map(|w|w.text.len()),
                    "sourceBytes":self.window.borrow().as_ref().map(|w|w.text.len())});
                eprintln!("structured-paint {value}");
                stages.push(value);
            }};
        }
        let original = read(root.join("project/compact.json5")).await?;
        let start = Instant::now();
        self.open(project, "compact.json5");
        until("structured default Preview", || {
            self.preview_active()
                && self.editor.stack.visible_child_name().as_deref() == Some("preview")
        })
        .await?;
        let opening_ms = start.elapsed().as_millis();
        painted!("initial-preview");
        let view = &self.editor.preview.view;
        let source_buffer = view.buffer().downcast::<sourceview::Buffer>().unwrap();
        let style = source_buffer
            .style_scheme()
            .and_then(|scheme| scheme.style("def:error"))
            .ok_or("Preview reading style was not loaded")?;
        if source_buffer.is_highlight_matching_brackets()
            || style.pango_underline() != gtk::pango::Underline::None
        {
            return Err("Preview incorrectly marks partial context as syntax errors".into());
        }
        if view
            .buffer()
            .downcast::<sourceview::Buffer>()
            .unwrap()
            .language()
            .map(|l| l.id().to_string())
            .as_deref()
            != Some("js")
        {
            return Err("JSON5 did not use the installed JavaScript language".into());
        }
        fn translate_entries(menu: &gtk::gio::MenuModel) -> usize {
            (0..menu.n_items())
                .map(|i| {
                    usize::from(
                        menu.item_attribute_value(i, "action", None)
                            .and_then(|v| v.str().map(str::to_owned))
                            .as_deref()
                            == Some("translation.translate"),
                    ) + menu
                        .item_link(i, "section")
                        .map_or(0, |m| translate_entries(&m))
                })
                .sum()
        }
        if view.extra_menu().as_ref().map_or(0, translate_entries) != 1 {
            return Err("Preview has duplicated or missing Translate actions".into());
        }
        let buffer = view.buffer();
        let displayed = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string();
        if !displayed.contains("\"items\": [")
            || !displayed.contains("1e+04")
            || !displayed.contains("0xFF")
        {
            return Err(
                "Structured Preview lost formatted fields or literal numeric spelling".into(),
            );
        }
        crate::smoke_input::request(root, serde_json::json!({"action":"focus"})).await?;
        if !view.grab_focus() {
            return Err("Preview did not receive focus".into());
        }
        buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
        crate::smoke_input::request(root, serde_json::json!({"action":"key","key":"copy"})).await?;
        let copied =
            crate::smoke_input::request(root, serde_json::json!({"action":"read"})).await?;
        if copied["text"].as_str() != Some(&displayed) {
            return Err("Preview keyboard Copy changed rendered text".into());
        }
        let quoted = "\"items\": [";
        let at = displayed.find(quoted).unwrap();
        let a = displayed[..at].chars().count() as i32;
        let b = a + quoted.chars().count() as i32;
        buffer.select_range(&buffer.iter_at_offset(a), &buffer.iter_at_offset(b));
        let captured = Rc::new(RefCell::new(None));
        let store = captured.clone();
        let previous = self.on_comment.replace(Some(Box::new(move |capture| {
            store.replace(Some(capture));
        })));
        self.comment.emit_clicked();
        self.on_comment.replace(previous);
        let anchor = captured
            .borrow_mut()
            .take()
            .ok_or("Preview Comment did not capture its selection")?;
        if anchor.excerpt != "\"items\":["
            || anchor.source["anchor"]["startLine"] != 2
            || anchor.source["anchor"]["endLine"] != 2
        {
            return Err(format!(
                "Preview comment invented formatted source coordinates: {:?}",
                anchor
            ));
        }
        self.smoke_find_query(quoted);
        until("formatted find", || self.smoke_find_has_match()).await?;
        let (a, b) = buffer
            .selection_bounds()
            .ok_or("Preview Find did not select a match")?;
        if buffer.text(&a, &b, false) != quoted || self.active_find_view() != view {
            return Err("Preview Find searched or selected the Source buffer".into());
        }
        self.smoke_find_close();
        if let Some(window) = self.widget.root().and_downcast::<gtk::Window>() {
            crate::window_capture::next_frame(&window).await?;
        }
        capture(self, root, "structured-preview.png").await?;
        let mut parent = self.widget.parent();
        let mut split = None;
        while let Some(widget) = parent {
            if let Ok(paned) = widget.clone().downcast::<gtk::Paned>() {
                split = Some(paned);
                break;
            }
            parent = widget.parent();
        }
        if let Some(paned) = split {
            let old = paned.position();
            paned.set_position((paned.width() - 360).max(1));
            until("narrow structured pane", || {
                self.widget.width() <= 400 && self.widget.width() > 0
            })
            .await?;
            crate::window_capture::next_frame(&self.widget).await?;
            for control in [
                self.editor.modes.upcast_ref::<gtk::Widget>(),
                self.close.upcast_ref(),
            ] {
                let bounds = control
                    .compute_bounds(&self.widget)
                    .ok_or("Structured toolbar control is unmapped")?;
                if bounds.x() < 0.0
                    || bounds.x() + bounds.width() > self.widget.width() as f32 + 1.0
                {
                    return Err("Structured toolbar overflowed its narrow pane".into());
                }
            }
            capture(self, root, "structured-narrow.png").await?;
            paned.set_position(old);
            crate::window_capture::next_frame(&self.widget).await?;
        } else {
            return Err("Structured fixture has no real document divider".into());
        }
        self.editor.source.set_active(true);
        if self.visible_text().as_bytes() != original {
            return Err("Source did not retain the original CRLF document".into());
        }
        self.editor.edit.set_active(true);
        until("structured edit page", || {
            self.editor.page.borrow().is_some() && !self.editor.busy.get()
        })
        .await?;
        self.edit_insert("// saved 中文😀\n");
        self.editor.preview.button.set_active(true);
        if !self.editor.dirty.get()
            || !self.editor.notice.is_visible()
            || read(root.join("project/compact.json5")).await? != original
        {
            return Err("Preview switch wrote or discarded an unsaved edit".into());
        }
        self.editor.edit.set_active(true);
        let editor = self.editor.view.buffer();
        editor.undo();
        editor.redo();
        if !self.edit_text().starts_with("// saved 中文😀\n") {
            return Err("Preview switch lost editor Undo/Redo".into());
        }
        self.editor.preview.button.set_active(true);
        self.editor.save.emit_clicked();
        until("Preview explicit save", || {
            !self.editor.busy.get() && !self.editor.dirty.get()
        })
        .await?;
        if !self
            .editor
            .preview
            .view
            .buffer()
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .starts_with("// saved 中文😀")
        {
            return Err("Save did not refresh the formatted snapshot".into());
        }
        let saved = read(root.join("project/compact.json5")).await?;
        if saved != ["// saved 中文😀\r\n".as_bytes(), original.as_slice()].concat() {
            return Err("Preview save wrote formatted text over the original".into());
        }
        self.open_at(project, "compact.json5", 1);
        until("explicit line remains Source", || {
            self.text
                .borrow()
                .as_ref()
                .is_some_and(|t| t.path.ends_with("compact.json5"))
                && !self.preview_active()
        })
        .await?;
        if self.editor.stack.visible_child_name().as_deref() != Some("source") {
            return Err("Explicit line 1 entered Preview".into());
        }
        painted!("edit-save-source");
        let mut cases = Vec::new();
        for (path, needle) in [
            ("tree.xml", "<item>中文&amp;😀</item>"),
            ("data.yaml", "literal = content"),
            ("settings.toml", "key = 2"),
            (".env.local", "URL = https://host/x=y"),
            ("invalid.json", "{broken"),
            ("mixed.xml", "before <b>bold</b> after"),
        ] {
            self.open(project, path);
            until("structured format", || {
                self.text
                    .borrow()
                    .as_ref()
                    .is_some_and(|t| t.path.ends_with(path))
                    && self.preview_active()
            })
            .await?;
            let b = self.editor.preview.view.buffer();
            if !b
                .text(&b.start_iter(), &b.end_iter(), false)
                .contains(needle)
            {
                return Err(format!("Preview did not preserve {path}"));
            }
            cases.push(path);
            painted!(path);
        }
        self.open(project, "many.json");
        until("paged structured Preview", || {
            self.text
                .borrow()
                .as_ref()
                .is_some_and(|t| t.path.ends_with("many.json"))
                && self.preview_active()
        })
        .await?;
        let first = self
            .editor
            .preview
            .window
            .borrow()
            .as_ref()
            .unwrap()
            .clone();
        painted!("many-open");
        if !self.next.is_sensitive() {
            return Err("Long formatted document did not page".into());
        }
        self.editor.edit.set_active(true);
        until("paged Preview edit", || {
            self.editor.page.borrow().is_some() && !self.editor.busy.get()
        })
        .await?;
        self.edit_insert(" ");
        self.editor.preview.button.set_active(true);
        self.next.emit_clicked();
        until("Preview page change saved original", || {
            !self.editor.busy.get()
                && !self.editor.dirty.get()
                && self
                    .editor
                    .preview
                    .window
                    .borrow()
                    .as_ref()
                    .is_some_and(|w| w.start > 0)
        })
        .await?;
        if self.editor.preview.window.borrow().as_ref().unwrap().start != first.end {
            return Err("Preview Next lost its page boundary".into());
        }
        self.previous.emit_clicked();
        self.smoke_find_query("last_row_1199");
        until("cross-page Preview find", || self.smoke_find_has_match()).await?;
        let captured = self
            .capture()
            .ok_or("Paged Preview match has no source capture")?;
        if captured.excerpt != "last_row_1199" || captured.source["anchor"]["startLine"] != 1 {
            return Err("Paged Preview comment used formatted line numbers".into());
        }
        self.smoke_find_close();
        capture(self, root, "structured-paged.png").await?;
        painted!("many-find");
        self.open(project, "large.json");
        until("large plain Preview", || {
            self.text
                .borrow()
                .as_ref()
                .is_some_and(|t| t.path.ends_with("large.json"))
                && self.preview_active()
        })
        .await?;
        if !self
            .text
            .borrow()
            .as_ref()
            .unwrap()
            .preview
            .as_ref()
            .unwrap()
            .limited
            || self
                .editor
                .preview
                .window
                .borrow()
                .as_ref()
                .unwrap()
                .text
                .len()
                > studio_markdown_native::text::WINDOW_BYTES
        {
            return Err("Large structured Preview exceeded its bounded page".into());
        }
        painted!("large-open");
        if self
            .editor
            .preview
            .view
            .buffer()
            .downcast::<sourceview::Buffer>()
            .unwrap()
            .is_highlight_syntax()
            || self
                .view
                .buffer()
                .downcast::<sourceview::Buffer>()
                .unwrap()
                .is_highlight_syntax()
        {
            return Err("Long JSON lines reached the GTK regex engine".into());
        }
        if self
            .editor
            .preview
            .view
            .buffer()
            .downcast::<sourceview::Buffer>()
            .unwrap()
            .language()
            .is_some()
            || self
                .view
                .buffer()
                .downcast::<sourceview::Buffer>()
                .unwrap()
                .language()
                .is_some()
        {
            return Err("Long JSON lines retained a GTK context engine".into());
        }
        self.editor.edit.set_active(true);
        until("large plain editor", || {
            self.editor.page.borrow().is_some() && !self.editor.busy.get()
        })
        .await?;
        if self
            .editor
            .view
            .buffer()
            .downcast::<sourceview::Buffer>()
            .unwrap()
            .is_highlight_syntax()
        {
            return Err("Large edit page enabled the GTK regex engine".into());
        }
        self.editor.preview.button.set_active(true);
        self.open(project, root.join("readonly/shared.json").to_str().unwrap());
        until("read-only structured Preview", || {
            self.text
                .borrow()
                .as_ref()
                .is_some_and(|t| t.path.ends_with("shared.json"))
                && self.preview_active()
        })
        .await?;
        if self.editor.edit.is_visible() || !self.editor.modes.is_visible() {
            return Err("Read-only Preview lost view controls or exposed Edit".into());
        }
        crate::language::set("zh-CN", "en-US")?;
        crate::i18n_ui::apply();
        if self.editor.preview.button.label().as_deref() != Some("预览")
            || !self.status.text().contains("预览")
        {
            return Err("Preview controls did not follow interface language".into());
        }
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
        let b = self
            .editor
            .preview
            .view
            .buffer()
            .downcast::<sourceview::Buffer>()
            .unwrap();
        if b.style_scheme().map(|s| s.id().to_string()).as_deref()
            != Some("studio-preview-Adwaita-dark")
        {
            return Err("Preview did not follow dark theme".into());
        }
        gtk::glib::timeout_future(Duration::from_millis(250)).await;
        capture(self, root, "structured-dark.png").await?;
        painted!("large-edit-readonly-dark");
        crate::language::set("en-US", "en-US")?;
        crate::i18n_ui::apply();
        self.open(project, "compact.json5");
        self.open(project, "other.rs");
        until("obsolete Preview rejected", || {
            self.text
                .borrow()
                .as_ref()
                .is_some_and(|t| t.path.ends_with("other.rs"))
        })
        .await?;
        gtk::glib::timeout_future(Duration::from_millis(150)).await;
        if self.preview_active()
            || self
                .text
                .borrow()
                .as_ref()
                .unwrap()
                .path
                .ends_with("compact.json5")
        {
            return Err("Late formatted reply replaced a new file".into());
        }
        self.editor.edit.set_active(true);
        until("ordinary edit colors", || {
            self.editor.page.borrow().is_some() && !self.editor.busy.get()
        })
        .await?;
        self.edit_insert(&"x".repeat(8192));
        let editable = self
            .editor
            .view
            .buffer()
            .downcast::<sourceview::Buffer>()
            .unwrap();
        if editable.is_highlight_syntax() {
            return Err("Long pasted line reached GTK highlighting".into());
        }
        editable.undo();
        until("Undo restored ordinary syntax colors", || {
            editable.is_highlight_syntax() && editable.language().is_some()
        })
        .await?;
        self.close.emit_clicked();
        Ok(
            serde_json::json!({"ok":true,"openingMs":opening_ms,"paintStages":stages,"formats":cases,"nativeKeyboardCopy":true,"originalCommentAnchors":true,"formattedFind":true,"pageFind":true,"pageChangeSave":true,"boundedLargePreview":true,"longLineSyntaxAdmission":true,"pasteUndoSyntax":true,"dirtyViewSwitch":true,"explicitSave":true,"undoRedo":true,"explicitSourceLine":true,"readOnly":true,"liveLanguageTheme":true,"staleReply":true}),
        )
    }
}
