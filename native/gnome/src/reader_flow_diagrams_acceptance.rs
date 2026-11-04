//! Real reader adapter: text first, native asynchronous diagrams afterwards.
use super::*;
impl Reader {
    pub async fn smoke_flow_diagrams(
        self: &Rc<Self>,
        directory: &Path,
    ) -> Result<serde_json::Value, String> {
        async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
            let at = std::time::Instant::now();
            while !test() {
                if at.elapsed() > Duration::from_secs(20) {
                    return Err(format!("Lazy flow diagram timed out: {step}"));
                }
                glib::timeout_future(Duration::from_millis(20)).await;
            }
            Ok(())
        }
        fn button(root: &gtk::Widget, label: &str) -> Option<gtk::ToggleButton> {
            if let Some(b) = root
                .downcast_ref::<gtk::ToggleButton>()
                .filter(|b| b.label().as_deref() == Some(label))
            {
                return Some(b.clone());
            }
            let mut child = root.first_child();
            while let Some(w) = child {
                child = w.next_sibling();
                if let Some(b) = button(&w, label) {
                    return Some(b);
                }
            }
            None
        }
        fn retry(root: &gtk::Widget) -> bool {
            if root
                .downcast_ref::<gtk::Button>()
                .is_some_and(|b| b.is_visible() && b.label().as_deref() == Some("Retry diagram"))
            {
                return true;
            }
            let mut child = root.first_child();
            while let Some(w) = child {
                child = w.next_sibling();
                if retry(&w) {
                    return true;
                }
            }
            false
        }
        fn text(root: &gtk::Widget, needle: &str) -> bool {
            if root.downcast_ref::<gtk::TextView>().is_some_and(|v| {
                let b = v.buffer();
                b.text(&b.start_iter(), &b.end_iter(), false)
                    .contains(needle)
            }) {
                return true;
            }
            let mut child = root.first_child();
            while let Some(w) = child {
                child = w.next_sibling();
                if text(&w, needle) {
                    return true;
                }
            }
            false
        }
        fn retained_pixels(root: &gtk::Widget) -> bool {
            if root
                .downcast_ref::<crate::scaled_picture::ScaledPicture>()
                .is_some_and(|p| p.paintable().is_some() || p.vector().is_some())
            {
                return true;
            }
            let mut child = root.first_child();
            while let Some(w) = child {
                child = w.next_sibling();
                if retained_pixels(&w) {
                    return true;
                }
            }
            false
        }
        self.flow_diagrams.smoke_preview_budget()?;
        let viewport = self
            .flow_diagrams
            .smoke_viewport(&self.window, directory)
            .await?;
        let source = r#"Before diagrams 中文😀.

```mermaid
flowchart LR
A["Native $$\frac{a}{b}$$"] -->|"$$\sqrt{x+1}$$"| B["Selectable $$x^2$$ and $$\unknownNativeMacro{x}$$"]
```

```mermaid
sequenceDiagram
participant A as $$\alpha$$
participant B as $$\beta$$
A->>B: Solve $$\sqrt{2+2}$$
B-->>A: $$\frac{1}{2}$$
Note right of B: Answer $$2$$
```

```plantuml
@startuml
Alice -> Bob: Native flow
@enduml
```

```d2
a -> b: Native flow
```

```math
x=\frac{a}{b}
```

```mermaid
flowchart LR
A[unterminated
```

After diagrams.
"#.to_string();
        let path = directory.join("project/flow-diagrams.md");
        let (write, data) = (path.clone(), source.clone());
        gio::spawn_blocking(move || std::fs::write(write, data))
            .await
            .map_err(|_| "Flow writer stopped")?
            .map_err(|e| e.to_string())?;
        // A line reference with no preceding heading opens Source, avoiding the
        // ordinary block renderer while this experimental adapter is installed.
        self.open_project_path(&directory.join("project"), &path, Some(1), None);
        until("indexed source", || {
            self.is_ready()
                && self.state.borrow().mode == Mode::Source
                && self.state.borrow().source.is_some()
        })
        .await?;
        let revision = self.state.borrow().index.as_ref().unwrap().revision.clone();
        let content = source.clone();
        let expected_revision = revision.clone();
        let at = std::time::Instant::now();
        let prepared = gio::spawn_blocking(move || {
            let f = studio_markdown_native::Fragment {
                page: 0,
                document_id: "private-flow-diagrams".into(),
                revision: expected_revision,
                metadata: studio_markdown_native::FragmentMetadata {
                    start: 0,
                    end: content.len(),
                    line: 1,
                    rows: 0,
                    row_start: 0,
                    table: None,
                },
                source: SourceWindow {
                    start: 0,
                    end: content.len(),
                    line: 1,
                    end_line: content.lines().count(),
                    text: content,
                    projection: vec![],
                    references: vec![],
                },
                header: None,
            };
            crate::text_flow::prepare_lazy_diagrams(
                &f,
                Arc::new(std::sync::atomic::AtomicBool::new(false)),
            )
        })
        .await
        .map_err(|_| "Flow preparation worker stopped")??;
        let prepare_ms = at.elapsed().as_millis();
        if prepared.raster_bytes != 0 || prepared.images.iter().any(Option::is_some) {
            return Err("Deferred preparation executed a standalone engine".into());
        }
        self.state.borrow_mut().mode = Mode::Preview;
        self.stack.set_visible_child_name("preview");
        self.sync_modes();
        // Diagram mounting must retain the saved snapshot independently of
        // comment eligibility while an editor has an in-memory dirty page.
        self.state.borrow_mut().dirty = true;
        let mounted = self.flow_view(prepared);
        self.state.borrow_mut().dirty = false;
        let flow = mounted?;
        let diagrams = flow
            .objects
            .iter()
            .map(|o| {
                o.clone()
                    .downcast::<gtk::Box>()
                    .map_err(|_| "Native diagram control missing".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        if diagrams.len() != 6 {
            return Err("Flow diagram identity changed".into());
        }
        self.preview_scroll.set_child(Some(&flow.widget));
        until("selectable text allocation", || {
            flow.widget.is_mapped()
                && flow.widget.width() > 0
                && flow
                    .widget
                    .iter_location(&flow.widget.buffer().start_iter())
                    .height()
                    > 0
        })
        .await?;
        let text_ms = at.elapsed().as_millis();
        let expected = flow
            .document
            .select(0..flow.document.characters, 512 * 1024)?;
        let buffer = flow.widget.buffer();
        buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
        flow.widget.emit_copy_clipboard();
        until("cross-block source copy", || {
            flow.copied.borrow().is_some() && !flow.copying.get()
        })
        .await?;
        let copy = flow
            .copied
            .borrow()
            .as_ref()
            .unwrap()
            .as_ref()
            .map_err(Clone::clone)?
            .clone();
        if copy.text != expected.text
            || !copy.text.contains("A[unterminated")
            || !copy.text.contains("After diagrams.")
        {
            return Err("Deferred diagrams lost Copy alternatives".into());
        }
        async fn reveal(
            scroll: &gtk::ScrolledWindow,
            view: &gtk::TextView,
            offset: usize,
        ) -> Result<(), String> {
            crate::window_capture::next_frame(scroll).await?;
            // TextView parks offscreen anchor children at negative coordinates.
            // Navigate by the buffer anchor, never that placeholder allocation.
            let mut iter = view.buffer().iter_at_offset(offset as i32);
            view.scroll_to_iter(&mut iter, 0.0, true, 0.0, 0.0);
            crate::window_capture::next_frame(scroll).await
        }
        // Viewport admission intentionally prevents every chart from retaining
        // a raster simultaneously. Visit each real engine, preserving the
        // original format/error coverage instead of disabling lazy loading.
        for (index, diagram) in diagrams.iter().enumerate() {
            reveal(
                &self.preview_scroll,
                &flow.widget,
                flow.document.objects[index].characters.start,
            )
            .await?;
            until("visible real engine or isolated error", || {
                if index < 5 {
                    self.flow_diagrams.smoke_style_state(diagram).is_some()
                } else {
                    retry(diagram.upcast_ref())
                }
            })
            .await
            .map_err(|error| {
                format!(
                    "{error}: diagram={index}, bounds={:?}, scroll={}, upper={}, page={}",
                    diagram.compute_bounds(&self.preview_scroll),
                    self.preview_scroll.vadjustment().value(),
                    self.preview_scroll.vadjustment().upper(),
                    self.preview_scroll.vadjustment().page_size(),
                )
            })?;
            if index < 2 {
                let actual = self.flow_diagrams.smoke_style_state(diagram).unwrap();
                if actual["mathLabels"] != [3, 7][index] {
                    return Err(format!(
                        "Mermaid math labels did not become native outlines: {actual}"
                    ));
                }
                crate::window_capture::capture(
                    &self.window,
                    &directory.join(format!("flow-mermaid-math-{index}.png")),
                )
                .await?;
            }
        }
        let source_button = button(diagrams[5].upcast_ref(), "Source")
            .ok_or("Failed diagram Source control missing")?;
        source_button.set_active(true);
        if !text(diagrams[5].upcast_ref(), "A[unterminated") {
            return Err("Failed diagram source became unreadable".into());
        }
        crate::language::set("zh-CN", "en-US")?;
        crate::i18n_ui::apply();
        let translated = source_button.label().as_deref() == Some("源码")
            && source_button.tooltip_text().as_deref() == Some("查看图表源码")
            && text(diagrams[5].upcast_ref(), "A[unterminated");
        crate::language::set("en-US", "en-US")?;
        crate::i18n_ui::apply();
        if !translated {
            return Err("Native diagram controls did not translate independently of source".into());
        }
        reveal(
            &self.preview_scroll,
            &flow.widget,
            flow.document.objects[0].characters.start,
        )
        .await?;
        until("first chart restored for theme replay", || {
            self.flow_diagrams.smoke_style_state(&diagrams[0]).is_some()
        })
        .await?;
        let first = self.flow_diagrams.smoke_style_state(&diagrams[0]).unwrap();
        let style = adw::StyleManager::default();
        let previous = style.color_scheme();
        style.set_color_scheme(if style.is_dark() {
            adw::ColorScheme::ForceLight
        } else {
            adw::ColorScheme::ForceDark
        });
        until("native theme replay", || {
            self.flow_diagrams
                .smoke_style_state(&diagrams[0])
                .is_some_and(|s| s["serial"] != first["serial"] && s["pixels"] != first["pixels"])
        })
        .await?;
        if self.flow_diagrams.smoke_style_state(&diagrams[0]).unwrap()["mathLabels"] != 3 {
            return Err("Theme replay lost Mermaid math outlines".into());
        }
        crate::window_capture::capture(
            &self.window,
            &directory.join("flow-mermaid-math-theme.png"),
        )
        .await?;
        style.set_color_scheme(previous);
        until("restored theme is rendered", || {
            self.flow_diagrams.smoke_style_state(&diagrams[0]).is_some()
        })
        .await?;
        crate::window_capture::capture(&self.window, &directory.join("flow-native-diagrams.png"))
            .await?;
        self.set_mode(Mode::Source);
        until("Source switch releases diagrams", || {
            self.state.borrow().source.is_some()
                && self.is_ready()
                && diagrams.iter().all(|w| {
                    self.flow_diagrams.smoke_style_state(w).is_none()
                        && !retained_pixels(w.upcast_ref())
                })
        })
        .await?;
        if self.preview_scroll.child().as_ref() != Some(self.preview.upcast_ref()) {
            return Err("Source switch retained an old continuous view".into());
        }
        self.finish_close();
        let unchanged =
            gio::spawn_blocking(move || std::fs::read_to_string(path).map_err(|e| e.to_string()))
                .await
                .map_err(|_| "Flow verification worker stopped")??;
        if unchanged != source {
            return Err("Reading deferred diagrams changed the file".into());
        }
        Ok(
            serde_json::json!({"ok":true,"prepareMs":prepare_ms,"firstTextAllocatedMs":text_ms,"rasterBytesBeforeMount":0,"formats":["mermaid","plantuml","d2","math"],"isolatedEngineError":true,"sourceReadable":true,"sourceCopy":true,"themeReplay":true,"previewBudget":true,"modeSwitchCleanup":true,"fileUnchanged":true,"experimentalAdapter":true,"viewportAdmission":viewport,"sequentialRealFormats":true,"mermaidMathLabels":[3,7]}),
        )
    }
}
