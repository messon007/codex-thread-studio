//! Real GTK cell Find controls and owned Wayland keyboard acceptance.
use super::*;
pub(super) async fn smoke(window: &gtk::Window, query: &str, expected: &str) -> Result<(), String> {
    fn find(root: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
        if root.is_mapped() && root.widget_name() == name {
            return Some(root.clone());
        }
        let mut child = root.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            if let Some(found) = find(&widget, name) {
                return Some(found);
            }
        }
        None
    }
    fn visible(root: &gtk::Widget, y: f32, height: f32) -> bool {
        let mut parent = root.parent();
        while let Some(widget) = parent {
            parent = widget.parent();
            if let Some(scroll) = widget.downcast_ref::<gtk::ScrolledWindow>() {
                if !root
                    .compute_point(scroll, &gtk::graphene::Point::new(0.0, y))
                    .is_some_and(|p| p.y() >= 0.0 && p.y() + height <= scroll.height() as f32)
                {
                    return false;
                }
            }
        }
        true
    }
    fn selected(root: &gtk::Widget, expected: &str) -> bool {
        if !root.is_mapped() {
            return false;
        }
        if root.is::<gtk::Label>() || root.is::<gtk::TextView>() {
            if content_widgets::selection(root, 16384).as_deref() == Some(expected) {
                if let Some(view) = root.downcast_ref::<gtk::TextView>() {
                    if let Some((start, _)) = view.buffer().selection_bounds() {
                        let glyph = view.iter_location(&start);
                        let viewport = view.visible_rect();
                        let (_, y) = view.buffer_to_window_coords(
                            gtk::TextWindowType::Widget,
                            glyph.x(),
                            glyph.y(),
                        );
                        return visible(root, y as f32, glyph.height() as f32)
                            && glyph.height() > 0
                            && glyph.y() >= viewport.y()
                            && glyph.y() + glyph.height() <= viewport.y() + viewport.height();
                    }
                } else if let Some(label) = root.downcast_ref::<gtk::Label>() {
                    if let Some((start, _)) = label.selection_bounds() {
                        let text = label.text();
                        let byte = text
                            .char_indices()
                            .nth(start as usize)
                            .map_or(text.len(), |(b, _)| b);
                        let rect = label.layout().index_to_pos(byte as i32);
                        let (_, offset) = label.layout_offsets();
                        let mut parent = label.parent();
                        while let Some(widget) = parent {
                            parent = widget.parent();
                            if let Some(scroll) = widget.downcast_ref::<gtk::ScrolledWindow>() {
                                if !label
                                    .compute_point(
                                        scroll,
                                        &gtk::graphene::Point::new(
                                            0.0,
                                            offset as f32
                                                + rect.y() as f32 / gtk::pango::SCALE as f32,
                                        ),
                                    )
                                    .is_some_and(|point| {
                                        point.y() >= 0.0
                                            && point.y()
                                                + rect.height() as f32 / gtk::pango::SCALE as f32
                                                <= scroll.height() as f32
                                    })
                                {
                                    return false;
                                }
                            }
                        }
                        return label.height() > 0;
                    }
                }
            }
        }
        let mut child = root.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            if selected(&widget, expected) {
                return true;
            }
        }
        false
    }
    async fn until(test: impl Fn() -> bool) -> Result<(), String> {
        let at = std::time::Instant::now();
        while !test() {
            if at.elapsed() > Duration::from_secs(8) {
                return Err("Cell Find did not select its original text".into());
            }
            glib::timeout_future(Duration::from_millis(20)).await;
        }
        Ok(())
    }
    if query == "tail-cell" {
        let scroll = find(window.upcast_ref(), "studio-cell-content-scroll")
            .and_downcast::<gtk::ScrolledWindow>()
            .ok_or("Image cell has no outer scroller")?;
        until(|| scroll.vadjustment().upper() > scroll.vadjustment().page_size() + 20.0).await?;
        scroll.vadjustment().set_value(scroll.vadjustment().upper());
        crate::window_capture::next_frame(window).await?;
    }
    let toggle = find(window.upcast_ref(), "studio-cell-find-toggle")
        .and_downcast::<gtk::ToggleButton>()
        .ok_or("Cell Find toggle missing")?;
    let owned = std::env::var_os("STUDIO_OWNED_MUTTER_INPUT").is_some();
    let fixture = std::env::var_os("STUDIO_GNOME_SMOKE_DIR").map(std::path::PathBuf::from);
    if owned {
        toggle.grab_focus();
        crate::smoke_input::request(
            fixture.as_ref().ok_or("No owned input root")?,
            serde_json::json!({"action":"key","key":"find"}),
        )
        .await?;
        until(|| toggle.is_active()).await?;
    } else {
        toggle.set_active(true);
    }
    until(|| find(window.upcast_ref(), "studio-cell-find-entry").is_some()).await?;
    let entry = find(window.upcast_ref(), "studio-cell-find-entry")
        .and_downcast::<gtk::SearchEntry>()
        .unwrap();
    let count = find(window.upcast_ref(), "studio-cell-find-count")
        .and_downcast::<gtk::Label>()
        .unwrap();
    entry.set_text("absent cell search phrase");
    entry.set_text(query);
    until(|| count.text().starts_with("1 of ") && selected(window.upcast_ref(), expected)).await?;
    if let Some(root) = std::env::var_os("STUDIO_GNOME_SMOKE_DIR") {
        let name = if query == "frac" {
            "cell-find-math.png"
        } else if query == "tail-cell" {
            "cell-find-long.png"
        } else {
            "cell-find-short.png"
        };
        crate::window_capture::capture(window, &std::path::PathBuf::from(root).join(name)).await?;
    }
    let previous = find(window.upcast_ref(), "studio-cell-find-previous")
        .and_downcast::<gtk::Button>()
        .unwrap();
    let next = find(window.upcast_ref(), "studio-cell-find-next")
        .and_downcast::<gtk::Button>()
        .unwrap();
    if owned {
        let before = count.text();
        crate::smoke_input::request(
            fixture.as_ref().unwrap(),
            serde_json::json!({"action":"key","key":"find-next"}),
        )
        .await?;
        if !before.starts_with("1 of 1 ") {
            until(|| count.text() != before).await?;
        }
        crate::smoke_input::request(
            fixture.as_ref().unwrap(),
            serde_json::json!({"action":"key","key":"find-previous"}),
        )
        .await?;
        until(|| count.text().starts_with("1 of ") && selected(window.upcast_ref(), expected))
            .await?;
    }
    next.emit_clicked();
    previous.emit_clicked();
    until(|| count.text().starts_with("1 of ") && selected(window.upcast_ref(), expected)).await?;
    crate::language::set("zh-CN", "en-US")?;
    crate::i18n_ui::apply();
    let translated = entry.placeholder_text().as_deref() == Some("在单元格中查找")
        && count.text().contains("匹配")
        && selected(window.upcast_ref(), expected);
    crate::language::set("en-US", "en-US")?;
    crate::i18n_ui::apply();
    if !translated {
        return Err("Cell Find live language changed the selection or missed labels".into());
    }
    entry.set_text("absent cell search phrase");
    until(|| {
        count.text() == "No matches found" && !next.is_sensitive() && !previous.is_sensitive()
    })
    .await?;
    let case = find(window.upcast_ref(), "studio-cell-find-case")
        .and_downcast::<gtk::ToggleButton>()
        .unwrap();
    entry.set_text(&format!("{query}  "));
    until(|| count.text() == "No matches found").await?;
    entry.set_text(&query.to_uppercase());
    until(|| count.text().starts_with("1 of ") && selected(window.upcast_ref(), expected)).await?;
    case.set_active(true);
    until(|| count.text() == "No matches found").await?;
    entry.set_text(query);
    until(|| count.text().starts_with("1 of ") && selected(window.upcast_ref(), expected)).await?;
    if query == "tail-cell" {
        entry.set_text("字");
        until(|| count.text().ends_with("Limit reached") && selected(window.upcast_ref(), "字"))
            .await?;
        next.emit_clicked();
        until(|| count.text().starts_with("2 of ")).await?;
        previous.emit_clicked();
        until(|| count.text().starts_with("1 of ")).await?;
    }
    if owned {
        crate::smoke_input::request(
            fixture.as_ref().unwrap(),
            serde_json::json!({"action":"key","key":"escape"}),
        )
        .await?;
        until(|| !toggle.is_active()).await?;
        crate::smoke_input::request(
            fixture.as_ref().unwrap(),
            serde_json::json!({"action":"key","key":"copy"}),
        )
        .await?;
        let at = std::time::Instant::now();
        loop {
            let copied = crate::smoke_input::request(
                fixture.as_ref().unwrap(),
                serde_json::json!({"action":"read"}),
            )
            .await?;
            if copied["text"].as_str() == Some(expected) {
                break;
            }
            if at.elapsed() > Duration::from_secs(2) {
                return Err(format!(
                    "Closing cell Find did not restore source Copy focus: {copied}"
                ));
            }
            glib::timeout_future(Duration::from_millis(20)).await;
        }
    } else {
        toggle.set_active(false);
    }
    Ok(())
}
