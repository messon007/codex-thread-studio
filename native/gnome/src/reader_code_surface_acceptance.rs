//! Real document-actor code surfaces, including uncolored budget fallbacks.
use super::*;
use gtk::{gdk, graphene};
use studio_markdown_native::syntax::{Paint, Reason, Status};

fn painted_pixel(view: &gtk::TextView, x: i32, y: i32) -> Result<[u8; 4], String> {
    let paintable = gtk::WidgetPaintable::new(Some(view));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, view.width() as f64, view.height() as f64);
    let node = snapshot.to_node().ok_or("Code view has no painted node")?;
    let renderer = view
        .native()
        .and_then(|n| n.renderer())
        .ok_or("Code view has no renderer")?;
    let texture = renderer.render_texture(
        &node,
        Some(&graphene::Rect::new(
            0.0,
            0.0,
            view.width() as f32,
            view.height() as f32,
        )),
    );
    if x < 0 || y < 0 || x >= texture.width() || y >= texture.height() {
        return Err(format!("Code sample outside painted viewport: {x},{y}"));
    }
    let mut download = gdk::TextureDownloader::new(&texture);
    download.set_format(gdk::MemoryFormat::R8g8b8a8);
    let (bytes, stride) = download.download_bytes();
    let offset = y as usize * stride + x as usize * 4;
    Ok(bytes.as_ref()[offset..offset + 4].try_into().unwrap())
}
fn matches(actual: [u8; 4], expected: [u8; 4]) -> bool {
    actual
        .into_iter()
        .zip(expected)
        .all(|(a, b)| a.abs_diff(b) <= 2)
}
fn gutter_ink(view: &gtk::TextView, y: i32, background: [u8; 4]) -> Result<bool, String> {
    let gutter = crate::code_gutter::widget(view).ok_or("Code gutter missing")?;
    let snapshot = gtk::Snapshot::new();
    gtk::WidgetPaintable::new(Some(&gutter)).snapshot(
        &snapshot,
        gutter.width() as f64,
        gutter.height() as f64,
    );
    let node = snapshot
        .to_node()
        .ok_or("Code gutter has no painted node")?;
    let renderer = view
        .native()
        .and_then(|n| n.renderer())
        .ok_or("Gutter renderer missing")?;
    let texture = renderer.render_texture(
        &node,
        Some(&graphene::Rect::new(
            0.,
            0.,
            gutter.width() as f32,
            gutter.height() as f32,
        )),
    );
    let mut download = gdk::TextureDownloader::new(&texture);
    download.set_format(gdk::MemoryFormat::R8g8b8a8);
    let (bytes, stride) = download.download_bytes();
    for row in y.max(0)..(y + 24).min(texture.height()) {
        for col in 4..(texture.width() - 4).max(4) {
            let offset = row as usize * stride + col as usize * 4;
            let pixel = &bytes.as_ref()[offset..offset + 4];
            if pixel[3] > 240
                && pixel[..3]
                    .iter()
                    .zip(background)
                    .any(|(a, b)| a.abs_diff(b) > 30)
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
fn code_paint(buffer: &gtk::TextBuffer, offset: usize) -> Option<gdk::RGBA> {
    buffer
        .iter_at_offset(offset as i32)
        .tags()
        .into_iter()
        .filter(|t| {
            t.name()
                .is_some_and(|n| n.starts_with("studio-flow-syntax-"))
        })
        .max_by_key(|t| t.priority())
        .and_then(|t| t.background_rgba())
}
impl Reader {
    pub(super) async fn smoke_code_surfaces(
        self: &Rc<Self>,
        root: &Path,
    ) -> Result<serde_json::Value, String> {
        let path = root.join("project/code-surfaces.md");
        self.open_path(&path);
        crate::acceptance_wait::until("code surfaces through document actor", || {
            self.is_ready()
                && self.state.borrow().path.as_deref() == Some(path.as_path())
                && self.flow_preview.borrow().is_some()
        })
        .await?;
        let (view, syntax, document) = {
            let flow = self.flow_preview.borrow();
            let flow = flow.as_ref().unwrap();
            (
                flow.widget.clone(),
                flow.syntax.clone(),
                flow.document.clone(),
            )
        };
        if syntax.blocks.len() != 3
            || syntax.blocks[0].status != Status::Styled
            || !matches!(
                syntax.blocks[1].status,
                Status::Plain(Reason::UnknownLanguage)
            )
            || !matches!(syntax.blocks[2].status, Status::Plain(Reason::LongLine))
        {
            return Err(format!(
                "Code surface fixture lost styled/plain cases: {:?}",
                syntax.blocks
            ));
        }
        crate::window_capture::ready(&view).await?;
        let buffer = view.buffer();
        let original = buffer
            .slice(&buffer.start_iter(), &buffer.end_iter(), true)
            .to_string();
        let code = &syntax.blocks[0];
        buffer.select_range(
            &buffer.iter_at_offset(code.characters.start as i32),
            &buffer.iter_at_offset(code.characters.end as i32),
        );
        buffer.set_modified(false);
        let manager = adw::StyleManager::default();
        let previous = manager.color_scheme();
        let mut samples = vec![];
        let mut peak_tags = 0;
        let mut prose_samples = 0;
        let mut gutter_samples = vec![];
        let mut wrapped_checks = 0;
        for dark in [false, true, false] {
            manager.set_color_scheme(if dark {
                adw::ColorScheme::ForceDark
            } else {
                adw::ColorScheme::ForceLight
            });
            crate::acceptance_wait::until("code theme", || {
                let expected = if dark {
                    syntax.base_dark
                } else {
                    syntax.base_light
                }
                .unwrap()
                .background;
                code_paint(&buffer, code.characters.start).is_some_and(|p| {
                    matches(
                        [
                            (p.red() * 255.0).round() as u8,
                            (p.green() * 255.0).round() as u8,
                            (p.blue() * 255.0).round() as u8,
                            (p.alpha() * 255.0).round() as u8,
                        ],
                        expected,
                    )
                })
            })
            .await?;
            if buffer.is_modified()
                || buffer.slice(&buffer.start_iter(), &buffer.end_iter(), true) != original
                || buffer
                    .selection_bounds()
                    .map(|(a, b)| (a.offset(), b.offset()))
                    != Some((code.characters.start as i32, code.characters.end as i32))
            {
                return Err("Code theme changed source, selection or dirty state".into());
            }
            let count = Cell::new(0);
            buffer.tag_table().foreach(|t| {
                if t.name()
                    .is_some_and(|n| n.starts_with("studio-flow-syntax-"))
                {
                    count.set(count.get() + 1);
                }
            });
            peak_tags = peak_tags.max(count.get());
            // Replayed tags share the bounded worker palette.
            if count.get() > studio_markdown_native::syntax::PALETTE {
                return Err("Code theme tags accumulated".into());
            }
            for (index, block) in syntax.blocks.iter().enumerate() {
                let paint: Paint = if dark {
                    syntax.base_dark
                } else {
                    syntax.base_light
                }
                .unwrap();
                let text: String = original
                    .chars()
                    .skip(block.characters.start)
                    .take(block.characters.len())
                    .collect();
                let blank = text.find("\n\n").ok_or("Code fixture has no blank line")?;
                let blank = block.characters.start + text[..blank + 1].chars().count();
                let iter = buffer.iter_at_offset(blank as i32);
                buffer.place_cursor(&iter);
                let mark = buffer.create_mark(None, &iter, true);
                view.scroll_to_mark(&mark, 0.0, true, 0.0, 0.35);
                let visible = crate::acceptance_wait::until(
                    "code blank line visible after native scroll",
                    || {
                        let rect = view.iter_location(&iter);
                        let (_, y) = view.buffer_to_window_coords(
                            gtk::TextWindowType::Widget,
                            rect.x(),
                            rect.y() + rect.height() / 2,
                        );
                        let first = view
                            .iter_location(&buffer.iter_at_offset(block.characters.start as i32));
                        let (_, first_y) = view.buffer_to_window_coords(
                            gtk::TextWindowType::Widget,
                            first.x(),
                            first.y(),
                        );
                        y > 8 && y < view.height() - 8 && first_y > 0
                    },
                )
                .await;
                if let Err(error) = visible {
                    crate::window_capture::capture(
                        &self.window,
                        &root.join("code-surface-scroll-failed.png"),
                    )
                    .await?;
                    return Err(format!("{error}: dark={dark}, block={index}, glyph={:?}, visible={:?}, view={}x{}, scroll={:?}", view.iter_location(&iter), view.visible_rect(), view.width(), view.height(), view.vadjustment().map(|a|(a.value(),a.upper(),a.page_size()))));
                }
                crate::window_capture::next_frame(&view).await?;
                let rect = view.iter_location(&iter);
                let (_, y) = view.buffer_to_window_coords(
                    gtk::TextWindowType::Widget,
                    rect.x(),
                    rect.y() + rect.height() / 2,
                );
                let x = view.width() - 64;
                let pixel = painted_pixel(&view, x, y)?;
                buffer.delete_mark(&mark);
                if !matches(pixel, paint.background) {
                    crate::window_capture::capture(
                        &self.window,
                        &root.join("code-surface-paint-failed.png"),
                    )
                    .await?;
                    return Err(format!("Code background does not fill blank line: dark={dark}, block={index}, point={x},{y}, actual={pixel:?}, expected={:?}", paint.background));
                }
                let first =
                    view.iter_location(&buffer.iter_at_offset(block.characters.start as i32));
                let (_, first_y) = view.buffer_to_window_coords(
                    gtk::TextWindowType::Widget,
                    first.x(),
                    first.y() + first.height() / 2,
                );
                let first_pixel = painted_pixel(&view, x, first_y)?;
                if !matches(first_pixel, paint.background) {
                    return Err(format!("Code first line has no full background: dark={dark}, block={index}, actual={first_pixel:?}"));
                }
                let rows = crate::code_gutter::painted_rows(&view).ok_or_else(|| {
                    format!(
                        "Native code gutter absent: gutter={:?}, starts={:?}",
                        view.gutter(gtk::TextWindowType::Left)
                            .map(|g| (g.type_().name(), g.first_child().map(|c| c.type_().name()))),
                        syntax.blocks.iter().map(|b| &b.lines).collect::<Vec<_>>()
                    )
                })?;
                for number in [1, 2] {
                    let (_, _, y) = rows
                        .iter()
                        .find(|(b, n, _)| *b == index && *n == number)
                        .ok_or_else(|| {
                            format!("Code gutter missing block {index} line {number}: {rows:?}")
                        })?;
                    if !gutter_ink(&view, *y, paint.background)? {
                        return Err(format!("Code gutter has no visible ink for block {index} line {number}, dark={dark}"));
                    }
                    gutter_samples.push(serde_json::json!({"dark":dark,"block":index,"line":number,"y":y,"painted":true}));
                }
                if dark {
                    let prose = block.characters.end
                        + original
                            .chars()
                            .skip(block.characters.end)
                            .position(|c| !c.is_whitespace())
                            .unwrap_or(0);
                    let rect = view.iter_location(&buffer.iter_at_offset(prose as i32));
                    let (_, prose_y) = view.buffer_to_window_coords(
                        gtk::TextWindowType::Widget,
                        rect.x(),
                        rect.y() + rect.height() / 2,
                    );
                    if prose_y > 0 && prose_y < view.height() {
                        if matches(painted_pixel(&view, x, prose_y)?, paint.background) {
                            return Err("Code surface paints over following prose".into());
                        }
                        prose_samples += 1;
                    }
                }
                samples.push(serde_json::json!({"dark":dark,"block":index,"blankLinePixel":pixel,"firstLinePixel":first_pixel,"expected":paint.background}));
                buffer.select_range(
                    &buffer.iter_at_offset(block.characters.start as i32),
                    &buffer.iter_at_offset(block.characters.end as i32),
                );
                self.flow_preview
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .copied
                    .replace(None);
                view.emit_copy_clipboard();
                crate::acceptance_wait::until("complete code source Copy", || {
                    self.flow_preview
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .copied
                        .borrow()
                        .is_some()
                })
                .await?;
                let copied = view
                    .clipboard()
                    .read_text_future()
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or("Code Copy empty")?;
                let expected = document
                    .select(
                        block.characters.clone(),
                        studio_markdown_native::flow::BUFFER_BYTES,
                    )?
                    .text;
                if copied.as_str() != expected {
                    return Err(format!("Code Copy changed source for block {index}"));
                }
                if index == 2 {
                    let middle = buffer.iter_at_offset(block.characters.start as i32 + 1500);
                    let mark = buffer.create_mark(None, &middle, true);
                    view.scroll_to_mark(&mark, 0., true, 0., 0.1);
                    crate::acceptance_wait::until("wrapped code continuation visible", || {
                        let rect = view.iter_location(&middle);
                        let (_, y) = view.buffer_to_window_coords(
                            gtk::TextWindowType::Widget,
                            rect.x(),
                            rect.y(),
                        );
                        let first =
                            view.iter_location(&buffer.iter_at_offset(block.lines[2] as i32));
                        let (_, top) = view.buffer_to_window_coords(
                            gtk::TextWindowType::Widget,
                            first.x(),
                            first.y(),
                        );
                        y >= 0 && y < view.height() && top < 0
                    })
                    .await?;
                    crate::window_capture::next_frame(&view).await?;
                    let rows = crate::code_gutter::painted_rows(&view)
                        .ok_or("Wrapped code gutter missing")?;
                    if rows.iter().any(|(b, _, _)| *b == index) {
                        return Err(format!(
                            "Wrapped code continuation repeated a logical line number: {rows:?}"
                        ));
                    }
                    buffer.delete_mark(&mark);
                    wrapped_checks += 1;
                }
            }
            // Prose following each fence must not inherit paragraph paint.
            for block in &syntax.blocks {
                if block.characters.end < document.characters
                    && code_paint(&buffer, block.characters.end).is_some()
                {
                    return Err("Code background escaped into following prose".into());
                }
            }
            let iter = buffer.iter_at_offset(code.characters.start as i32);
            buffer.place_cursor(&iter);
            let mark = buffer.create_mark(None, &iter, true);
            view.scroll_to_mark(&mark, 0.0, true, 0.0, 0.1);
            let header = self
                .flow_preview
                .borrow()
                .as_ref()
                .unwrap()
                .objects
                .iter()
                .find(|w| w.widget_name() == "studio-code-header")
                .cloned()
                .ok_or("Code header missing")?;
            crate::acceptance_wait::until("first code header visible for capture", || {
                header
                    .compute_bounds(&view)
                    .is_some_and(|r| r.y() >= 0.0 && r.y() + r.height() < view.height() as f32)
            })
            .await?;
            crate::window_capture::capture(
                &self.window,
                &root.join(if dark {
                    "code-surfaces-dark.png"
                } else {
                    "code-surfaces-light.png"
                }),
            )
            .await?;
            buffer.delete_mark(&mark);
            buffer.select_range(
                &buffer.iter_at_offset(code.characters.start as i32),
                &buffer.iter_at_offset(code.characters.end as i32),
            );
        }
        manager.set_color_scheme(previous);
        if buffer.slice(&buffer.start_iter(), &buffer.end_iter(), true) != original
            || buffer.is_modified()
        {
            return Err("Code surface checks mutated the document".into());
        }
        if prose_samples < 2 {
            return Err("Following prose was not visually sampled".into());
        }
        view.set_buffer(Some(&gtk::TextBuffer::new(None)));
        crate::window_capture::next_frame(&view).await?;
        if crate::code_gutter::painted_rows(&view).is_none_or(|rows| !rows.is_empty()) {
            return Err("Replaced buffer retained the previous source's code numbers".into());
        }
        Ok(
            serde_json::json!({"normalDocumentActor":true,"styledAndPlain":true,"blankLinePixels":samples,"codeGutters":gutter_samples,"wrappedGutterChecks":wrapped_checks,"retiredGutterRejected":true,"exactSourceCopy":true,"themeSelectionUnchanged":true,"noProsePaint":true,"prosePixelSamples":prose_samples,"maximumTags":peak_tags}),
        )
    }
}
