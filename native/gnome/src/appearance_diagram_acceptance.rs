use super::*;
use std::{
    path::Path,
    time::{Duration, Instant},
};
async fn wait(label: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let at = Instant::now();
    while !test() {
        if at.elapsed() > Duration::from_secs(20) {
            return Err(format!("Diagram preferences timed out: {label}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
impl Panel {
    pub async fn smoke_diagram_styles(
        self: &Rc<Self>,
        renderer: &Rc<crate::chat_diagrams::Renderer>,
        reader: &Rc<crate::reader::Reader>,
        directory: &Path,
    ) -> Result<Value, String> {
        wait("initial settings", || {
            !self.busy.get() && self.snapshot.borrow().is_some()
        })
        .await?;
        let source = "flowchart LR\n A[Native 中文] --> B[Second node]\n A --> C[Third node]\n";
        let page = adw::PreferencesPage::builder()
            .title("Diagram sample")
            .build();
        let group = adw::PreferencesGroup::builder()
            .title("Saved Mermaid preferences")
            .build();
        let diagram = renderer.widget("mermaid", source, crate::content_widgets::Styles::new());
        group.add(&diagram);
        page.add(&group);
        self.dialog.add(&page);
        let original_style = self.diagram_style.selected();
        let original_layout = self.diagram_layout.selected();
        async fn save(
            panel: &Rc<Panel>,
            size: f64,
            density: u32,
            curve: u32,
        ) -> Result<(), String> {
            let before = diagram_style();
            panel.diagram_size.set_value(size);
            panel.diagram_density.set_selected(density);
            panel.diagram_curve.set_selected(curve);
            if diagram_style() != before {
                return Err("Diagram style applied before Save".into());
            }
            if !panel.dirty.get() {
                return Ok(());
            }
            panel.save.emit_clicked();
            wait("Save", || !panel.busy.get()).await?;
            if panel.dirty.get() {
                return Err(format!(
                    "Diagram preference save failed: {}",
                    panel.status.text()
                ));
            }
            Ok(())
        }
        self.dialog.set_visible_page(&self.controls_page);
        self.diagram_style.set_selected(1); // Classic/Dagre baseline for curve checks.
        self.diagram_layout.set_selected(1);
        save(self, 12.0, 0, 1).await?;
        self.dialog.set_visible_page(&page);
        wait("small rasters", || {
            renderer.smoke_style_state(&diagram).is_some()
                && reader.smoke_diagram_signature().is_some()
        })
        .await?;
        let small = renderer.smoke_style_state(&diagram).unwrap();
        let small_reader = reader.smoke_diagram_signature().unwrap();
        if small["width"] != small_reader["width"] || small["pixels"] != small_reader["pixels"] {
            return Err("Reader and conversation diagrams did not use the same profile".into());
        }
        self.smoke_capture(directory, "diagram-profile-small.png")
            .await?;
        let editor = reader.smoke_diagram_dirty_begin().await?;
        self.dialog.set_visible_page(&self.controls_page);
        save(self, 20.0, 2, 2).await?;
        reader.smoke_diagram_dirty_check(&editor)?;
        self.dialog.set_visible_page(&page);
        wait("large raster", || {
            renderer
                .smoke_style_state(&diagram)
                .is_some_and(|v| v["serial"] != small["serial"])
        })
        .await?;
        let large = renderer.smoke_style_state(&diagram).unwrap();
        if !(large["width"].as_i64() > small["width"].as_i64()
            && large["height"].as_i64() > small["height"].as_i64()
            && large["pixels"] != small["pixels"])
        {
            return Err(format!(
                "Saved diagram settings did not change the real raster: {small}; {large}"
            ));
        }
        renderer.smoke_stale_style_reply(small["serial"].as_u64().unwrap());
        if renderer.smoke_style_state(&diagram).unwrap() != large {
            return Err("An old style result replaced the current diagram".into());
        }
        reader.smoke_diagram_preview();
        wait("reader refresh after Edit", || {
            reader
                .smoke_diagram_signature()
                .is_some_and(|v| v["pixels"] == large["pixels"])
        })
        .await?;
        reader.smoke_diagram_dirty_check(&editor)?;
        self.smoke_capture(directory, "diagram-profile-large.png")
            .await?;
        self.dialog.set_visible_page(&self.controls_page);
        save(self, 20.0, 2, 1).await?;
        self.dialog.set_visible_page(&page);
        wait("curve-only change", || {
            renderer
                .smoke_style_state(&diagram)
                .is_some_and(|v| v["pixels"] != large["pixels"])
        })
        .await?;
        let straight = renderer.smoke_style_state(&diagram).unwrap();
        self.dialog.set_visible_page(&self.controls_page);
        save(self, 12.0, 0, 1).await?;
        self.dialog.set_visible_page(&page);
        wait("original raster restored", || {
            renderer
                .smoke_style_state(&diagram)
                .is_some_and(|v| v["pixels"] == small["pixels"])
        })
        .await?;
        self.dialog.set_visible_page(&self.controls_page);
        self.fonts[1].family.set_text("\"DejaVu Serif\", serif");
        self.save.emit_clicked();
        wait("font family Save", || !self.busy.get()).await?;
        self.dialog.set_visible_page(&page);
        wait("font-only redraw", || {
            renderer
                .smoke_style_state(&diagram)
                .is_some_and(|v| v["pixels"] != small["pixels"])
        })
        .await?;
        let family = renderer.smoke_style_state(&diagram).unwrap();
        if diagram_style().font_family != "\"DejaVu Serif\",serif" {
            return Err("Diagram did not receive reading font families".into());
        }
        self.dialog.set_visible_page(&self.controls_page);
        self.fonts[1].family.set_text("\"DejaVu Sans\", sans-serif");
        self.save.emit_clicked();
        wait("font restore", || !self.busy.get()).await?;
        save(self, 20.0, 2, 2).await?;
        self.dialog.set_visible_page(&page);
        wait("final profile", || {
            renderer
                .smoke_style_state(&diagram)
                .is_some_and(|v| v["pixels"] == large["pixels"])
                && reader
                    .smoke_diagram_signature()
                    .is_some_and(|v| v["pixels"] == large["pixels"])
        })
        .await?;
        reader.smoke_diagram_dirty_check(&editor)?;
        let mut looks = Vec::new();
        for selected in [0, 2, 3, 4] {
            self.dialog.set_visible_page(&self.controls_page);
            self.diagram_style.set_selected(selected);
            save(self, 20.0, 2, 2).await?;
            self.dialog.set_visible_page(&page);
            wait("style-only redraw", || {
                renderer
                    .smoke_style_state(&diagram)
                    .is_some_and(|v| v["pixels"] != large["pixels"])
            })
            .await?;
            let actual = renderer.smoke_style_state(&diagram).unwrap();
            wait("reader style parity", || {
                reader
                    .smoke_diagram_signature()
                    .is_some_and(|v| v["pixels"] == actual["pixels"])
            })
            .await?;
            reader.smoke_diagram_dirty_check(&editor)?;
            self.smoke_capture(directory, &format!("diagram-style-{selected}.png"))
                .await?;
            looks.push(json!({"selected":selected,"signature":actual}));
        }
        self.dialog.set_visible_page(&self.controls_page);
        self.diagram_style.set_selected(1);
        self.diagram_layout.set_selected(2);
        save(self, 20.0, 2, 2).await?;
        self.dialog.set_visible_page(&page);
        wait("ELK redraw", || {
            renderer
                .smoke_style_state(&diagram)
                .is_some_and(|v| v["pixels"] != large["pixels"])
        })
        .await?;
        let elk = renderer.smoke_style_state(&diagram).unwrap();
        wait("reader ELK parity", || {
            reader
                .smoke_diagram_signature()
                .is_some_and(|v| v["pixels"] == elk["pixels"])
        })
        .await?;
        self.smoke_capture(directory, "diagram-layout-elk.png")
            .await?;
        reader.smoke_diagram_dirty_check(&editor)?;
        self.dialog.set_visible_page(&self.controls_page);
        self.diagram_layout.set_selected(0);
        save(self, 20.0, 2, 2).await?;
        self.dialog.set_visible_page(&page);
        wait("automatic layout redraw", || {
            renderer
                .smoke_style_state(&diagram)
                .is_some_and(|v| v["serial"] != elk["serial"])
        })
        .await?;
        let automatic = renderer.smoke_style_state(&diagram).unwrap();
        if diagram_style().layout != "auto" {
            return Err("Automatic layout was not saved".into());
        }
        wait("automatic reader parity", || {
            reader
                .smoke_diagram_signature()
                .is_some_and(|v| v["pixels"] == automatic["pixels"])
        })
        .await?;
        self.dialog.set_visible_page(&self.controls_page);
        self.diagram_style.set_selected(original_style);
        self.diagram_layout.set_selected(original_layout);
        save(self, 20.0, 2, 2).await?;
        self.dialog.set_visible_page(&page);
        wait("original look/layout restored", || {
            let chat = renderer.smoke_style_state(&diagram);
            chat.as_ref().is_some_and(|v| {
                reader
                    .smoke_diagram_signature()
                    .is_some_and(|r| r["pixels"] == v["pixels"])
            })
        })
        .await?;
        self.dialog.set_visible_page(&self.controls_page);
        self.smoke_capture(directory, "diagram-controls.png")
            .await?;
        let source_file = directory.join("project/diagram-settings.md");
        let bytes = gtk::gio::spawn_blocking(move || {
            std::fs::read_to_string(source_file).map_err(|e| e.to_string())
        })
        .await
        .map_err(|_| "Fixture file worker stopped")??;
        if bytes.contains("Unsaved diagram note") {
            return Err("Diagram settings saved the editor implicitly".into());
        }
        reader.smoke_diagram_discard();
        self.dialog.set_visible_page(&self.controls_page);
        self.dialog.remove(&page);
        Ok(
            json!({"explicitSave":true,"realRasters":true,"curveOnly":true,"fontOnly":true,"styleOnly":true,"layoutOnly":true,"readerAndConversation":true,"sourceEditsRetained":true,"hiddenViewRefresh":true,"staleReplyRejected":true,"originalRestored":true,"small":small,"large":large,"straight":straight,"family":family,"looks":looks,"elk":elk,"automatic":automatic}),
        )
    }
}
