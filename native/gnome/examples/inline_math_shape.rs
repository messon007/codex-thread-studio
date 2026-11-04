//! Owned Wayland probe of production shapes, source Copy and multiline limits.
#![allow(dead_code)]
#[path = "../src/diagram.rs"]
mod diagram;
#[path = "../src/diagram_style.rs"]
mod diagram_style;
#[path = "../src/engine_process.rs"]
mod engine_process;
#[path = "../src/inline_math_shape.rs"]
#[cfg(target_os = "linux")]
mod inline_math_shape;
#[path = "../src/math.rs"]
mod math;
#[path = "../src/mermaid_math.rs"]
mod mermaid_math;
#[path = "../src/smoke_input.rs"]
#[cfg(target_os = "linux")]
mod smoke_input;
#[cfg(target_os = "linux")]
mod gui {
    use super::*;
    use adw::prelude::*;
    use gtk::{gio, glib};
    use serde_json::{json, Value};
    use std::{
        cell::Cell,
        path::{Path, PathBuf},
        rc::Rc,
        sync::{atomic::AtomicBool, Arc},
        time::{Duration, Instant},
    };

    async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
        let at = Instant::now();
        while !test() {
            if at.elapsed() > Duration::from_secs(15) {
                return Err(format!("Inline shape probe timed out: {step}"));
            }
            glib::timeout_future(Duration::from_millis(20)).await;
        }
        Ok(())
    }
    #[allow(deprecated)]
    fn capture(window: &adw::ApplicationWindow, path: &Path) -> Result<(), String> {
        let content =
            gtk::prelude::GtkWindowExt::child(window).ok_or("No native window content")?;
        let paintable = gtk::WidgetPaintable::new(Some(&content));
        let snapshot = gtk::Snapshot::new();
        let background = window
            .style_context()
            .lookup_color("window_bg_color")
            .unwrap_or_else(|| {
                if adw::StyleManager::default().is_dark() {
                    gtk::gdk::RGBA::new(0.14, 0.14, 0.14, 1.0)
                } else {
                    gtk::gdk::RGBA::new(0.98, 0.98, 0.98, 1.0)
                }
            });
        snapshot.append_color(
            &background,
            &gtk::graphene::Rect::new(0.0, 0.0, content.width() as f32, content.height() as f32),
        );
        paintable.snapshot(
            &snapshot,
            f64::from(content.width()),
            f64::from(content.height()),
        );
        let node = snapshot.to_node().ok_or("No native render node")?;
        window
            .renderer()
            .ok_or("No native renderer")?
            .render_texture(&node, None)
            .save_to_png(path)
            .map_err(|e| e.to_string())
    }
    async fn multiline_probe(
        window: &adw::ApplicationWindow,
        root: &Path,
    ) -> Result<Value, String> {
        let mut cases = Vec::new();
        for (name, input) in [
            ("lf", "Before $\\frac{\na}{b}$ after."),
            ("crlf", "Before $\\frac{\r\na}{b}$ after."),
            ("outside", "Before $\\frac{\na}{b}$ after.\nOutside line."),
        ] {
            let (content, formulas) = gio::spawn_blocking(move || {
                let content = studio_markdown_native::parse_text(input)?
                    .into_iter()
                    .find_map(|b| match b.kind {
                        studio_markdown_native::BlockKind::Paragraph(c) => Some(c),
                        _ => None,
                    })
                    .ok_or("No multiline paragraph")?;
                if content.math.len() != 1 {
                    return Err("Missing multiline candidate".into());
                }
                let formulas = content
                    .math
                    .iter()
                    .map(|m| {
                        diagram::render_cancellable(
                            "math-inline",
                            &content.plain[m.body.clone()],
                            true,
                            &Default::default(),
                            Arc::new(AtomicBool::new(false)),
                        )
                        .and_then(inline_math_shape::Prepared::new)
                        .map(|p| (m.range.clone(), p))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Ok::<_, String>((content, formulas))
            })
            .await
            .map_err(|_| "Multiline worker stopped")??;
            let label = gtk::Label::builder()
                .selectable(true)
                .wrap(true)
                .xalign(0.0)
                .valign(gtk::Align::Start)
                .margin_top(32)
                .margin_start(24)
                .margin_end(24)
                .build();
            label.set_markup(&content.markup);
            gtk::prelude::GtkWindowExt::set_focus(window, None::<&gtk::Widget>);
            window.set_content(Some(&label));
            let shape_applied = label.text() == content.plain;
            let draws = if shape_applied {
                Some(inline_math_shape::apply(&label, formulas, 20.0)?)
            } else {
                None
            };
            until("multiline source draw", || {
                label.is_mapped() && label.width() > 0 && draws.as_ref().is_none_or(|d| d.get() > 0)
            })
            .await?;
            let span = &content.math[0];
            let a = label.layout().index_to_pos(span.range.start as i32);
            let b = label.layout().index_to_pos(span.range.end as i32);
            let normal_lines = label.layout().line_count();
            let normal_after_delta = b.y() - a.y();
            label.grab_focus();
            label.select_region(0, 0);
            label.select_region(0, -1);
            label.emit_copy_clipboard();
            let copied = smoke_input::request(root, json!({"action":"read"})).await?;
            if copied["text"].as_str() != Some(label.text().as_str()) {
                return Err("Raw-label clipboard differs from its actual text".into());
            }
            let raw_copy_exact = copied["text"].as_str() == Some(content.plain.as_str());
            label.select_region(0, 0);
            glib::timeout_future(Duration::from_millis(80)).await;
            capture(window, &root.join(format!("multiline-{name}-normal.png")))?;
            // Use GTK's public property, never mutate its read-only layout.
            label.set_single_line_mode(true);
            glib::timeout_future(Duration::from_millis(60)).await;
            let single_lines = label.layout().line_count();
            let a = label.layout().index_to_pos(span.range.start as i32);
            let b = label.layout().index_to_pos(span.range.end as i32);
            capture(window, &root.join(format!("multiline-{name}-single.png")))?;
            let mut production_exact = Value::Null;
            if name == "crlf" {
                let formatted = codex_thread_studio_gnome::content_widgets::formatted_at(
                    &content,
                    Rc::new(|_| {}),
                    Rc::new(codex_thread_studio_gnome::content_widgets::ParagraphState::default()),
                );
                gtk::prelude::GtkWindowExt::set_focus(window, None::<&gtk::Widget>);
                window.set_content(Some(&formatted));
                fn text_view(root: &gtk::Widget) -> Option<gtk::TextView> {
                    if let Some(view) = root.downcast_ref::<gtk::TextView>().filter(|v| {
                        v.widget_name() == "studio-multiline-rich-text"
                            && v.is_mapped()
                            && v.width() > 0
                    }) {
                        return Some(view.clone());
                    }
                    let mut child = root.first_child();
                    while let Some(widget) = child {
                        child = widget.next_sibling();
                        if let Some(view) = text_view(&widget) {
                            return Some(view);
                        }
                    }
                    None
                }
                until("production CRLF view", || text_view(&formatted).is_some()).await?;
                let view = text_view(&formatted).unwrap();
                let buffer = view.buffer();
                buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
                view.grab_focus();
                if codex_thread_studio_gnome::content_widgets::selection(view.upcast_ref(), 8192)
                    .as_deref()
                    != Some(content.plain.as_str())
                    || buffer.line_count() != 1
                    || buffer.char_count() >= content.plain.chars().count() as i32
                {
                    return Err(
                        "CRLF formula was not a source-preserving single-line object".into(),
                    );
                }
                view.emit_copy_clipboard();
                let started = Instant::now();
                loop {
                    let reply = smoke_input::request(root, json!({"action":"read"})).await?;
                    if reply["text"].as_str() == Some(content.plain.as_str()) {
                        break;
                    }
                    if started.elapsed() > Duration::from_secs(10) {
                        return Err(format!("Production CRLF Copy differs: {reply}"));
                    }
                    glib::timeout_future(Duration::from_millis(20)).await;
                }
                glib::timeout_future(Duration::from_millis(80)).await;
                capture(window, &root.join("multiline-crlf-production.png"))?;
                production_exact =
                    json!({"textView":true,"copyExact":true,"renderedMath":true,"singleLine":true});
            }
            cases.push(json!({"name":name,"sourceBytes":content.plain.len(),"rawLabelCopyExact":raw_copy_exact,"shapeApplied":shape_applied,"normalLines":normal_lines,
                "normalAfterFormulaDelta":normal_after_delta,"singleLines":single_lines,"singleAfterFormulaDelta":b.y()-a.y(),
                "outsideBreak":name=="outside","production":production_exact}));
        }
        Ok(
            json!({"diagnosticOnly":true,"gtk":format!("{}.{}.{}",gtk::major_version(),gtk::minor_version(),gtk::micro_version()),"pango":gtk::pango::version_string().as_str(),"cases":cases}),
        )
    }
    async fn run(window: &adw::ApplicationWindow, root: &Path) -> Result<Value, String> {
        let input = "Native **中文😀** before $\\frac{a}{b}$, after $\\sum_{i=0}^{n} x_i$, a [safe link](https://example.invalid/math), and $\\color{red}{x^2}$. More surrounding words demonstrate native wrapping and exact selection.";
        let blocks = studio_markdown_native::parse_text(input)?;
        let studio_markdown_native::BlockKind::Paragraph(content) = &blocks[0].kind else {
            return Err("No test paragraph".into());
        };
        if content.math.len() != 3 {
            return Err("Expected three math candidates".into());
        }
        let label = gtk::Label::builder()
            .selectable(true)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .xalign(0.0)
            .valign(gtk::Align::Start)
            .margin_top(32)
            .margin_bottom(32)
            .margin_start(24)
            .margin_end(24)
            .width_chars(1)
            .build();
        label.set_markup(&content.markup);
        let links = Rc::new(Cell::new(0));
        let clicked = links.clone();
        label.connect_activate_link(move |_, uri| {
            if uri == "https://example.invalid/math" {
                clicked.set(clicked.get() + 1);
            }
            glib::Propagation::Stop
        });
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&adw::HeaderBar::new());
        toolbar.set_content(Some(&label));
        window.set_content(Some(&toolbar));
        let mut layouts = Vec::new();
        for dark in [false, true] {
            adw::StyleManager::default().set_color_scheme(if dark {
                adw::ColorScheme::ForceDark
            } else {
                adw::ColorScheme::ForceLight
            });
            let formulas = content
                .math
                .iter()
                .map(|span| {
                    (
                        span.range.clone(),
                        content.plain[span.body.clone()].to_owned(),
                        span.display,
                    )
                })
                .collect::<Vec<_>>();
            let prepared = gio::spawn_blocking(move || {
                formulas
                    .into_iter()
                    .map(|(range, source, display)| {
                        diagram::render_cancellable(
                            if display {
                                "math-inline-display"
                            } else {
                                "math-inline"
                            },
                            &source,
                            dark,
                            &Default::default(),
                            Arc::new(AtomicBool::new(false)),
                        )
                        .and_then(inline_math_shape::Prepared::new)
                        .map(|image| (range, image))
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(|_| "Inline render worker stopped")??;
            let draws = inline_math_shape::apply(&label, prepared, 20.0)?;
            until("mapped native formula draw", || {
                label.is_mapped() && label.width() > 0 && draws.get() >= 3
            })
            .await?;
            let wide_lines = label.layout().line_count();
            let wide = label.width();
            smoke_input::request(root, json!({"action":"focus"})).await?;
            for span in std::iter::once(None).chain(content.math.iter().map(Some)) {
                let (start, end, expected) = span.map_or((0, -1, content.plain.as_str()), |span| {
                    (
                        span.characters.start as i32,
                        span.characters.end as i32,
                        &content.plain[span.range.clone()],
                    )
                });
                label.grab_focus();
                label.select_region(start, end);
                label.emit_copy_clipboard();
                let read = smoke_input::request(root, json!({"action":"read"})).await?;
                if read["text"].as_str() != Some(expected) {
                    return Err(format!(
                        "Native shape copy differs: expected={expected:?}, actual={read}"
                    ));
                }
            }
            label.select_region(0, 0);
            label.emit_by_name::<bool>("activate-link", &[&"https://example.invalid/math"]);
            window.set_default_size(440, 420);
            until("native reflow", || {
                label.width() < wide && label.layout().line_count() > wide_lines
            })
            .await?;
            glib::timeout_future(Duration::from_millis(100)).await;
            if label.text() != content.plain {
                return Err("Shape rendering replaced the original TeX text".into());
            }
            capture(
                window,
                &root.join(if dark {
                    "inline-dark.png"
                } else {
                    "inline-light.png"
                }),
            )?;
            layouts.push(json!({"dark":dark,"draws":draws.get(),"wideLines":wide_lines,"narrowLines":label.layout().line_count()}));
            window.set_default_size(900, 420);
            until("wide layout restored", || label.width() >= wide).await?;
        }
        if links.get() != 2 {
            return Err("Math shapes changed native link activation".into());
        }
        let multiline = multiline_probe(window, root).await?;
        Ok(
            json!({"ok":true,"multilineProbe":multiline,"layouts":layouts,"sourceCharactersRetained":true,"nativeCopyExact":true,"independentWaylandClipboard":true,"linksRetained":true,"nativeReflow":true,"prototypeOnly":true}),
        )
    }
    pub(super) fn launch() -> glib::ExitCode {
        if std::env::args_os().nth(1).as_deref()
            == Some(std::ffi::OsStr::new("--native-math-worker"))
        {
            std::process::exit(math::worker_stdio());
        }
        let root = PathBuf::from(
            std::env::args_os()
                .nth(1)
                .expect("Private output directory required"),
        );
        let app = adw::Application::builder()
            .application_id(format!(
                "io.github.messon007.codexthreadstudio.MathProbe{}",
                std::process::id()
            ))
            .build();
        app.connect_activate(move |app| {
            let window = adw::ApplicationWindow::builder()
                .application(app)
                .title("Native inline math layout probe")
                .default_width(900)
                .default_height(420)
                .build();
            window.present();
            let root = root.clone();
            let app = app.clone();
            glib::spawn_future_local(async move {
                let result = run(&window, &root)
                    .await
                    .unwrap_or_else(|e| json!({"ok":false,"error":e}));
                let path = root.join("result.json");
                let data = serde_json::to_vec_pretty(&result).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, data)).await;
                window.close();
                app.quit();
            });
        });
        app.run_with_args::<&str>(&[])
    }
}

#[cfg(target_os = "linux")]
fn main() -> gtk::glib::ExitCode {
    gui::launch()
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("The native math layout probe requires Linux with GTK4/libadwaita.");
    std::process::exit(2);
}
