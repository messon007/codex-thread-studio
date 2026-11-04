//! Production label path, real Rust render queue and owned Wayland clipboard.
use super::*;
async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let at = std::time::Instant::now();
    while !test() {
        if at.elapsed() > Duration::from_secs(20) {
            return Err(format!("Inline label acceptance timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
fn label(root: &gtk::Widget, text: &str) -> Option<gtk::Label> {
    if let Some(label) = root
        .downcast_ref::<gtk::Label>()
        .filter(|l| l.is_mapped() && l.text() == text)
    {
        return Some(label.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if let Some(label) = label(&widget, text) {
            return Some(label);
        }
    }
    None
}
fn ready(ui: &Ui, label: &gtk::Label, parts: u64) -> bool {
    ui.inline_math.smoke_info(label).is_some_and(|state| {
        state["parts"] == parts
            && state["ready"] == parts
            && state["draws"].as_u64().is_some_and(|n| n > 0)
            && state["failed"] == false
    })
}
async fn copy(root: &std::path::Path, label: &gtk::Label, expected: &str) -> Result<(), String> {
    if !label.is_mapped()
        || crate::content_widgets::selection(label.upcast_ref(), 16384).as_deref() != Some(expected)
    {
        return Err(format!(
            "Native inline Copy has no current source selection: mapped={}, bounds={:?}",
            label.is_mapped(),
            label.selection_bounds()
        ));
    }
    label.emit_copy_clipboard();
    // GTK publishes its Wayland offer asynchronously. Independently require the
    // exact text within a bounded interval, after checking the local selection.
    let at = std::time::Instant::now();
    loop {
        let reply = crate::smoke_input::request(root, json!({"action":"read"})).await?;
        if reply["text"].as_str() == Some(expected) {
            break;
        }
        if at.elapsed() > Duration::from_secs(2) {
            let local = label
                .clipboard()
                .read_text_future()
                .await
                .map_err(|e| e.to_string())?;
            return Err(format!("Native inline copy changed source: {reply}; mapped={}, bounds={:?}, local={local:?}",label.is_mapped(),label.selection_bounds()));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
fn cell_page(root: &gtk::Widget) -> Option<gtk::Label> {
    if root.widget_name() == "studio-cell-math-page" {
        return root.clone().downcast().ok();
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if let Some(page) = cell_page(&widget) {
            return Some(page);
        }
    }
    None
}
fn text_page_button(root: &gtk::Widget, tip: &str) -> Option<gtk::Button> {
    if let Some(button) = root
        .downcast_ref::<gtk::Button>()
        .filter(|b| b.tooltip_text().as_deref() == Some(tip))
    {
        return Some(button.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if let Some(button) = text_page_button(&widget, tip) {
            return Some(button);
        }
    }
    None
}
fn reader_columns(root: &gtk::Widget) -> Option<gtk::ColumnView> {
    if let Some(view) = root.downcast_ref::<gtk::ColumnView>() {
        return Some(view.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if let Some(view) = reader_columns(&widget) {
            return Some(view);
        }
    }
    None
}
fn crlf_view(root: &gtk::Widget, expected: &str) -> Option<gtk::TextView> {
    if let Some(view) = root
        .downcast_ref::<gtk::TextView>()
        .filter(|v| v.is_mapped() && v.widget_name() == "studio-multiline-rich-text")
    {
        if crate::text_flow::quick_projection(view).is_some_and(|p| {
            p.quick.as_ref().unwrap().select(0..p.characters, 8192) == Some(expected)
        }) {
            return Some(view.clone());
        }
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if let Some(view) = crlf_view(&widget, expected) {
            return Some(view);
        }
    }
    None
}
async fn copy_view(
    root: &std::path::Path,
    view: &gtk::TextView,
    expected: &str,
) -> Result<(), String> {
    view.clipboard()
        .set_text("Waiting for projected source Copy");
    view.emit_copy_clipboard();
    let started = std::time::Instant::now();
    loop {
        let copied = crate::smoke_input::request(root, json!({"action":"read"})).await?;
        if copied["text"].as_str() == Some(expected) {
            return Ok(());
        }
        if started.elapsed() > Duration::from_secs(10) {
            return Err(format!("Projected Copy differs: {copied}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
}
#[path = "paged_math_acceptance.rs"]
mod paged_acceptance;
#[path = "rich_paragraph_acceptance.rs"]
mod rich_acceptance;
pub async fn run(ui: &Rc<Ui>, root: &std::path::Path) -> Result<Value, String> {
    // This fixture measures the retained GtkLabel adapter and short rich
    // paragraphs. Installed/default-reader and flow-input fixtures separately
    // verify the normal whole-document text surface.
    let _diagnostic = ui.reader.smoke_block_adapter();
    if std::env::var_os("STUDIO_GNOME_SMOKE_PAGED_MATH").is_some() {
        return paged_acceptance::run(ui, root).await;
    }
    let path = root.join("inline-fixture.json");
    let fixture = gio::spawn_blocking(move || {
        std::fs::read(path)
            .map_err(|e| e.to_string())
            .and_then(|b| serde_json::from_slice::<Value>(&b).map_err(|e| e.to_string()))
    })
    .await
    .map_err(|_| "Fixture worker stopped")??;
    let plain = fixture["plain"].as_str().ok_or("No expected inline text")?;
    let currency = fixture["currency"].as_str().unwrap();
    let unsupported = fixture["unsupported"].as_str().unwrap();
    let start = std::time::Instant::now();
    until("verified conversation", || {
        let s = ui.state.borrow();
        s.ready && s.history_verified && !s.loading && s.options_loaded
    })
    .await?;
    ui.composer
        .buffer()
        .set_text("Unsent native math draft 中文😀 ");
    until("mapped chat formulas", || {
        label(ui.transcript_view.upcast_ref(), plain).is_some_and(|l| ready(ui, &l, 2))
    })
    .await?;
    let first_ms = start.elapsed().as_millis();
    let chat = label(ui.transcript_view.upcast_ref(), plain).unwrap();
    let currency =
        label(ui.transcript_view.upcast_ref(), currency).ok_or("Currency label missing")?;
    if ui.inline_math.smoke_info(&currency).is_some() {
        return Err("Currency was registered as a formula".into());
    }
    let failed = label(ui.transcript_view.upcast_ref(), unsupported)
        .ok_or("Unsupported formula label missing")?;
    until("readable error fallback", || {
        ui.inline_math
            .smoke_info(&failed)
            .is_some_and(|s| s["failed"] == true)
    })
    .await?;
    if failed.text() != unsupported
        || !failed
            .tooltip_text()
            .is_some_and(|s| s.starts_with("Inline math:"))
    {
        return Err("Math error hid original TeX".into());
    }
    crate::smoke_input::request(root, json!({"action":"focus"})).await?;
    for dark in [false, true] {
        adw::StyleManager::default().set_color_scheme(if dark {
            adw::ColorScheme::ForceDark
        } else {
            adw::ColorScheme::ForceLight
        });
        until("themed chat shapes", || ready(ui, &chat, 2)).await?;
        chat.grab_focus();
        chat.select_region(0, -1);
        copy(root, &chat, plain).await?;
        crate::window_capture::capture(
            &ui.window,
            &root.join(if dark {
                "inline-chat-dark.png"
            } else {
                "inline-chat-light.png"
            }),
        )
        .await?;
    }
    let multiline = fixture["crlfPlain"]
        .as_str()
        .ok_or("No multiline chat fixture")?;
    until("mapped chat multiline object", || {
        crlf_view(ui.transcript_view.upcast_ref(), multiline).is_some()
    })
    .await?;
    let multiline_view = crlf_view(ui.transcript_view.upcast_ref(), multiline).unwrap();
    let multiline_buffer = multiline_view.buffer();
    multiline_buffer.select_range(&multiline_buffer.start_iter(), &multiline_buffer.end_iter());
    copy_view(root, &multiline_view, multiline).await?;
    if multiline_buffer.line_count() != 1 {
        return Err("Chat formula retained internal hard break".into());
    }
    crate::window_capture::capture(&ui.window, &root.join("inline-chat-multiline.png")).await?;
    chat.grab_focus();
    chat.select_region(0, -1);
    let attributes = gtk::pango::AttrList::new();
    attributes.insert(gtk::pango::AttrSize::new_size_absolute(
        30 * gtk::pango::SCALE,
    ));
    chat.set_attributes(Some(&attributes));
    until("font-resized math shapes", || {
        ready(ui, &chat, 2)
            && ui
                .inline_math
                .smoke_info(&chat)
                .is_some_and(|s| (s["font"].as_f64().unwrap_or(0.0) - 30.0).abs() < 0.1)
    })
    .await?;
    if chat.selection_bounds() != Some((0, plain.chars().count() as i32)) {
        return Err("Math relayout reset native selection".into());
    }
    copy(root, &chat, plain).await?;
    chat.set_attributes(None::<&gtk::pango::AttrList>);
    until("normal typography restored", || {
        ready(ui, &chat, 2)
            && ui
                .inline_math
                .smoke_info(&chat)
                .is_some_and(|s| s["font"].as_f64().unwrap_or(96.0) < 30.0)
    })
    .await?;
    chat.select_region(0, 0);
    ui.reader.open_path(&root.join("project/inline.md"));
    ui.show_tool("document");
    until("mapped reader formulas", || {
        ui.reader.is_ready()
            && label(ui.reader.widget.upcast_ref(), plain).is_some_and(|l| ready(ui, &l, 2))
    })
    .await?;
    let paragraph = label(ui.reader.widget.upcast_ref(), plain).unwrap();
    until("native table formula", || {
        label(ui.reader.widget.upcast_ref(), "$x+y$").is_some_and(|l| ready(ui, &l, 1))
    })
    .await?;
    let excerpt_text = fixture["excerpt"]
        .as_str()
        .ok_or("No math excerpt fixture")?;
    let clipped_text = fixture["clipped"]
        .as_str()
        .ok_or("No clipped formula fixture")?;
    until("native excerpt formula", || {
        label(ui.reader.widget.upcast_ref(), excerpt_text).is_some_and(|l| ready(ui, &l, 1))
    })
    .await?;
    let excerpt = label(ui.reader.widget.upcast_ref(), excerpt_text).unwrap();
    let clipped =
        label(ui.reader.widget.upcast_ref(), clipped_text).ok_or("Clipped math excerpt missing")?;
    if ui.inline_math.smoke_info(&clipped).is_some() {
        return Err("Truncated formula was submitted to the math renderer".into());
    }
    for dark in [false, true] {
        adw::StyleManager::default().set_color_scheme(if dark {
            adw::ColorScheme::ForceDark
        } else {
            adw::ColorScheme::ForceLight
        });
        until("themed excerpt formula", || ready(ui, &excerpt, 1)).await?;
        excerpt.grab_focus();
        // GTK treats the ellipsized tail atomically and toggles it when the
        // same endpoint is selected again. Reset before selecting all.
        excerpt.select_region(0, 0);
        excerpt.select_region(0, -1);
        copy(root, &excerpt, excerpt_text).await?;
        crate::window_capture::capture(
            &ui.window,
            &root.join(if dark {
                "inline-excerpt-dark.png"
            } else {
                "inline-excerpt-light.png"
            }),
        )
        .await?;
    }
    let quote = r"$\frac{a}{b}$";
    let offset = excerpt_text
        .find(quote)
        .ok_or("Excerpt formula source missing")?;
    let first = excerpt_text[..offset].chars().count() as i32;
    excerpt.select_region(first, first + quote.chars().count() as i32);
    copy(root, &excerpt, quote).await?;
    let excerpt_comment = ui.reader.smoke_inline_comment(quote, 14)?;
    excerpt.select_region(0, 0);
    let excerpt_recycling = ui
        .inline_math
        .smoke_recycling(&excerpt, &excerpt.label())
        .await?;
    until("recycled excerpt painted", || ready(ui, &excerpt, 1)).await?;
    let open = excerpt
        .parent()
        .and_then(|p| p.first_child())
        .and_downcast::<gtk::Button>()
        .filter(|b| b.widget_name() == "studio-cell-open")
        .ok_or("Math cell full reader action missing")?;
    let cell_started = std::time::Instant::now();
    let cell_ticks = Rc::new(std::cell::Cell::new(0usize));
    let cell_gap = Rc::new(std::cell::Cell::new(0u128));
    let cell_last = Rc::new(std::cell::Cell::new(std::time::Instant::now()));
    let (ticks, gap, last) = (cell_ticks.clone(), cell_gap.clone(), cell_last.clone());
    let cell_timer = glib::timeout_add_local(Duration::from_millis(10), move || {
        ticks.set(ticks.get() + 1);
        gap.set(
            gap.get().max(
                last.replace(std::time::Instant::now())
                    .elapsed()
                    .as_millis(),
            ),
        );
        glib::ControlFlow::Continue
    });
    open.emit_clicked();
    fn find_cell_window() -> Option<gtk::Window> {
        gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Window>().ok())
            .find(|w| w.is_mapped() && w.widget_name() == "studio-selection-window")
    }
    until("complete math cell window", || find_cell_window().is_some()).await?;
    let cell_window = find_cell_window().unwrap();
    until("paged cell first formula", || {
        cell_page(cell_window.upcast_ref()).is_some_and(|p| ready(ui, &p, 1))
    })
    .await?;
    let cell_open_ms = cell_started.elapsed().as_millis();
    crate::table_cell::smoke_find(&cell_window, "frac", quote).await?;
    let first_page = cell_page(cell_window.upcast_ref()).unwrap();
    if first_page.text().len() > studio_markdown_native::rich_pages::PAGE_BYTES
        || !first_page.label().contains("<b>styled</b>")
        || !first_page.label().contains("href=\"nested/note.md\"")
    {
        return Err("Paged math cell lost bounds, formatting or link identity".into());
    }
    let offset = first_page
        .text()
        .find(quote)
        .ok_or("No first-page formula")?;
    let start = first_page.text()[..offset].chars().count() as i32;
    for dark in [false, true] {
        adw::StyleManager::default().set_color_scheme(if dark {
            adw::ColorScheme::ForceDark
        } else {
            adw::ColorScheme::ForceLight
        });
        until("themed full-cell formula", || ready(ui, &first_page, 1)).await?;
        first_page.grab_focus();
        first_page.select_region(0, 0);
        first_page.select_region(start, start + quote.chars().count() as i32);
        copy(root, &first_page, quote).await?;
        crate::window_capture::capture(
            &cell_window,
            &root.join(if dark {
                "inline-cell-page-dark.png"
            } else {
                "inline-cell-page-light.png"
            }),
        )
        .await?;
    }
    let paged_cell_comment = ui.reader.smoke_inline_comment(quote, 14)?;
    text_page_button(cell_window.upcast_ref(), "Next text page")
        .ok_or("No next cell page")?
        .emit_clicked();
    until("new native cell page", || {
        cell_page(cell_window.upcast_ref()).is_some_and(|p| p != first_page && p.is_mapped())
    })
    .await?;
    until("atomic boundary formula", || {
        cell_page(cell_window.upcast_ref())
            .is_some_and(|p| p.text().contains(r"$\frac{c}{d}$") && ready(ui, &p, 1))
    })
    .await?;
    let boundary_page = cell_page(cell_window.upcast_ref()).unwrap();
    if boundary_page.text().len() > studio_markdown_native::rich_pages::PAGE_BYTES {
        return Err("Atomic formula unexpectedly enlarged a normal cell page".into());
    }
    if first_page.is_mapped()
        || ui
            .inline_math
            .smoke_info(&first_page)
            .is_some_and(|s| s["ready"] != 0)
    {
        return Err("Changing a math cell page retained old formula surfaces".into());
    }
    text_page_button(cell_window.upcast_ref(), "Last text page")
        .ok_or("No last cell page")?
        .emit_clicked();
    until("last cell formula", || {
        cell_page(cell_window.upcast_ref())
            .is_some_and(|p| p.text().contains("complete-cell-tail") && ready(ui, &p, 1))
    })
    .await?;
    let last_page = cell_page(cell_window.upcast_ref()).unwrap();
    let tail_quote = r"$\prod_{i=1}^n i$";
    let at = last_page
        .text()
        .find(tail_quote)
        .ok_or("No last-page formula")?;
    let at = last_page.text()[..at].chars().count() as i32;
    last_page.grab_focus();
    last_page.select_region(at, at + tail_quote.chars().count() as i32);
    copy(root, &last_page, tail_quote).await?;
    ui.reader.smoke_inline_comment(tail_quote, 14)?;
    if let Some(scroll) = last_page
        .ancestor(gtk::ScrolledWindow::static_type())
        .and_downcast::<gtk::ScrolledWindow>()
    {
        let adjustment = scroll.vadjustment();
        adjustment.set_value((adjustment.upper() - adjustment.page_size()).max(0.0));
        glib::timeout_future(Duration::from_millis(60)).await;
    }
    crate::window_capture::capture(&cell_window, &root.join("inline-cell-last-page.png")).await?;
    fn copy_cell(widget: &gtk::Widget) -> Option<gtk::Button> {
        if let Some(button) = widget
            .downcast_ref::<gtk::Button>()
            .filter(|b| b.tooltip_text().as_deref() == Some("Copy complete cell text"))
        {
            return Some(button.clone());
        }
        let mut child = widget.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            if let Some(button) = copy_cell(&widget) {
                return Some(button);
            }
        }
        None
    }
    copy_cell(cell_window.upcast_ref())
        .ok_or("Complete cell Copy missing")?
        .emit_clicked();
    let copied = crate::smoke_input::request(root, json!({"action":"read"})).await?;
    if copied["text"] != fixture["completeCell"] {
        return Err("Math excerpt truncated or rewrote complete-cell Copy".into());
    }
    cell_window.close();
    until("complete cell closed", || !cell_window.is_mapped()).await?;
    if ui
        .inline_math
        .smoke_info(&last_page)
        .is_some_and(|s| s["ready"] != 0)
    {
        return Err("Closing the full-cell reader retained math surfaces".into());
    }
    open.emit_clicked();
    let closed_early = find_cell_window().ok_or("Immediate cell open did not map")?;
    closed_early.close();
    glib::timeout_future(Duration::from_millis(80)).await;
    if closed_early.is_mapped() || find_cell_window().is_some() {
        return Err("A closed cell reader accepted late page preparation".into());
    }
    cell_timer.remove();
    let cell_measurement = json!({"firstFormulaMs":cell_open_ms,"workflowMs":cell_started.elapsed().as_millis(),"heartbeatTicks":cell_ticks.get(),"maximumHeartbeatGapMs":cell_gap.get(),"pageBytes":studio_markdown_native::rich_pages::PAGE_BYTES,"boundaryFormulaAtomic":true,"closedBeforePreparation":true});
    reader_columns(ui.reader.widget.upcast_ref())
        .ok_or("No native reader table")?
        .activate_action("cell-header-1.read", None)
        .map_err(|e| e.to_string())?;
    until("expanded math header", || {
        find_cell_window().is_some_and(|w| {
            cell_page(w.upcast_ref())
                .is_some_and(|p| p.text().starts_with("Header") && ready(ui, &p, 1))
        })
    })
    .await?;
    let header_window = find_cell_window().unwrap();
    let header_page = cell_page(header_window.upcast_ref()).unwrap();
    let header_quote = r"$\sqrt{x}$";
    let at = header_page
        .text()
        .find(header_quote)
        .ok_or("Header formula missing")?;
    let at = header_page.text()[..at].chars().count() as i32;
    header_page.grab_focus();
    header_page.select_region(at, at + header_quote.chars().count() as i32);
    copy(root, &header_page, header_quote).await?;
    let paged_header_comment = ui.reader.smoke_inline_comment(header_quote, 11)?;
    crate::window_capture::capture(&header_window, &root.join("inline-header-page.png")).await?;
    header_window.close();
    until("math header closed", || !header_window.is_mapped()).await?;
    if ui
        .inline_math
        .smoke_info(&header_page)
        .is_some_and(|s| s["ready"] != 0)
    {
        return Err("Header close retained math surfaces".into());
    }
    let byte = plain.find(quote).unwrap();
    let first = plain[..byte].chars().count() as i32;
    paragraph.grab_focus();
    paragraph.select_region(first, first + quote.chars().count() as i32);
    copy(root, &paragraph, quote).await?;
    let comment = ui.reader.smoke_inline_comment(quote, 3)?;
    paragraph.select_region(0, 0);
    let recycling = ui
        .inline_math
        .smoke_recycling(&paragraph, &paragraph.label())
        .await?;
    until("recycled reader shapes painted", || {
        ready(ui, &paragraph, 2)
    })
    .await?;
    crate::window_capture::capture(&ui.window, &root.join("inline-reader-dark.png")).await?;
    let flow = ui.reader.smoke_flow_math_table().await?;
    fn table_scroll(widget: &gtk::Widget) -> Option<gtk::ScrolledWindow> {
        if widget.is::<gtk::ColumnView>() {
            return widget
                .ancestor(gtk::ScrolledWindow::static_type())
                .and_downcast();
        }
        let mut child = widget.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            if let Some(scroll) = table_scroll(&widget) {
                return Some(scroll);
            }
        }
        None
    }
    let table_scroll =
        table_scroll(flow.widget.upcast_ref()).ok_or("Continuous table viewport missing")?;
    let table = table_scroll
        .child()
        .and_downcast::<gtk::ColumnView>()
        .ok_or("No continuous ColumnView")?;
    table.scroll_to(
        1,
        None::<&gtk::ColumnViewColumn>,
        gtk::ListScrollFlags::NONE,
        None,
    );
    let flow_ready = until("continuous table excerpt math", || {
        label(flow.widget.upcast_ref(), excerpt_text).is_some_and(|l| ready(ui, &l, 1))
    })
    .await;
    if let Err(error) = flow_ready {
        crate::window_capture::capture(&ui.window, &root.join("inline-flow-visibility-failed.png"))
            .await?;
        return Err(format!(
            "{error}: {:?}",
            label(flow.widget.upcast_ref(), excerpt_text).map(|l| {
                let scopes = crate::viewport::scopes(&l);
                (
                    ui.inline_math.smoke_info(&l),
                    l.compute_bounds(&flow.widget),
                    crate::viewport::intersects(&l, &scopes),
                    scopes
                        .iter()
                        .filter_map(glib::WeakRef::upgrade)
                        .map(|s| {
                            (
                                s.width(),
                                s.height(),
                                s.hadjustment().value(),
                                s.vadjustment().value(),
                                l.compute_bounds(&s),
                            )
                        })
                        .collect::<Vec<_>>(),
                )
            })
        ));
    }
    let columns = table.columns();
    let first_column = columns
        .item(0)
        .and_downcast::<gtk::ColumnViewColumn>()
        .ok_or("No first column")?;
    let last_column = columns
        .item(1)
        .and_downcast::<gtk::ColumnViewColumn>()
        .ok_or("No second column")?;
    let old_widths = (first_column.fixed_width(), last_column.fixed_width());
    first_column.set_fixed_width(700);
    last_column.set_fixed_width(220);
    ui.window.fullscreen();
    until("owned fullscreen resize desktop", || {
        ui.window.width() == 1600 && ui.window.height() == 1000
    })
    .await?;
    let position = ui.workspace.position();
    let buffer = flow.widget.buffer();
    let original_buffer = buffer
        .slice(&buffer.start_iter(), &buffer.end_iter(), true)
        .to_string();
    buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
    let mut widths = Vec::new();
    fn controls_fit(widget: &gtk::Widget, owner: &gtk::Widget) -> bool {
        if widget.is_mapped() && (widget.is::<gtk::Button>() || widget.is::<gtk::MenuButton>()) {
            let Some(bounds) = widget.compute_bounds(owner) else {
                return false;
            };
            if bounds.x() < -1.0 || bounds.x() + bounds.width() > owner.width() as f32 + 1.0 {
                return false;
            }
        }
        let mut child = widget.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            if !controls_fit(&widget, owner) {
                return false;
            }
        }
        true
    }
    for requested in [900, 420, 700, 420] {
        let at = std::time::Instant::now();
        ui.workspace
            .set_position((ui.workspace.width() - requested).max(0));
        until("table follows document pane", || {
            let Some(bounds) = flow.objects[0].compute_bounds(&flow.widget) else {
                return false;
            };
            (flow.widget.width() - requested).abs() < 32
                && (flow.objects[0].width() - (flow.widget.width() - 48)).abs() <= 1
                && bounds.x() >= 23.0
                && bounds.x() + bounds.width() <= flow.widget.width() as f32 - 23.0
                && flow.objects[0]
                    .first_child()
                    .is_some_and(|bar| controls_fit(&bar, &flow.objects[0]))
                && ui
                    .reader
                    .widget
                    .first_child()
                    .is_some_and(|bar| controls_fit(&bar, ui.reader.widget.upcast_ref()))
        })
        .await?;
        let allocated_ms = at.elapsed().as_millis();
        let adjustment = table_scroll.hadjustment();
        adjustment.set_value(40.0);
        glib::timeout_future(Duration::from_millis(40)).await;
        if first_column.fixed_width() != 700
            || last_column.fixed_width() != 220
            || (adjustment.value() - 40.0).abs() > 1.0
            || !flow.syntax_current()
            || buffer.slice(&buffer.start_iter(), &buffer.end_iter(), true) != original_buffer
            || buffer
                .selection_bounds()
                .map(|(a, z)| (a.offset(), z.offset()))
                != Some((0, buffer.char_count()))
        {
            return Err("Pane resize changed columns, scroll, source or selection".into());
        }
        widths.push(json!({"requested":requested,"pane":flow.widget.width(),"table":flow.objects[0].width(),"allocatedMs":allocated_ms,"settledMs":at.elapsed().as_millis(),"observationMs":40,"layoutUpdates":flow.layout_updates()}));
    }
    let updates = flow.layout_updates();
    glib::timeout_future(Duration::from_millis(120)).await;
    if flow.layout_updates() != updates {
        return Err("Table allocation did not become idle".into());
    }
    flow.copied.borrow_mut().take();
    flow.widget.grab_focus();
    flow.widget.emit_copy_clipboard();
    until("resized table Copy", || {
        !flow.copying.get() && flow.copied.borrow().is_some()
    })
    .await?;
    let expected = flow
        .document
        .select(
            0..flow.document.characters,
            studio_markdown_native::flow::BUFFER_BYTES,
        )?
        .text;
    let copied = crate::smoke_input::request(root, json!({"action":"read"})).await?;
    if copied["text"].as_str() != Some(expected.as_str()) {
        return Err("Resizing rewrote projected table Copy".into());
    }
    crate::window_capture::capture(&ui.window, &root.join("inline-flow-narrow.png")).await?;
    let left = ui
        .workspace
        .start_child()
        .and_then(|w| w.compute_bounds(&ui.window))
        .ok_or("No owned pane handle geometry")?;
    let right = ui
        .workspace
        .end_child()
        .and_then(|w| w.compute_bounds(&ui.window))
        .ok_or("No owned document pane geometry")?;
    let x = (f64::from(left.x() + left.width() + right.x()) * 0.5).floor();
    let y = f64::from(left.y() + left.height() * 0.80);
    let before_drag = flow.widget.width();
    let active_before_drag = ui.window.is_active();
    // Standalone Mutter may leave a remapped fullscreen window inactive after
    // the modal cell reader closes. Activate it before testing the divider.
    crate::smoke_input::request(root, json!({"action":"focus"})).await?;
    until("active owned drag window", || ui.window.is_active()).await?;
    // Match the standalone input fixture's initial plain click: establish
    // pointer focus before asserting a native drag sequence.
    crate::smoke_input::request(root, json!({"action":"pointer","x":x,"y":y})).await?;
    glib::timeout_future(Duration::from_millis(60)).await;
    let pointer_events = Rc::new(RefCell::new(Vec::new()));
    pointer_events.borrow_mut().push(json!({"beforeTarget":ui.window.pick(x,y,gtk::PickFlags::DEFAULT).map(|w|format!("{}:{}",w.type_().name(),w.css_name())),"active":ui.window.is_active(),"modifiers":gtk::gdk::Display::default().and_then(|d|d.default_seat()).and_then(|s|s.pointer()).map(|p|format!("{:?}",p.modifier_state()))}));
    let mut drag_handlers = Vec::new();
    let controllers = ui.workspace.observe_controllers();
    pointer_events.borrow_mut().push(json!({"controllers":(0..controllers.n_items()).filter_map(|i|controllers.item(i).map(|v|v.type_().name().to_string())).collect::<Vec<_>>()}));
    for index in 0..controllers.n_items() {
        if let Some(drag) = controllers.item(index).and_downcast::<gtk::GestureDrag>() {
            let events = pointer_events.clone();
            let begin = drag.connect_drag_begin(move |drag,x,y| events.borrow_mut().push(json!({"dragBegin":[x,y],"button":drag.current_button(),"recognized":drag.is_recognized()})));
            let events = pointer_events.clone();
            let update = drag.connect_drag_update(move |_, x, y| {
                events.borrow_mut().push(json!({"dragUpdate":[x,y]}))
            });
            drag_handlers.push((drag, begin, update));
        }
    }
    let observe = gtk::EventControllerLegacy::new();
    observe.set_propagation_phase(gtk::PropagationPhase::Capture);
    let (events, window) = (pointer_events.clone(), ui.window.downgrade());
    observe.connect_event(move |_, event| {
        if matches!(event.event_type(), gtk::gdk::EventType::ButtonPress | gtk::gdk::EventType::ButtonRelease | gtk::gdk::EventType::MotionNotify) && events.borrow().len() < 32 {
            if let (Some(window), Some((x,y))) = (window.upgrade(), event.position()) {
                let target = window.pick(x,y,gtk::PickFlags::DEFAULT);
                events.borrow_mut().push(json!({"event":format!("{:?}",event.event_type()),"modifiers":format!("{:?}",event.modifier_state()),"x":x,"y":y,"target":target.as_ref().map(|w|w.type_().name()),"css":target.as_ref().map(|w|w.css_name().to_string())}));
            }
        }
        glib::Propagation::Proceed
    });
    ui.window.add_controller(observe.clone());
    crate::smoke_input::request(root, json!({"action":"pointer","x":x,"y":y,"dx":-120.0})).await?;
    ui.window.remove_controller(&observe);
    for (drag, begin, update) in drag_handlers {
        drag.disconnect(begin);
        drag.disconnect(update);
    }
    if let Err(error) = until("owned pointer resized the document pane", || {
        (flow.widget.width() - before_drag - 120).abs() <= 8
            && (flow.objects[0].width() - flow.widget.width() + 48).abs() <= 1
    })
    .await
    {
        crate::window_capture::capture(&ui.window, &root.join("inline-flow-drag-failed.png"))
            .await?;
        return Err(format!("{error}; pointer=({x},{y}), before={before_drag}, after={}, position={}, table={}, left={left:?}, right={right:?}, events={:?}", flow.widget.width(), ui.workspace.position(), flow.objects[0].width(), pointer_events.borrow()));
    }
    // Let the final allocation reach painting before inspecting/capturing it.
    glib::timeout_future(Duration::from_millis(40)).await;
    if first_column.fixed_width() != 700
        || last_column.fixed_width() != 220
        || buffer.slice(&buffer.start_iter(), &buffer.end_iter(), true) != original_buffer
        || buffer
            .selection_bounds()
            .map(|(a, z)| (a.offset(), z.offset()))
            != Some((0, buffer.char_count()))
        || ui
            .reader
            .widget
            .first_child()
            .is_none_or(|bar| !controls_fit(&bar, ui.reader.widget.upcast_ref()))
        || flow.objects[0]
            .first_child()
            .is_none_or(|bar| !controls_fit(&bar, &flow.objects[0]))
    {
        return Err(
            "Native pane drag changed source, selection, columns or header reachability".into(),
        );
    }
    {
        let events = pointer_events.borrow();
        if !events
            .iter()
            .any(|e| e["event"] == "ButtonPress" && e["css"] == "separator")
            || !events
                .iter()
                .any(|e| e["button"] == 1 && e["recognized"] == true)
            || !events.iter().any(|e| e["dragUpdate"][0] == -120.0)
        {
            return Err("Pane changed without a verified native divider drag sequence".into());
        }
    }
    crate::window_capture::capture(&ui.window, &root.join("inline-flow-dragged.png")).await?;
    crate::window_capture::capture_widget(
        &ui.window,
        &ui.reader.widget,
        &root.join("inline-flow-pane-dragged.png"),
    )
    .await?;
    first_column.set_fixed_width(old_widths.0);
    last_column.set_fixed_width(old_widths.1);
    table_scroll.hadjustment().set_value(0.0);
    ui.workspace.set_position(position);
    until("reader pane restored", || {
        (flow.widget.width() - (ui.workspace.width() - position)).abs() < 32
    })
    .await?;
    let adjustment = table_scroll.vadjustment();
    adjustment.set_value((adjustment.upper() - adjustment.page_size()).max(0.0));
    until("continuous clipped row realized", || {
        label(flow.widget.upcast_ref(), clipped_text).is_some()
    })
    .await?;
    let flow_clipped = label(flow.widget.upcast_ref(), clipped_text).unwrap();
    if ui.inline_math.smoke_info(&flow_clipped).is_some() {
        return Err("Continuous table rendered a clipped formula".into());
    }
    adjustment.set_value(0.0);
    until("continuous math row restored", || {
        label(flow.widget.upcast_ref(), excerpt_text).is_some_and(|l| ready(ui, &l, 1))
    })
    .await?;
    let flow_excerpt = label(flow.widget.upcast_ref(), excerpt_text).unwrap();
    flow_excerpt.grab_focus();
    flow_excerpt.select_region(0, 0);
    flow_excerpt.select_region(0, -1);
    copy(root, &flow_excerpt, excerpt_text).await?;
    // This table has a different prefix from the paragraph's formula.
    let offset = excerpt_text.find(quote).unwrap();
    let begin = excerpt_text[..offset].chars().count() as i32;
    flow_excerpt.select_region(begin, begin + quote.chars().count() as i32);
    let flow_comment = ui.reader.smoke_inline_comment(quote, 14)?;
    // A realized row may still be clipped by the bounded inner table viewport.
    // Center the actual formula glyph, not the first row or the whole label.
    let glyph_y = |label: &gtk::Label| -> Option<(f64, f64)> {
        let bounds = label.compute_bounds(&table_scroll)?;
        let glyph = label.layout().index_to_pos(offset as i32);
        let (_, y) = label.layout_offsets();
        Some((
            f64::from(bounds.y())
                + f64::from(y)
                + f64::from(glyph.y()) / f64::from(gtk::pango::SCALE),
            f64::from(glyph.height()) / f64::from(gtk::pango::SCALE),
        ))
    };
    let (top, height) = glyph_y(&flow_excerpt).ok_or("Math glyph has no viewport geometry")?;
    adjustment.set_value(
        adjustment.value() + top + height * 0.5 - f64::from(table_scroll.height()) * 0.65,
    );
    until("continuous formula actually visible", || {
        glyph_y(&flow_excerpt).is_some_and(|(top, height)| {
            top >= 32.0 && height > 0.0 && top + height <= f64::from(table_scroll.height())
        })
    })
    .await?;
    crate::window_capture::capture(&ui.window, &root.join("inline-flow-table.png")).await?;
    ui.reader.smoke_restore_block_preview();
    until("continuous math unmap cleanup", || {
        !flow.widget.is_mapped()
            && ui
                .inline_math
                .smoke_info(&flow_excerpt)
                .is_some_and(|s| s["ready"] == 0)
    })
    .await?;
    ui.reader.show_source();
    until("native Source", || {
        ui.reader.source_mode() && ui.reader.is_ready()
    })
    .await?;
    if ui
        .inline_math
        .smoke_info(&paragraph)
        .is_some_and(|s| s["ready"] != 0)
    {
        return Err("Source mode retained preview surfaces".into());
    }
    if ui
        .inline_math
        .smoke_info(&excerpt)
        .is_some_and(|s| s["ready"] != 0)
    {
        return Err("Source mode retained excerpt math surfaces".into());
    }
    ui.reader.show_preview();
    until("Preview formulas restored", || {
        ui.reader.is_ready()
            && label(ui.reader.widget.upcast_ref(), plain).is_some_and(|l| ready(ui, &l, 2))
    })
    .await?;
    let crlf_plain = fixture["crlfPlain"].as_str().ok_or("No CRLF fixture")?;
    ui.reader.open_path(&root.join("project/crlf-math.md"));
    until("source-preserving CRLF paragraph", || {
        ui.reader.is_ready() && crlf_view(ui.reader.widget.upcast_ref(), crlf_plain).is_some()
    })
    .await?;
    let crlf = crlf_view(ui.reader.widget.upcast_ref(), crlf_plain).unwrap();
    let buffer = crlf.buffer();
    crlf.grab_focus();
    buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
    copy_view(root, &crlf, crlf_plain).await?;
    let projection = crate::text_flow::quick_projection(&crlf).unwrap();
    let q = projection.objects[0].alternative.clone();
    let range = projection.objects[0].characters.clone();
    buffer.select_range(
        &buffer.iter_at_offset(range.start as i32),
        &buffer.iter_at_offset(range.end as i32),
    );
    copy_view(root, &crlf, &q).await?;
    let crlf_comment = ui.reader.smoke_inline_comment_range(&q, 3, 4)?;
    let original_bold = crlf_plain[..crlf_plain.find("bold").unwrap()]
        .chars()
        .count();
    let original_link = crlf_plain[..crlf_plain.find("safe link").unwrap()]
        .chars()
        .count();
    let quick = projection.quick.as_ref().unwrap();
    let bold = quick
        .projected_range(original_bold..original_bold)
        .unwrap()
        .start as i32;
    let link = quick
        .projected_range(original_link..original_link)
        .unwrap()
        .start as i32;
    if buffer.line_count() != 1
        || crlf.iter_location(&buffer.start_iter()).y()
            != crlf
                .iter_location(&buffer.iter_at_offset(buffer.char_count() - 1))
                .y()
    {
        return Err("Rendered multiline formula retained an internal hard break".into());
    }
    if !buffer
        .iter_at_offset(bold)
        .tags()
        .iter()
        .any(|t| t.weight() >= 700)
        || !buffer
            .iter_at_offset(link)
            .tags()
            .iter()
            .any(|t| t.underline() == gtk::pango::Underline::Single)
    {
        return Err("CRLF text routing lost styles/link ranges".into());
    }
    if crlf.height() > 320 {
        return Err("Short CRLF paragraph allocated an unbounded viewport".into());
    }
    crate::window_capture::capture(&ui.window, &root.join("inline-crlf-reader.png")).await?;
    buffer.place_cursor(&buffer.iter_at_offset(link + 1));
    crlf.activate_action("rich.open-link", None)
        .map_err(|e| e.to_string())?;
    until("native CRLF link", || {
        ui.reader.is_ready()
            && ui
                .reader
                .location()
                .is_some_and(|(_, p)| p.ends_with("nested/note.md"))
    })
    .await?;
    chat.emit_by_name::<bool>("activate-link", &[&"nested/note.md"]);
    until("native local link navigation", || {
        ui.reader.is_ready()
            && ui
                .reader
                .location()
                .is_some_and(|(_, path)| path.ends_with("nested/note.md"))
    })
    .await?;
    let rich_paragraph = rich_acceptance::run(ui, root).await?;
    if ui.composer_text() != "Unsent native math draft 中文😀 " {
        return Err("Math viewing changed the conversation draft".into());
    }
    let mut result = json!({"ok":true,"chat":true,"reader":true,"table":true,"nativeCopyExact":true,"independentWaylandClipboard":true,"fontResizeSelection":true,"themes":true,
        "currencyLiteral":true,"errorSourceRetained":true,"sourcePreviewSwitch":true,"localLinks":true,"comment":comment,"recycling":recycling,
        "excerptMath":true,"excerptCopyExact":true,"clippedMathLiteral":true,"completeCellCopyExact":true,"excerptComment":excerpt_comment,"excerptRecycling":excerpt_recycling,
        "continuousTableMath":true,"continuousMathVisible":true,"continuousExcerptCopy":true,"continuousComment":flow_comment,"continuousUnmapCleanup":true,
        "responsiveTable":true,"resizePreservesState":true,"idleLayout":true,"nativePaneDrag":true,"activeBeforeDrag":active_before_drag,"pointerEvents":*pointer_events.borrow(),"readerHeaderReachable":true,"paneWidths":widths,"draftRetained":true,"initialMappedMathMs":first_ms});
    if let Value::Object(extra) = json!({"richParagraph":rich_paragraph,"crlfTextView":true,"crlfCopyExact":true,"crlfSourceComment":crlf_comment,"crlfStyles":true,"crlfLink":true,"fullCellFind":true,"cellFindKeyboard":true,"pagedCellMath":true,"pagedCellMeasurement":cell_measurement,"pagedCellSelectionExact":true,"pagedCellThemes":true,"pagedCellUnmapCleanup":true,"pagedCellComment":paged_cell_comment,"pagedHeaderMath":true,"pagedHeaderComment":paged_header_comment})
    {
        result.as_object_mut().unwrap().extend(extra);
    }
    Ok(result)
}
