//! Measure actual mapped GTK layout after the normal appearance Save path.
use super::*;
use std::time::{Duration, Instant};
impl Panel {
    pub(super) async fn smoke_markdown_modes(
        self: &Rc<Self>,
        directory: &std::path::Path,
        customized: bool,
    ) -> Result<Value, String> {
        let probe = inline_acceptance::Probe::new().await?;
        let controls = self.controls_page.clone();
        let sample = probe.page();
        self.dialog.add(&sample);
        let mut measured = serde_json::Map::new();
        let mut text = None;
        for (index, name, mode, spacing) in [
            (1, "reading", appearance::MarkdownMode::Reading, 12),
            (2, "compact", appearance::MarkdownMode::Compact, 4),
            (0, "technical", appearance::MarkdownMode::Technical, 8),
        ] {
            self.dialog.set_visible_page(&controls);
            let previous = self.runtime.plan.borrow().as_ref().unwrap().markdown;
            self.markdown.set_selected(index);
            if !self.dirty.get()
                || self.runtime.plan.borrow().as_ref().unwrap().markdown != previous
            {
                return Err("Markdown mode applied without Save".into());
            }
            self.save.emit_clicked();
            let start = Instant::now();
            while self.busy.get() {
                if start.elapsed() > Duration::from_secs(15) {
                    return Err("Markdown layout save timed out".into());
                }
                glib::timeout_future(Duration::from_millis(20)).await;
            }
            if self.dirty.get() || self.runtime.plan.borrow().as_ref().unwrap().markdown != mode {
                return Err(format!("Markdown mode save failed: {}", self.status.text()));
            }
            self.dialog.set_visible_page(&sample);
            glib::timeout_future(Duration::from_millis(60)).await;
            self.smoke_capture(
                directory,
                &format!(
                    "markdown-{name}-{}.png",
                    if customized { "custom" } else { "system" }
                ),
            )
            .await?;
            let mut geometry = probe.geometry()?;
            let page = geometry
                .as_object_mut()
                .unwrap()
                .remove("pageText")
                .unwrap();
            if geometry["textViewLineAdvance"].as_f64().unwrap_or(0.0)
                < geometry["fontPixels"].as_f64().unwrap_or(0.0) * mode.line_height() as f64 - 1.0
            {
                return Err(format!("Paged paragraph ignored line spacing: {geometry}"));
            }
            if geometry["top"] != spacing || geometry["bottom"] != spacing {
                return Err(format!("Markdown spacing did not apply: {geometry}"));
            }

            if text.as_ref().is_some_and(|before| before != &page) {
                return Err("Mode changed the rendered paragraph page/text".into());
            }
            text = Some(page);
            if customized {
                probe.verify(true)?;
            }
            measured.insert(name.into(), geometry);
        }
        for field in ["fontPixels", "labelLineHeight", "textViewLineAdvance"] {
            let number = |mode: &str| measured[mode][field].as_f64().unwrap_or(0.0);
            if !(number("reading") > number("technical") && number("technical") > number("compact"))
            {
                return Err(format!(
                    "Markdown layout did not change {field}: {measured:?}"
                ));
            }
        }
        self.dialog.set_visible_page(&controls);
        self.dialog.remove(&sample);
        Ok(json!({"geometry":measured,"pageTextRetained":true,"explicitSave":true}))
    }
}
