//! Actual GTK allocations in a private Wayland application, not CSS/source snapshots.
use super::*;
use std::path::Path;

async fn until(test: impl Fn() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !test() {
        if start.elapsed() > Duration::from_secs(12) {
            return Err("Layout fixture did not become ready".into());
        }
        glib::timeout_future(Duration::from_millis(25)).await;
    }
    Ok(())
}
fn geometry(widget: &impl IsA<gtk::Widget>, relative: &impl IsA<gtk::Widget>) -> Value {
    let widget = widget.as_ref();
    let bounds = widget.compute_bounds(relative);
    let (minimum, natural, _, _) = widget.measure(gtk::Orientation::Horizontal, -1);
    json!({"type":widget.type_().name(),"name":widget.widget_name().as_str(),"width":widget.width(),"height":widget.height(),"minimum":minimum,"natural":natural,"bounds":bounds.map(|r|json!({"x":r.x(),"y":r.y(),"width":r.width(),"height":r.height()}))})
}
fn walk(widget: &gtk::Widget, accept: &mut impl FnMut(&gtk::Widget)) {
    accept(widget);
    let mut child = widget.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        walk(&current, accept);
    }
}
async fn model_popup(ui: &Rc<Ui>, name: &str, directory: &Path) -> Result<Value, String> {
    ui.model.emit_activate();
    let mut popup = None;
    walk(ui.model.upcast_ref(), &mut |w| {
        if let Some(p) = w.downcast_ref::<gtk::Popover>() {
            popup = Some(p.clone());
        }
    });
    let popup = popup.ok_or("Native model popup missing")?;
    until(|| popup.is_mapped()).await?;
    glib::timeout_future(Duration::from_millis(60)).await;
    popup.present();
    let child = popup
        .first_child()
        .ok_or("Native model popup content missing")?;
    child.queue_allocate();
    popup.allocate(popup.width(), popup.height(), -1, None);
    let mut full = None;
    let mut checked = false;
    walk(child.upcast_ref(), &mut |widget| {
        if let Some(label) = widget.downcast_ref::<gtk::Label>() {
            if label.text() == name && label.is_mapped() {
                full = Some(label.clone());
            }
        }
        if let Some(image) = widget.downcast_ref::<gtk::Image>() {
            if image.icon_name().as_deref() == Some("object-select-symbolic")
                && image.opacity() > 0.9
                && image.compute_bounds(&popup).is_some_and(|r| {
                    r.y() >= 0.0
                        && r.y() + r.height() <= popup.height() as f32
                        && r.x() >= 0.0
                        && r.x() + r.width() <= popup.width() as f32
                })
            {
                checked = true;
            }
        }
    });
    let label = full.ok_or("Full model name was not rendered in the popup")?;
    if !label.wraps() || label.layout().is_ellipsized() || !checked {
        return Err("Model popup lost wrapped text or its selection marker".into());
    }
    let snapshot = gtk::Snapshot::new();
    popup.snapshot_child(&child, &snapshot);
    let node = snapshot
        .to_node()
        .ok_or("Model popup render node missing")?;
    popup
        .renderer()
        .ok_or("Model popup renderer missing")?
        .render_texture(&node, None)
        .save_to_png(directory.join("layout-model-popup.png"))
        .map_err(|e| e.to_string())?;
    let result = json!({"fullName":true,"wrapped":true,"selectionMarker":checked,"width":popup.width(),"height":popup.height()});
    popup.popdown();
    Ok(result)
}
fn context(ui: &Rc<Ui>, directory: &Path) -> Result<(), String> {
    ui.finder.widget.set_visible(true);
    let mut refs = crate::composer_refs::Book::default();
    refs.add_prepared("layout-ui-only",(0..3).map(|i|Arc::new(crate::composer_refs::Reference{id:format!("layout-ref-{i}"),label:format!("Reference-{i}-{}", "long title 中文😀".repeat(20)),input:json!({"type":"skill","name":format!("ref{i}"),"path":format!("/private/fixture/ref{i}")}),bytes:512})).collect())?;
    ui.composer_tools.render_refs("layout-ui-only", &refs);
    let mut queued = crate::message_queue::Entry::new(
        crate::message_queue::Context {
            backend: "codex".into(),
            thread: "layout-ui-only".into(),
            root: directory.to_string_lossy().into(),
            kind: "codex".into(),
            generation: 1,
        },
        1,
    );
    queued.loaded = true;
    queued
        .messages
        .push(Arc::new(crate::message_queue::Message {
            id: "layout-message".into(),
            text: "Queued layout context 中文😀".repeat(10),
            input: Arc::new(vec![]),
            created_at: 1,
            bytes: 512,
        }));
    ui.queue.render("layout-ui-only", Some(&queued));
    ui.router_file_button.set_visible(true);
    ui.router_target_label
        .set_text(&format!("Target {}", "long route label 中文😀".repeat(20)));
    ui.router_target.set_visible(true);
    Ok(())
}

