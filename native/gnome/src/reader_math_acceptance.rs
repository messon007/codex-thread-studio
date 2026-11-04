//! Display math uses the real Rust worker, shared cache and native image rows.
use super::*;
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !test() {
        if start.elapsed() > Duration::from_secs(15) {
            return Err(format!("Native math timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(30)).await;
    }
    Ok(())
}
impl Reader {
    /// Mount only the private fixture's table through the real reader adapter.
    /// Its coordinates come from the current file; parsing stays on the worker.
    pub async fn smoke_flow_math_table(self: &Rc<Self>) -> Result<crate::text_flow::View, String> {
        let (generation, path, revision) = {
            let state = self.state.borrow();
            (
                state.generation,
                state.path.clone().ok_or("No math fixture file")?,
                state
                    .index
                    .as_ref()
                    .ok_or("No math fixture index")?
                    .revision
                    .clone(),
            )
        };
        let prepared = gio::spawn_blocking(move || {
            use std::io::Read;
            let mut source = String::new();
            std::fs::File::open(path)
                .map_err(|e| e.to_string())?
                .take(65537)
                .read_to_string(&mut source)
                .map_err(|e| e.to_string())?;
            if source.len() > 65536 {
                return Err("Math table fixture exceeds its limit".into());
            }
            let start = source
                .find("| Expression |")
                .ok_or("Math table fixture missing")?;
            let line = source[..start].bytes().filter(|c| *c == b'\n').count() + 1;
            let fragment = studio_markdown_native::Fragment {
                page: 0,
                document_id: "inline-table-acceptance".into(),
                revision,
                metadata: studio_markdown_native::FragmentMetadata {
                    start,
                    end: source.len(),
                    line,
                    rows: 3,
                    row_start: 0,
                    table: None,
                },
                source: studio_markdown_native::SourceWindow {
                    start,
                    end: source.len(),
                    line,
                    end_line: source.lines().count(),
                    text: source[start..].to_owned(),
                    projection: vec![],
                    references: vec![],
                },
                header: None,
            };
            crate::text_flow::prepare(&fragment)
        })
        .await
        .map_err(|_| "Math table projection worker stopped")??;
        if self.state.borrow().generation != generation {
            return Err("Math fixture changed while preparing".into());
        }
        let view = self.flow_view(prepared)?;
        self.preview_scroll.set_child(Some(&view.widget));
        // This probe replaces a reader already scrolled through other content.
        // Test visible math at the new table's start, rather than relying on
        // rendering every mapped offscreen label from the previous allocation.
        self.preview_scroll
            .vadjustment()
            .set_value(self.preview_scroll.vadjustment().lower());
        self.preview_scroll
            .hadjustment()
            .set_value(self.preview_scroll.hadjustment().lower());
        Ok(view)
    }
    pub fn smoke_restore_block_preview(&self) {
        self.preview_scroll.set_child(Some(&self.preview));
    }
    pub(super) async fn smoke_math(
        self: &Rc<Self>,
        root: &Path,
        capture: impl Fn(&Path),
    ) -> Result<(), String> {
        let style = adw::StyleManager::default();
        let original = style.color_scheme();
        style.set_color_scheme(adw::ColorScheme::ForceLight);
        let started = std::time::Instant::now();
        let ticks = Rc::new(Cell::new(0_u64));
        let tick_count = ticks.clone();
        let timer = glib::timeout_add_local(Duration::from_millis(16), move || {
            tick_count.set(tick_count.get() + 1);
            glib::ControlFlow::Continue
        });
        self.open_path(&root.join("project/math.md"));
        let loaded = until("reader formulas", || {
            self.state.borrow().diagrams.len() == 3
                && self
                    .state
                    .borrow()
                    .diagrams
                    .values()
                    .all(|d| d.kind == "math" && d.picture.paintable().is_some())
        })
        .await;
        timer.remove();
        loaded?;
        let open_ms = started.elapsed().as_millis();
        let signatures = || {
            self.state
                .borrow()
                .diagrams
                .values()
                .map(|d| (d.id, d.picture.smoke_signature()))
                .collect::<std::collections::BTreeMap<_, _>>()
        };
        let light = signatures();
        if light.values().any(Option::is_none) {
            return Err("Math did not produce a native texture".into());
        }
        glib::timeout_future(Duration::from_millis(80)).await;
        capture(&root.join("math-light.png"));
        style.set_color_scheme(adw::ColorScheme::ForceDark);
        until("theme recolors formulas", || {
            let current = signatures();
            current.len() == 3
                && current
                    .iter()
                    .all(|(id, value)| value.is_some() && light.get(id) != Some(value))
        })
        .await?;
        glib::timeout_future(Duration::from_millis(80)).await;
        capture(&root.join("math-dark.png"));
        self.set_mode(Mode::Source);
        until("math source", || {
            !self.state.borrow().busy && self.state.borrow().source.is_some()
        })
        .await?;
        let source = self.source_view.buffer();
        let text = source.text(&source.start_iter(), &source.end_iter(), false);
        if !text.contains("$$") || !text.contains("\\frac") || !text.contains("```latex") {
            return Err("Math source lost its original delimiters".into());
        }
        self.set_mode(Mode::Preview);
        until("math preview restored", || {
            !self.state.borrow().busy
                && self.state.borrow().mode == Mode::Preview
                && self.state.borrow().diagrams.len() == 3
                && self
                    .state
                    .borrow()
                    .diagrams
                    .values()
                    .all(|d| d.picture.paintable().is_some())
        })
        .await?;
        self.set_mode(Mode::Edit);
        until("math edit source", || {
            !self.state.borrow().busy && self.state.borrow().editor.is_some()
        })
        .await?;
        let source = self.edit_view.buffer();
        let text = source.text(&source.start_iter(), &source.end_iter(), false);
        if !text.contains("\\begin{pmatrix}") || self.state.borrow().dirty {
            return Err("Math Edit changed original TeX".into());
        }
        self.finish_close();
        self.open_path(&root.join("project/math-invalid.md"));
        until("invalid formula is visible", || {
            self.state.borrow().diagrams.values().any(|d| {
                d.label.text().contains("Undefined control sequence")
                    && d.picture.paintable().is_none()
            })
        })
        .await?;
        capture(&root.join("math-invalid.png"));
        self.finish_close();
        style.set_color_scheme(original);
        std::fs::write(root.join("math.json"),serde_json::to_vec_pretty(&serde_json::json!({"ok":true,"workerImages":true,"displayAndFence":true,"darkTheme":true,"sourceEditPreserved":true,"invalidReported":true,"textures":light,"openMs":open_ms,"openingTicks":ticks.get()})).unwrap()).map_err(|e|e.to_string())?;
        Ok(())
    }
}
