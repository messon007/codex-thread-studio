//! Observe actual GTK glyph/viewport coordinates across native view replacement.
use super::*;

async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !test() {
        if start.elapsed() > Duration::from_secs(20) {
            return Err(format!("Reading position timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
fn paragraph(root: &gtk::Widget) -> Option<gtk::TextView> {
    if root.is_mapped() && root.widget_name() == "studio-rich-text" {
        if let Some(view) = root.downcast_ref::<gtk::TextView>() {
            let b = view.buffer();
            if b.text(&b.start_iter(), &b.end_iter(), false)
                .contains("rich-layout-begin")
            {
                return Some(view.clone());
            }
        }
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if let Some(v) = paragraph(&widget) {
            return Some(v);
        }
    }
    None
}
#[derive(Clone, Copy)]
struct Anchor {
    character: i32,
    y: i32,
    width: i32,
    insertion: i32,
    selection: i32,
}
async fn first_page(view: &gtk::TextView) -> Result<(), String> {
    // First is disabled when already on page 1. Exercise a real page change,
    // rather than treating activation of a disabled action as navigation.
    view.activate_action("rich.last-page", None)
        .map_err(|e| e.to_string())?;
    view.activate_action("rich.first-page", None)
        .map_err(|e| e.to_string())?;
    until("explicit page change starts at top", || {
        view.visible_rect().y() <= 8
    })
    .await?;
    glib::timeout_future(Duration::from_millis(100)).await;
    if view.visible_rect().y() > 8 {
        return Err("Old restore overrode explicit page navigation".into());
    }
    Ok(())
}
async fn seek(view: &gtk::TextView) -> Result<Anchor, String> {
    view.activate_action("rich.first-page", None)
        .map_err(|e| e.to_string())?;
    glib::timeout_future(Duration::from_millis(100)).await;
    let b = view.buffer();
    let target = b.iter_at_offset(b.char_count() / 2);
    b.place_cursor(&target);
    view.scroll_to_mark(&b.get_insert(), 0.0, true, 0.0, 0.25);
    until("middle of paragraph is visible", || {
        let glyph = view.iter_location(&target);
        let visible = view.visible_rect();
        visible.y() > 100
            && glyph.height() > 0
            && glyph.y() >= visible.y()
            && glyph.y() + glyph.height() <= visible.y() + visible.height()
    })
    .await?;
    glib::timeout_future(Duration::from_millis(100)).await;
    b.select_range(
        &target,
        &b.iter_at_offset((target.offset() + 15).min(b.char_count())),
    );
    // Selection can finish validating a wrapped line and move the viewport a
    // few pixels. Record the settled position before yielding to history work.
    glib::timeout_future(Duration::from_millis(100)).await;
    let visible = view.visible_rect();
    let iter = view
        .iter_at_location(view.left_margin(), visible.y() + 1)
        .ok_or("Reading anchor has no visible text")?;
    Ok(Anchor {
        character: iter.offset(),
        y: visible.y(),
        width: view.width(),
        insertion: b.cursor_position(),
        selection: b.iter_at_mark(&b.selection_bound()).offset(),
    })
}
fn at(view: &gtk::TextView, anchor: Anchor, reflow: bool) -> bool {
    if !view.is_mapped()
        || view.width() < 100
        || view.height() < 100
        || view.buffer().cursor_position() != anchor.insertion
        || view
            .buffer()
            .iter_at_mark(&view.buffer().selection_bound())
            .offset()
            != anchor.selection
    {
        return false;
    }
    let glyph = view.iter_location(&view.buffer().iter_at_offset(anchor.character));
    let visible = view.visible_rect();
    glyph.height() > 0
        && if reflow {
            view.width() != anchor.width && (glyph.y() - visible.y()).abs() <= glyph.height() + 2
        } else {
            (visible.y() - anchor.y).abs() <= 2
        }
}
async fn replacement(
    root: &gtk::Widget,
    old: &gtk::TextView,
    anchor: Anchor,
    reflow: bool,
) -> Result<gtk::TextView, String> {
    let result = until("replacement retains visible text", || {
        paragraph(root).is_some_and(|v| v != *old && at(&v, anchor, reflow))
    })
    .await;
    if let Err(error) = result {
        let observed = paragraph(root).map(|v| {
            (
                v == *old,
                v.width(),
                v.visible_rect().y(),
                v.iter_location(&v.buffer().iter_at_offset(anchor.character))
                    .y(),
            )
        });
        return Err(format!(
            "{error}: expected character={} y={} width={} reflow={reflow}; observed={observed:?}",
            anchor.character, anchor.y, anchor.width
        ));
    }
    let view = paragraph(root).unwrap();
    glib::timeout_future(Duration::from_millis(100)).await;
    if !at(&view, anchor, reflow) {
        return Err("Reading position moved after settling".into());
    }
    Ok(view)
}
pub async fn chat(ui: &Rc<Ui>, view: &gtk::TextView) -> Result<(gtk::TextView, Value), String> {
    let anchor = seek(view).await?;
    let outer = ui.transcript_scroll.vadjustment().value();
    let original = ui.state.borrow().items.clone();
    // A real history read prepares a new presentation on a worker. A shallow
    // clone now correctly retains GTK widgets, so use independently parsed,
    // value-equal blocks to keep exercising actual widget replacement here.
    let input = original.clone();
    let equal = gio::spawn_blocking(move || {
        input
            .iter()
            .map(|i| {
                let mut item = (**i).clone();
                if item.rich.is_some() {
                    item.rich = Some(crate::chat_content::Content::parse(&item.body)?);
                }
                Ok(Arc::new(item))
            })
            .collect::<Result<Vec<_>, String>>()
    })
    .await
    .map_err(|_| "History presentation fixture worker stopped")??;
    crate::chat_ui::reconcile(&ui.transcript, &equal);
    let next = replacement(ui.transcript_view.upcast_ref(), view, anchor, false).await?;
    // Hold the discarded object alive and emit its callbacks deliberately.
    // A detached widget may no longer resolve its ancestor action group.
    let _ = view.activate_action("rich.last-page", None);
    if let Some(adjustment) = view.vadjustment() {
        adjustment.set_value(0.0);
    }
    crate::chat_ui::reconcile(&ui.transcript, &original);
    let current = replacement(ui.transcript_view.upcast_ref(), &next, anchor, false).await?;
    if (outer - ui.transcript_scroll.vadjustment().value()).abs() > 1.0 {
        return Err("Restoration moved the outer transcript".into());
    }
    let size = (ui.window.width(), ui.window.height());
    ui.window.set_default_size(1000, size.1);
    crate::chat_ui::reconcile(&ui.transcript, &equal);
    let narrow = replacement(ui.transcript_view.upcast_ref(), &current, anchor, true).await?;
    let narrow_width = narrow.width();
    ui.window.set_default_size(size.0, size.1);
    crate::chat_ui::reconcile(&ui.transcript, &original);
    until("window size restores", || ui.window.width() == size.0).await?;
    until("paragraph remains available", || {
        paragraph(ui.transcript_view.upcast_ref()).is_some()
    })
    .await?;
    let current = paragraph(ui.transcript_view.upcast_ref()).unwrap();
    first_page(&current).await?;
    let top = Anchor {
        character: 0,
        y: current.visible_rect().y(),
        width: current.width(),
        insertion: current.buffer().cursor_position(),
        selection: current
            .buffer()
            .iter_at_mark(&current.buffer().selection_bound())
            .offset(),
    };
    crate::chat_ui::reconcile(&ui.transcript, &equal);
    let next = replacement(ui.transcript_view.upcast_ref(), &current, top, false).await?;
    crate::chat_ui::reconcile(&ui.transcript, &original);
    let current = replacement(ui.transcript_view.upcast_ref(), &next, top, false).await?;
    Ok((
        current,
        json!({"equalRefresh":true,"obsoleteCallbacksIgnored":true,"outerScrollStable":true,"explicitPageReset":true,"widthReflow":true,"topMargin":true,"cursorAndSelection":true,"anchorCharacter":anchor.character,"scrollY":anchor.y,"oldWidth":anchor.width,"narrowWidth":narrow_width}),
    ))
}
pub async fn reader(ui: &Rc<Ui>, view: &gtk::TextView) -> Result<(gtk::TextView, Value), String> {
    let anchor = seek(view).await?;
    ui.reader.show_source();
    until("source view", || {
        ui.reader.source_mode() && ui.reader.is_ready()
    })
    .await?;
    ui.reader.show_preview();
    let next = replacement(ui.reader.widget.upcast_ref(), view, anchor, false).await?;
    // A detached widget may no longer resolve its ancestor action group.
    let _ = view.activate_action("rich.last-page", None);
    if let Some(adjustment) = view.vadjustment() {
        adjustment.set_value(0.0);
    }
    ui.reader.show_source();
    until("source view again", || {
        ui.reader.source_mode() && ui.reader.is_ready()
    })
    .await?;
    ui.reader.show_preview();
    let current = replacement(ui.reader.widget.upcast_ref(), &next, anchor, false).await?;
    first_page(&current).await?;
    seek(&current).await?;
    ui.reader.smoke_jump_to_first_line();
    until("explicit source-line jump resets cached paragraph", || {
        ui.reader.is_ready()
            && paragraph(ui.reader.widget.upcast_ref())
                .is_some_and(|v| v != current && v.visible_rect().y() <= 8)
    })
    .await?;
    let current = paragraph(ui.reader.widget.upcast_ref()).unwrap();
    glib::timeout_future(Duration::from_millis(100)).await;
    if current.visible_rect().y() > 8 {
        return Err("Cached reading position overrode a source-line jump".into());
    }
    Ok((
        current,
        json!({"sourcePreviewRoundTrip":true,"obsoleteCallbacksIgnored":true,"explicitPageReset":true,"explicitLineJump":true,"cursorAndSelection":true,"anchorCharacter":anchor.character,"scrollY":anchor.y}),
    ))
}