pub(super) async fn run(
    ui: &Rc<Ui>,
    directory: &Path,
    capture: impl Fn(&str),
) -> Result<Value, String> {
    until(|| {
        ui.state.borrow().ready
            && !ui.state.borrow().catalog_loading
            && ui
                .state
                .borrow()
                .threads
                .iter()
                .any(|t| t.id == "native-smoke-thread")
    })
    .await?;
    let thread = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|t| t.id == "native-smoke-thread")
        .cloned()
        .unwrap();
    ui.select(thread);
    until(|| !ui.state.borrow().loading && ui.state.borrow().options_loaded).await?;
    ui.composer.buffer().set_text("Unsent layout draft 中文😀");
    ui.split.set_show_sidebar(false);
    ui.reader.open_path(&directory.join("project/report.md"));
    ui.show_tool("reader");
    until(|| ui.reader.is_ready()).await?;
    let card = ui
        .composer
        .parent()
        .and_then(|p| p.parent())
        .ok_or("Composer card missing")?;
    let mut samples = Vec::new();
    let mut failures = Vec::new();
    let mut popup_result = Value::Null;
    let long_name = format!("native-model-{}", "VeryLongModelName中文😀".repeat(12));
    for phase in ["normal", "long-names", "context", "queued", "large-type"] {
        if phase == "long-names" {
            ui.state.borrow_mut().updating_options = true;
            ui.model
                .set_model(Some(&gtk::StringList::new(&["Default model", &long_name])));
            ui.model.set_selected(1);
            ui.state.borrow_mut().updating_options = false;
        }
        if phase == "context" {
            ui.finder.open();
        }
        if phase == "queued" {
            for index in 0..3 {
                ui.composer.buffer().set_text(&format!("Paused layout message {index} 中文😀 · enough text to exercise a narrow conversation pane"));
                ui.enqueue()?;
                ui.pause_selected_queue("Layout fixture keeps queued messages paused");
                until(|| {
                    ui.state
                        .borrow()
                        .queues
                        .entries
                        .values()
                        .any(|q| q.paused && q.messages.len() == index + 1 && q.saving.is_none())
                })
                .await?;
            }
            until(|| {
                ui.state
                    .borrow()
                    .queues
                    .entries
                    .values()
                    .any(|q| q.paused && q.messages.len() == 3 && q.saving.is_none())
            })
            .await?;
            ui.composer.buffer().set_text("Unsent layout draft 中文😀");
        }
        if phase == "large-type" {
            let snapshot:crate::appearance::Snapshot=serde_json::from_value(json!({"revision":"c".repeat(64),"values":{"contentWidth":"wide","typography":{"uiFontFamily":"DejaVu Sans","uiFontSize":20,"uiFontWeight":400,"contentFontSize":24,"workspaceFontFamily":"DejaVu Sans","workspaceFontSize":20,"codeFontFamily":"monospace","codeFontSize":20,"codeFontWeight":400,"highContrast":false}},"fonts":{"ui":["DejaVu Sans"],"content":["DejaVu Sans"],"workspace":["DejaVu Sans"],"code":["monospace"]}})).map_err(|e|e.to_string())?;
            let plan = snapshot.plan()?;
            ui.appearance
                .apply(&crate::appearance::Reply { snapshot, plan });
        }
        for position in [600, 420, 320, 960] {
            ui.workspace.set_position(position);
            glib::timeout_future(Duration::from_millis(80)).await;
            if matches!(phase, "context" | "queued" | "large-type") {
                context(ui, directory)?;
            }
            capture(&format!("layout-{phase}-{position}.png"));
            let mut rows = Vec::new();
            let mut offenders = Vec::new();
            walk(ui.transcript_view.upcast_ref(), &mut |widget| {
                if let Some(clamp) = widget.downcast_ref::<adw::Clamp>() {
                    if widget.is_mapped() {
                        if let Some(child) = clamp.child() {
                            rows.push(geometry(&child, &ui.workspace));
                        }
                    }
                }
            });
            walk(ui.conversation.upcast_ref(), &mut |widget| {
                if !widget.is_mapped() || offenders.len() >= 100 {
                    return;
                }
                if let Some(parent) = widget.parent() {
                    if let Some(bounds) = widget.compute_bounds(&parent) {
                        if bounds.x() < -1.0
                            || bounds.x() + bounds.width() > parent.width() as f32 + 1.0
                        {
                            offenders.push(geometry(widget, &parent));
                        }
                    }
                }
            });
            if ui
                .conversation
                .compute_bounds(&ui.workspace)
                .is_none_or(|r| {
                    r.x() < 0.0 || r.x() + r.width() > ui.workspace.position() as f32 + 1.0
                })
            {
                failures.push(format!("Conversation pane clipped at {phase}/{position}"));
            }
            let composer = geometry(&card, &ui.workspace);
            if composer["bounds"]["x"].as_f64().unwrap_or(-1.0) < 0.0 {
                failures.push(format!("Composer clipped at {phase}/{position}"));
            }
            if rows
                .iter()
                .any(|r| r["bounds"]["x"].as_f64().unwrap_or(-1.0) < 0.0)
            {
                failures.push(format!("Conversation clipped at {phase}/{position}"));
            }
            let buttons = [
                ("send", ui.send.upcast_ref::<gtk::Widget>()),
                ("stop", ui.stop.upcast_ref()),
                ("queue", ui.queue_button.upcast_ref()),
                ("continue", ui.continue_button.upcast_ref()),
                ("model", ui.model.upcast_ref()),
                ("effort", ui.effort.upcast_ref()),
                ("fast", ui.fast.upcast_ref()),
                ("attach", ui.image_button.upcast_ref()),
                ("router-file", ui.router_file_button.upcast_ref()),
                ("router-target", ui.router_target.upcast_ref()),
            ]
            .into_iter()
            .map(|(name, widget)| {
                let g = geometry(widget, &ui.workspace);
                if widget.is_visible()
                    && widget.compute_bounds(&ui.workspace).is_none_or(|r| {
                        r.x() < 0.0
                            || r.x() + r.width() > ui.workspace.position() as f32 + 1.0
                            || r.y() < 0.0
                            || r.y() + r.height() > ui.workspace.height() as f32 + 1.0
                    })
                {
                    failures.push(format!("{name} clipped at {phase}/{position}"));
                }
                (name.into(), g)
            })
            .collect::<serde_json::Map<_, _>>();
            if matches!(phase, "context" | "large-type")
                && !(ui.finder.widget.is_visible()
                    && ui.queue.widget.is_visible()
                    && ui.composer_tools.widget.is_visible()
                    && ui.router_target.is_visible())
            {
                return Err("Layout context controls were not exercised".into());
            }
            let mut heading_font = None;
            let mut heading_body_pixels = None;
            if phase == "large-type" {
                walk(ui.reader.widget.upcast_ref(), &mut |widget| {
                    if widget.has_css_class("title-1") {
                        heading_font = widget.pango_context().font_description();
                    }
                    if let Some(view) = widget.downcast_ref::<gtk::TextView>() {
                        let buffer = view.buffer();
                        let start = buffer.start_iter();
                        if buffer
                            .text(
                                &start,
                                &buffer.iter_at_offset(buffer.char_count().min(13)),
                                false,
                            )
                            .as_str()
                            == "Layout report"
                        {
                            if let Some(tag) = start
                                .tags()
                                .into_iter()
                                .find(|tag| tag.weight() >= 700 && tag.scale() > 1.0)
                            {
                                if let Some(mut font) = view.pango_context().font_description() {
                                    let factor = if font.is_size_absolute() {
                                        1.0
                                    } else {
                                        96.0 / 72.0
                                    };
                                    heading_body_pixels = Some(
                                        font.size() as f64 / gtk::pango::SCALE as f64 * factor,
                                    );
                                    font.set_weight(gtk::pango::Weight::Bold);
                                    let size = font.size() as f64 * tag.scale();
                                    if font.is_size_absolute() {
                                        font.set_absolute_size(size);
                                    } else {
                                        font.set_size(size.round() as i32);
                                    }
                                    heading_font = Some(font);
                                }
                            }
                        }
                    }
                });
                if heading_font.as_ref().is_none_or(|font| {
                    let pixels = font.size() as f64 / gtk::pango::SCALE as f64
                        * if font.is_size_absolute() {
                            1.0
                        } else {
                            96.0 / 72.0
                        };
                    font.weight() != gtk::pango::Weight::Bold
                        || match heading_body_pixels {
                            Some(body) => (body - 24.0).abs() > 0.5 || pixels < body * 1.25,
                            None => (pixels - 48.0).abs() > 0.5,
                        }
                }) {
                    return Err(format!(
                        "Reading typography lost Markdown heading hierarchy: body={heading_body_pixels:?}, heading={heading_font:?}"
                    ));
                }
            }
            let reader = geometry(&ui.reader.widget, &ui.workspace);
            if ui
                .reader
                .widget
                .compute_bounds(&ui.workspace)
                .is_none_or(|r| {
                    r.x() < ui.workspace.position() as f32
                        || r.x() + r.width() > ui.workspace.width() as f32 + 1.0
                })
            {
                failures.push(format!("Reader clipped at {phase}/{position}"));
            }
            if phase != "normal"
                && ui
                    .model
                    .selected_item()
                    .and_downcast::<gtk::StringObject>()
                    .is_none_or(|o| o.string() != long_name)
            {
                return Err("Responsive model selector truncated the stored display value".into());
            }
            samples.push(json!({"phase":phase,"headingBodyPixels":heading_body_pixels,"headingFont":heading_font.map(|f|f.to_string()),"contextVisible":matches!(phase,"context"|"large-type"),"reader":reader,"requested":position,"effective":ui.workspace.position(),"minimumPosition":ui.workspace.min_position(),"maximumPosition":ui.workspace.max_position(),"conversation":geometry(&ui.conversation,&ui.window),"composer":composer,"transcript":geometry(&ui.transcript_view,&ui.conversation),"horizontalValue":ui.transcript_scroll.hadjustment().value(),"horizontalUpper":ui.transcript_scroll.hadjustment().upper(),"rows":rows,"controls":buttons,"offenders":offenders}));
            if phase == "long-names" && position == 320 {
                popup_result = model_popup(ui, &long_name, directory).await?;
            }
        }
    }
    if ui.composer_text() != "Unsent layout draft 中文😀" {
        return Err("Resizing lost the unsent draft".into());
    }
    let probing = std::env::var_os("STUDIO_GNOME_LAYOUT_PROBE").is_some();
    Ok(
        json!({"ok":failures.is_empty()||probing,"probeOnly":probing,"samples":samples,"failures":failures,"draftRetained":true,"longModelValueRetained":true,"modelPopup":popup_result}),
    )
}
