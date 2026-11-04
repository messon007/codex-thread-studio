//! Owned Wayland input against the shared picker and actual flow table.
use super::*;
use serde_json::{json, Value};
use std::{
    path::Path,
    time::{Duration, Instant},
};
use studio_markdown_native::Fragment;

async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let started = Instant::now();
    while !test() {
        if started.elapsed() > Duration::from_secs(10) {
            return Err(format!("Column picker timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
fn all<T: IsA<gtk::Widget> + glib::object::ObjectType>(widget: &gtk::Widget) -> Vec<T> {
    let mut result = widget
        .clone()
        .downcast::<T>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut child = widget.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        result.extend(all::<T>(&widget));
    }
    result
}
fn checks(popover: &gtk::Popover) -> Vec<gtk::CheckButton> {
    all(popover.upcast_ref())
}
async fn space(check: &gtk::CheckButton, root: &Path) -> Result<(), String> {
    if !check.grab_focus() {
        return Err("Checkbox cannot receive keyboard focus".into());
    }
    crate::smoke_input::request(root, json!({"action":"key","key":"space"})).await?;
    Ok(())
}
async fn show(
    menu: &gtk::MenuButton,
    window: &adw::ApplicationWindow,
    root: &Path,
) -> Result<gtk::Popover, String> {
    if !menu.grab_focus() {
        return Err("Column menu cannot receive focus".into());
    }
    crate::smoke_input::request(root, json!({"action":"key","key":"space"})).await?;
    let popover = menu.popover().ok_or("No column popover")?;
    if let Err(error) = until("mapped choices", || {
        checks(&popover).iter().any(|c| c.is_mapped())
    })
    .await
    {
        crate::window_capture::capture(window, &root.join("columns-failed.png")).await?;
        return Err(format!(
            "{error}; visible={}, mapped={}, widgets={}",
            popover.is_visible(),
            popover.is_mapped(),
            checks(&popover).len()
        ));
    }
    Ok(popover)
}
async fn choice(
    list: &gtk::ListView,
    popover: &gtk::Popover,
    index: u32,
    title: &str,
) -> Result<gtk::CheckButton, String> {
    list.scroll_to(index, gtk::ListScrollFlags::FOCUS, None);
    until("recycled choice", || {
        checks(popover)
            .iter()
            .any(|c| c.is_mapped() && c.label().as_deref() == Some(title))
    })
    .await?;
    checks(popover)
        .into_iter()
        .find(|c| c.is_mapped() && c.label().as_deref() == Some(title))
        .ok_or("Choice not realized".into())
}
fn row_line(view: &gtk::ColumnView) -> Option<usize> {
    let item = view
        .model()?
        .item(0)?
        .downcast::<glib::BoxedAnyObject>()
        .ok()?;
    let row = item.borrow::<studio_markdown_native::Row>();
    Some(row.source.line)
}
fn fragment(text: String) -> Fragment {
    Fragment {
        page: 0,
        document_id: "column-probe".into(),
        revision: "a".repeat(64),
        metadata: studio_markdown_native::FragmentMetadata {
            start: 0,
            end: text.len(),
            line: 1,
            rows: 0,
            row_start: 0,
            table: None,
        },
        source: studio_markdown_native::SourceWindow {
            start: 0,
            end: text.len(),
            line: 1,
            end_line: text.lines().count(),
            text,
            projection: vec![],
            references: vec![],
        },
        header: None,
    }
}
pub async fn run(window: &adw::ApplicationWindow, root: &Path) -> Result<Value, String> {
    crate::language::set("en-US", "en-US")?;
    crate::i18n_ui::apply();
    window.fullscreen();
    let menu = gtk::MenuButton::new();
    let host = gtk::Box::new(gtk::Orientation::Vertical, 8);
    menu.set_halign(gtk::Align::Start);
    host.append(&menu);
    host.set_margin_start(20);
    host.set_margin_top(20);
    let current = Rc::new(RefCell::new((0..512).collect::<Vec<_>>()));
    let changes = Rc::new(Cell::new(0));
    let live = Rc::new(Cell::new(true));
    let started = Instant::now();
    crate::table_columns_ui::install(
        &menu,
        (0..2000).map(|i| format!("Column {i}")).collect(),
        current.borrow().clone(),
        512,
        Rc::new({
            let (current, changes, live) = (current.clone(), changes.clone(), live.clone());
            move |indices| {
                if !live.get() {
                    return false;
                }
                *current.borrow_mut() = indices.to_vec();
                changes.set(changes.get() + 1);
                true
            }
        }),
    );
    let build_us = started.elapsed().as_micros();
    window.set_content(Some(&host));
    window.present();
    until("picker button", || {
        window.width() == 1600 && window.height() == 1000 && menu.is_mapped() && menu.width() > 0
    })
    .await?;
    crate::smoke_input::request(root, json!({"action":"focus"})).await?;
    let opened = Instant::now();
    let popover = show(&menu, window, root).await?;
    let open_ms = opened.elapsed().as_millis();
    let list = all::<gtk::ListView>(popover.upcast_ref())
        .into_iter()
        .next()
        .ok_or("No virtual choices")?;
    let initial_widgets = checks(&popover).len();
    if list.model().unwrap().n_items() != 2000 || initial_widgets >= 1000 {
        return Err("Picker eagerly constructed all 2,000 checkboxes".into());
    }
    let last = choice(&list, &popover, 1999, "Column 1999").await?;
    space(&last, root).await?;
    if last.is_active() || current.borrow().contains(&1999) {
        return Err("Picker exceeded 512 selected columns".into());
    }
    let first = choice(&list, &popover, 0, "Column 0").await?;
    space(&first, root).await?;
    let last = choice(&list, &popover, 1999, "Column 1999").await?;
    space(&last, root).await?;
    if current.borrow().len() != 512
        || current.borrow().contains(&0)
        || !current.borrow().contains(&1999)
    {
        return Err("Recycled checkbox changed a stale column index".into());
    }
    let peak_widgets = initial_widgets.max(checks(&popover).len());
    if peak_widgets >= 1000 {
        return Err("Scrolling accumulated column checkbox widgets".into());
    }
    crate::window_capture::capture_widget(window, &popover, &root.join("columns-picker.png"))
        .await?;
    live.set(false);
    let before = changes.get();
    space(&last, root).await?;
    if !last.is_active() || changes.get() != before {
        return Err("Expired source accepted a column proposal".into());
    }
    popover.popdown();
    until("picker closed", || !last.is_mapped()).await?;
    window.set_content(Some(&gtk::Box::new(gtk::Orientation::Vertical, 0)));
    last.set_active(false);
    if changes.get() != before || !current.borrow().contains(&1999) || current.borrow().len() != 512
    {
        return Err("Unmapped column choice mutated state".into());
    }

    let text = format!(
        "| Open | Columns | 中文😀 |\n|---|---|---|\n{}",
        (0..40)
            .map(|i| format!("| row{i} | hidden{i} | 中文😀{i} |\n"))
            .collect::<String>()
    );
    let prepared = gio::spawn_blocking(move || prepare(&fragment(text)))
        .await
        .map_err(|_| "Table worker stopped")??;
    let flow = view(prepared, Rc::new(|_| {}))?;
    let scroll = gtk::ScrolledWindow::builder()
        .child(&flow.widget)
        .vexpand(true)
        .build();
    window.set_content(Some(&scroll));
    until("flow table", || {
        all::<gtk::ColumnView>(flow.widget.upcast_ref())
            .first()
            .is_some_and(|t| t.width() > 0)
    })
    .await?;
    let table = all::<gtk::ColumnView>(flow.widget.upcast_ref()).remove(0);
    let first = table
        .columns()
        .item(0)
        .and_downcast::<gtk::ColumnViewColumn>()
        .unwrap();
    first.set_fixed_width(310);
    let next = all::<gtk::Button>(flow.widget.upcast_ref())
        .into_iter()
        .find(|b| b.widget_name() == "probe-table-next")
        .ok_or("Table Next missing")?;
    crate::smoke_input::request(root, json!({"action":"focus"})).await?;
    if !next.grab_focus() {
        return Err("Table Next cannot receive focus".into());
    }
    crate::smoke_input::request(root, json!({"action":"key","key":"space"})).await?;
    until("next page source row", || row_line(&table) == Some(19)).await?;
    let menu = all::<gtk::MenuButton>(flow.widget.upcast_ref())
        .into_iter()
        .find(|b| b.widget_name() == "studio-table-columns")
        .ok_or("Flow column picker missing")?;
    let popover = show(&menu, window, root).await?;
    until("three column choices", || checks(&popover).len() == 3).await?;
    let options = checks(&popover);
    space(&options[1], root).await?;
    if table
        .columns()
        .item(1)
        .and_downcast::<gtk::ColumnViewColumn>()
        .unwrap()
        .is_visible()
        || row_line(&table) != Some(19)
        || first.fixed_width() != 310
    {
        return Err("Hiding a column reset rows or resized width".into());
    }
    space(&options[0], root).await?;
    space(&options[2], root).await?;
    if !options[2].is_active() || options[..2].iter().any(|c| c.is_active()) {
        return Err("Flow table allowed zero visible columns".into());
    }
    space(&options[0], root).await?;
    if !first.is_visible() || first.fixed_width() != 310 {
        return Err("Restoring a column lost its width".into());
    }
    crate::language::set("zh-CN", "en-US")?;
    crate::i18n_ui::apply();
    if menu.label().as_deref() != Some("列 2/3")
        || options[0].label().as_deref() != Some("Open")
        || options[1].label().as_deref() != Some("Columns")
        || !all::<gtk::Label>(flow.widget.upcast_ref())
            .iter()
            .any(|l| l.text().contains("第 17–32 行，共 40 行"))
    {
        return Err(format!("Live table language changed source headers or lost row status: menu={:?}; choices={:?}; labels={:?}",menu.label(),options.iter().map(|c|c.label()).collect::<Vec<_>>(),all::<gtk::Label>(flow.widget.upcast_ref()).iter().map(|l|l.text()).filter(|s|s.contains("rows") || s.contains('第')).collect::<Vec<_>>()));
    }
    crate::window_capture::capture_widget(
        window,
        &popover,
        &root.join("columns-source-labels.png"),
    )
    .await?;
    popover.popdown();
    until("flow picker closed", || !options[0].is_mapped()).await?;
    flow.widget.grab_focus();
    let buffer = flow.widget.buffer();
    buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
    crate::smoke_input::request(root, json!({"action":"key","key":"copy"})).await?;
    until("flow full fragment copy", || flow.copied.borrow().is_some()).await?;
    let expected = flow
        .document
        .select(0..flow.document.characters, flow::BUFFER_BYTES)?
        .text;
    if crate::smoke_input::request(root, json!({"action":"read"})).await?["text"] != expected
        || !expected.contains("hidden39")
    {
        return Err("Visual column filtering discarded source/copy data".into());
    }
    crate::window_capture::capture(window, &root.join("columns-flow.png")).await?;
    crate::language::set("en-US", "en-US")?;
    crate::i18n_ui::apply();
    Ok(
        json!({"choices":2000,"initialCheckboxes":initial_widgets,"peakCheckboxes":peak_widgets,"constructionUs":build_us,"popupMs":open_ms,"nativeKeyboard":true,"recycledIdentity":true,"selectionLimit":512,"expiredSourceRejected":true,"unmappedRejected":true,"atLeastOne":true,"flowRowsAndWidthRetained":true,"liveLanguage":true,"literalHeaders":true,"completeFragmentCopy":true}),
    )
}
