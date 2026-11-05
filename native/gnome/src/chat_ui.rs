//! Flat, virtualized conversation blocks; no HTML or nested message DOM.
use crate::{
    content_widgets::{self, OpenLink, Styles},
    i18n_ui::{self, UiText},
    model::Item,
};
use adw::prelude::*;
use gtk::{gio, glib};
use std::{cell::RefCell, rc::Rc, sync::Arc};
use studio_markdown_native::{BlockKind, Cell as MarkdownCell};
pub type FavoriteAction = Rc<dyn Fn(Arc<Item>, Option<String>)>;
pub type CommentAction = Rc<dyn Fn(Arc<Item>, Option<String>)>;
#[path = "chat_comment_selection.rs"]
mod comment_selection;

#[cfg(debug_assertions)]
pub struct StreamProbe {
    rows: Vec<glib::BoxedAnyObject>,
    table: Rc<Table>,
    paragraph: gtk::TextView,
    offset: i32,
    selection: (i32, i32),
}
#[cfg(debug_assertions)]
impl StreamProbe {
    pub fn verify(&self, store: &gio::ListStore, current: &Arc<Item>) -> Result<(), String> {
        for object in &self.rows {
            let row = object.borrow::<Row>();
            if !Arc::ptr_eq(&row.item, current)
                || !Arc::ptr_eq(&row.latest.borrow(), current)
                || !(0..store.n_items())
                    .any(|i| store.item(i).as_ref() == Some(object.upcast_ref()))
            {
                return Err("A streaming block or its action lost the current message".into());
            }
        }
        if self.table.state.borrow().row != 50
            || self.table.state.borrow().widths.get(&0) != Some(&315)
            || self.table.state.borrow().wrap
        {
            return Err("Streaming reset table reading controls".into());
        }
        let b = self.paragraph.buffer();
        if !self.paragraph.is_mapped()
            || (self.paragraph.visible_rect().y() - self.offset).abs() > 2
            || b.cursor_position() != self.selection.0
            || b.iter_at_mark(&b.selection_bound()).offset() != self.selection.1
        {
            return Err("Streaming recreated or scrolled an unchanged paragraph".into());
        }
        Ok(())
    }
}
#[cfg(debug_assertions)]
pub async fn stream_probe(
    store: &gio::ListStore,
    view: &gtk::ListView,
) -> Result<StreamProbe, String> {
    let mut rows = Vec::new();
    let mut table = None;
    let mut paragraph = None;
    for i in 0..store.n_items() {
        let object = store
            .item(i)
            .and_downcast::<glib::BoxedAnyObject>()
            .unwrap();
        let row = object.borrow::<Row>();
        if row.item.id == "stream-assistant" {
            if let Part::Block(index) = row.part {
                if index < 5 {
                    rows.push(object.clone());
                }
                if matches!(
                    &row.item.rich.as_ref().unwrap().rows[index].block.kind,
                    BlockKind::Table { .. }
                ) {
                    view.scroll_to(i, gtk::ListScrollFlags::NONE, None);
                    table = Some(row.table.clone());
                }
                if index == 1 {
                    paragraph = Some((i, row.paragraph.clone()));
                }
            }
        }
    }
    let table = table.ok_or("Streaming table missing")?;
    let start = std::time::Instant::now();
    while table.borrow().live.upgrade().is_none() {
        if start.elapsed() > std::time::Duration::from_secs(5) {
            return Err("Streaming table is not mapped".into());
        }
        glib::timeout_future(std::time::Duration::from_millis(20)).await;
    }
    let table = table.borrow().live.upgrade().unwrap();
    table.next.emit_clicked();
    table.wrap.set_active(false);
    table
        .view
        .columns()
        .item(0)
        .and_downcast::<gtk::ColumnViewColumn>()
        .unwrap()
        .set_fixed_width(315);
    let (index, paragraph) = paragraph.ok_or("Streaming paragraph missing")?;
    view.scroll_to(index, gtk::ListScrollFlags::NONE, None);
    let start = std::time::Instant::now();
    while paragraph.smoke_view().is_none_or(|p| !p.is_mapped()) {
        if start.elapsed() > std::time::Duration::from_secs(5) {
            return Err("Streaming paragraph is not mapped".into());
        }
        glib::timeout_future(std::time::Duration::from_millis(20)).await;
    }
    let paragraph = paragraph.smoke_view().unwrap();
    let b = paragraph.buffer();
    let a = b.iter_at_offset(b.char_count() / 2);
    let z = b.iter_at_offset(a.offset() + 10);
    b.select_range(&a, &z);
    paragraph.scroll_to_mark(&b.get_insert(), 0.0, true, 0.0, 0.25);
    glib::timeout_future(std::time::Duration::from_millis(200)).await;
    Ok(StreamProbe {
        rows,
        table,
        offset: paragraph.visible_rect().y(),
        selection: (a.offset(), z.offset()),
        paragraph,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Part {
    Header,
    Plain,
    Block(usize),
    Attachment(usize),
}
#[derive(Default)]
struct TableState {
    widths: std::collections::HashMap<usize, i32>,
    row: usize,
    columns: Vec<usize>,
    wrap: bool,
    pending_row: Option<usize>,
    live: std::rc::Weak<Table>,
}
pub struct Row {
    pub item: Arc<Item>,
    latest: Rc<RefCell<Arc<Item>>>,
    pub part: Part,
    table: Rc<RefCell<TableState>>,
    paragraph: Rc<content_widgets::ParagraphState>,
}
impl Row {
    fn new(item: Arc<Item>, part: Part) -> Self {
        Self {
            latest: Rc::new(RefCell::new(item.clone())),
            item,
            part,
            paragraph: Rc::new(content_widgets::ParagraphState::default()),
            table: Rc::new(RefCell::new(TableState {
                wrap: true,
                ..Default::default()
            })),
        }
    }
}
pub fn parts(item: &Item) -> impl Iterator<Item = Part> {
    let rows = item.rich.as_ref().map_or(0, |content| content.rows.len());
    std::iter::once(Part::Header)
        .chain((rows == 0 && !item.body.is_empty()).then_some(Part::Plain))
        .chain((0..rows).map(Part::Block))
        .chain((0..item.images.len()).map(Part::Attachment))
}
pub fn reconcile(store: &gio::ListStore, items: &[Arc<Item>]) {
    // Stable immutable blocks keep widgets alive and refresh their action's
    // current-message handle. Independently parsed history can instead rebind
    // value-equal blocks while retaining the table/paragraph reading state.
    let old = store.n_items() as usize;
    let mut first = 0;
    let mut prefix = 0;
    for item in items {
        let count = parts(item).count();
        if first + count > old {
            break;
        }
        let object = store
            .item(first as u32)
            .and_downcast::<glib::BoxedAnyObject>()
            .unwrap();
        let row = object.borrow::<Row>();
        if row.part != Part::Header || !Arc::ptr_eq(&row.item, item) {
            break;
        }
        first += count;
        prefix += 1;
    }
    if prefix == items.len() {
        if first < old {
            store.splice(
                first as u32,
                (old - first) as u32,
                &[] as &[glib::BoxedAnyObject],
            );
        }
        return;
    }
    // Only inspect the changed suffix. Streaming/appending a new reply keeps
    // the unchanged prefix and its widgets entirely out of reconciliation.
    let mut groups =
        std::collections::HashMap::<(String, String, String), Vec<glib::BoxedAnyObject>>::new();
    for index in first..old {
        let object = store
            .item(index as u32)
            .and_downcast::<glib::BoxedAnyObject>()
            .unwrap();
        let key = {
            let row = object.borrow::<Row>();
            (
                row.item.id.clone(),
                row.item.turn_id.clone(),
                row.item.role.clone(),
            )
        };
        groups.entry(key).or_default().push(object);
    }
    let mut tail = Vec::new();
    for item in &items[prefix..] {
        let key = (item.id.clone(), item.turn_id.clone(), item.role.clone());
        let previous = groups.remove(&key).unwrap_or_default();
        let equal = previous
            .first()
            .is_some_and(|object| object.borrow::<Row>().item == *item);
        let mut previous = previous
            .into_iter()
            .map(|object| {
                let part = object.borrow::<Row>().part;
                (part, object)
            })
            .collect::<std::collections::HashMap<_, _>>();
        for part in parts(item) {
            if let Some(object) = previous.remove(&part) {
                let row = object.borrow::<Row>();
                if Arc::ptr_eq(&row.item, item) {
                    drop(row);
                    tail.push(object);
                    continue;
                }
                let retained = equal
                    || match part {
                        Part::Block(index) => item
                            .rich
                            .as_ref()
                            .zip(row.item.rich.as_ref())
                            .is_some_and(|(new, old)| new.retain_state(old, index)),
                        _ => false,
                    };
                let stable = match part {
                    Part::Block(index) => {
                        item.rich
                            .as_ref()
                            .zip(row.item.rich.as_ref())
                            .is_some_and(|(a, b)| {
                                a.rows
                                    .get(index)
                                    .zip(b.rows.get(index))
                                    .is_some_and(|(a, b)| Arc::ptr_eq(a, b))
                            })
                            && row.item.origin == item.origin
                            && row.item.images == item.images
                    }
                    _ => false,
                };
                if stable {
                    row.latest.replace(item.clone());
                    drop(row);
                    object.borrow_mut::<Row>().item = item.clone();
                    tail.push(object);
                    continue;
                }
                if retained {
                    tail.push(glib::BoxedAnyObject::new(Row {
                        latest: Rc::new(RefCell::new(item.clone())),
                        item: item.clone(),
                        part,
                        table: row.table.clone(),
                        paragraph: row.paragraph.clone(),
                    }));
                    continue;
                }
            }
            tail.push(glib::BoxedAnyObject::new(Row::new(item.clone(), part)));
        }
    }
    // Retained block objects are anchors. Splice only changed gaps so GTK keeps
    // mapped paragraphs, tables and running diagram jobs alive during a stream.
    let old_objects = (first..old)
        .map(|i| {
            store
                .item(i as u32)
                .and_downcast::<glib::BoxedAnyObject>()
                .unwrap()
        })
        .collect::<Vec<_>>();
    let positions = old_objects
        .iter()
        .enumerate()
        .map(|(i, o)| (o.clone(), i))
        .collect::<std::collections::HashMap<_, _>>();
    let (mut previous, mut next, mut at) = (0, 0, first);
    for (index, object) in tail.iter().enumerate() {
        if let Some(&position) = positions.get(object).filter(|&&p| p >= previous) {
            if previous != position || next != index {
                store.splice(at as u32, (position - previous) as u32, &tail[next..index]);
            }
            at += index - next + 1;
            previous = position + 1;
            next = index + 1;
        }
    }
    if previous != old_objects.len() || next != tail.len() {
        store.splice(
            at as u32,
            (old_objects.len() - previous) as u32,
            &tail[next..],
        );
    }
}

pub fn locate(store: &gio::ListStore, hit: &crate::session_search::Occurrence) -> Option<u32> {
    let mut header = None;
    for index in 0..store.n_items() {
        let object = store.item(index).and_downcast::<glib::BoxedAnyObject>()?;
        let row = object.borrow::<Row>();
        if row.item.id != hit.item_id || row.item.turn_id != hit.turn_id {
            continue;
        }
        if let Some(source) = &hit.source {
            if row.item.body.get(source.clone()) != hit.snippet.get(hit.highlight.clone()) {
                return None;
            }
        }
        if row.part == Part::Header {
            header = Some(index);
        }
        if row.part == Part::Plain
            || hit
                .block
                .is_some_and(|block| row.part == Part::Block(block))
        {
            if let Some(page) = hit.paragraph_page {
                if let Some(range) = &hit.paragraph_range {
                    row.paragraph.focus(page, range.clone());
                } else {
                    row.paragraph.select(page);
                }
            } else if let Part::Block(index) = row.part {
                if row.item.rich.as_ref().is_some_and(|content| {
                    match &content.rows[index].block.kind {
                        BlockKind::Paragraph(text) | BlockKind::Heading { content: text, .. } => {
                            text.attributes.is_some()
                        }
                        _ => false,
                    }
                }) {
                    row.paragraph.source_match();
                }
            }
            if let Some(target) = hit.table_row {
                let state = row.table.borrow();
                let size = (512 / state.columns.len().max(1)).clamp(1, 50);
                let page = target / size * size;
                let live = state.live.upgrade();
                drop(state);
                row.table.borrow_mut().row = page;
                row.table.borrow_mut().pending_row = Some(target);
                if let Some(table) = live {
                    table.page(page);
                    table.focus_target();
                }
            }
            return Some(index);
        }
    }
    header
}

#[cfg(debug_assertions)]
pub fn target_visible(store: &gio::ListStore, hit: &crate::session_search::Occurrence) -> bool {
    let index = (0..store.n_items()).find(|index| {
        let object = store
            .item(*index)
            .and_downcast::<glib::BoxedAnyObject>()
            .unwrap();
        let row = object.borrow::<Row>();
        row.item.id == hit.item_id
            && row.item.turn_id == hit.turn_id
            && hit
                .block
                .is_some_and(|block| row.part == Part::Block(block))
    });
    let Some(index) = index else {
        return false;
    };
    let object = store
        .item(index)
        .and_downcast::<glib::BoxedAnyObject>()
        .unwrap();
    let row = object.borrow::<Row>();
    if hit.paragraph_range.is_some() {
        return hit.snippet.get(hit.highlight.clone()).is_some_and(|text| {
            row.paragraph
                .target_visible(text, hit.paragraph_range.as_ref().unwrap())
        });
    }
    if hit.table_row.is_none() {
        return true;
    }
    let Some(table) = row.table.borrow().live.upgrade() else {
        return false;
    };
    let Some(text) = hit.snippet.get(hit.highlight.clone()) else {
        return false;
    };
    let visible = table
        .labels
        .borrow()
        .iter()
        .filter_map(glib::WeakRef::upgrade)
        .any(|label| {
            label.text().contains(text)
                && label.compute_bounds(&table.view).is_some_and(|bounds| {
                    bounds.y() >= 0.0
                        && bounds.y() + bounds.height() <= table.view.height() as f32 + 1.0
                })
        });
    visible
}

pub enum ImageContent<'a> {
    Cell(&'a studio_markdown_native::RichText, bool),
    Markdown(
        &'a studio_markdown_native::RichText,
        Rc<content_widgets::ParagraphState>,
    ),
    Attachment(&'a crate::conversation_images::Image),
}
pub type ImageAction = Rc<dyn Fn(&Arc<Item>, ImageContent<'_>, OpenLink) -> gtk::Widget>;
pub type MessageLink = Rc<dyn Fn(&Arc<Item>, &str)>;
pub type SourceAction = Rc<dyn Fn(Arc<Item>)>;
pub fn factory(
    open: OpenLink,
    styles: Rc<Styles>,
    diagrams: Rc<crate::chat_diagrams::Renderer>,
    favorite: Option<FavoriteAction>,
    images: Option<ImageAction>,
) -> gtk::SignalListItemFactory {
    factory_observed(
        Rc::new(move |_, uri| open(uri)),
        styles,
        diagrams,
        favorite,
        None,
        None,
        None,
        None,
        images,
    )
}
pub fn factory_observed(
    message_link: MessageLink,
    styles: Rc<Styles>,
    diagrams: Rc<crate::chat_diagrams::Renderer>,
    favorite: Option<FavoriteAction>,
    comment: Option<CommentAction>,
    source: Option<SourceAction>,
    visibility: Option<Rc<crate::chat_visibility::Visibility>>,
    target: Option<SourceAction>,
    images: Option<ImageAction>,
) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, object| {
        let item = object.downcast_ref::<gtk::ListItem>().unwrap();
        let row = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let clamp = crate::appearance_ui::reading_clamp(&row);
        row.add_css_class("studio-content");
        row.add_css_class("studio-message");
        item.set_child(Some(&clamp));
    });
    if let Some(visibility) = visibility.clone() {
        factory.connect_unbind(move |_, object| {
            let item = object.downcast_ref::<gtk::ListItem>().unwrap();
            if let Some(child) = item.child() {
                visibility.unbind(&child);
            }
        });
    }
    factory.connect_bind(move |_, object| {
        let item = object.downcast_ref::<gtk::ListItem>().unwrap();
        let data = item.item().and_downcast::<glib::BoxedAnyObject>().unwrap();
        let data = data.borrow::<Row>();
        let link_item = data.latest.clone();
        let message_link = message_link.clone();
        let open: OpenLink = Rc::new(move |uri| {
            let item=link_item.borrow().clone();
            message_link(&item,uri);
        });
        let favorite=favorite.as_ref().map(|action| {
            let action=action.clone();let latest=data.latest.clone();
            Rc::new(move |_item:Arc<Item>,selection:Option<String>|{
                let item=latest.borrow().clone();
                action(item,selection)
            }) as FavoriteAction
        });
        let comment=comment.as_ref().map(|action| {
            let action=action.clone();let latest=data.latest.clone();
            Rc::new(move |_item:Arc<Item>,selection:Option<String>|{
                let item=latest.borrow().clone();
                action(item,selection)
            }) as CommentAction
        });
        let clamp = item.child().and_downcast::<adw::Clamp>().unwrap();
        if let Some(visibility) = &visibility {
            visibility.bind(clamp.upcast_ref(), &data.item, data.part);
        }
        let row = clamp.child().and_downcast::<gtk::Box>().unwrap();
        while let Some(child) = row.first_child() {
            row.remove(&child);
        }
        row.remove_css_class("studio-user-message");
        row.remove_css_class("studio-assistant-message");
        if data.item.role == "You" {
            row.add_css_class("studio-user-message");
        } else if data.item.role == "Assistant" {
            row.add_css_class("studio-assistant-message");
        }
        row.set_margin_top(if data.part == Part::Header { 18 } else { 4 });
        row.set_margin_bottom(6);
        let content: gtk::Widget = match data.part {
            Part::Header => {
                let header = gtk::Box::new(gtk::Orientation::Horizontal, 4);
                header.add_css_class("studio-message-meta");
                let name = gtk::Label::builder()
                    .label(&data.item.role)
                    .xalign(0.0)
                    .hexpand(true)
                    .build();
                name.add_css_class("heading");
                if matches!(data.item.role.as_str(),"You"|"Assistant"|"Reasoning"|"Command"|"Files"|"Tool"|"Activity"|"Plan"|"Favorite") {
                    crate::i18n_ui::bind(&name,"label",&data.item.role);
                }
                header.append(&name);
                if let Some(status)=&data.item.status {
                    let label=gtk::Label::new(None);
                    if status.translated{crate::i18n_ui::bind(&label,"label",&status.text);}else{crate::i18n_ui::literal(&label,"label",&status.text);}
                    label.add_css_class(if status.failed{"error"}else{"dim-label"});
                    label.add_css_class("caption");header.append(&label);
                }
                let copy = gtk::Button::builder()
                    .icon_name("edit-copy-symbolic")
                    .build();
                let copy_label = if data.item.truncated {"Copy displayed excerpt"}
                    else if data.item.output.is_some() {"Copy displayed text"}
                    else {"Copy message Markdown"};
                i18n_ui::bind(&copy,"tooltip-text",copy_label);
                i18n_ui::accessible_label(&copy,copy_label);
                copy.add_css_class("flat");
                let message = data.item.clone();
                copy.connect_clicked(move |button| button.clipboard().set_text(&message.body));
                header.append(&copy);
                if let Some(source)=data.item.details.clone() {
                    let details=gtk::Button::builder().icon_name("view-list-symbolic").build();
                    crate::i18n_ui::bind(&details,"tooltip-text","Inspect activity snapshot");
                    crate::i18n_ui::accessible_label(&details,"Inspect activity snapshot");
                    details.set_widget_name(&format!("activity:{}",data.item.id));
                    details.add_css_class("flat");
                    let title=data.item.role.clone();
                    details.connect_clicked(move |_|crate::conversation_output_ui::open_details(source.clone(),&title));
                    header.append(&details);
                }
                if data.item.truncated || data.item.output.is_some() {
                    let full=gtk::Button::builder().icon_name("document-open-symbolic").build().ui_text("tooltip-text","Open complete output snapshot");
                    i18n_ui::accessible_label(&full,"Open complete output snapshot");
                    full.add_css_class("flat");
                    if let Some(source)=data.item.output.clone() {
                        full.set_widget_name(&format!("output:{}",source.token));
                        let title=data.item.role.clone();
                        full.connect_clicked(move |_|crate::conversation_output_ui::open(source.clone(),&title));
                    } else {
                        full.set_sensitive(false);
                        i18n_ui::bind(&full,"tooltip-text","Full output becomes available when the provider sends the complete item");
                    }
                    header.append(&full);
                }
                if let Some(origin) = &data.item.origin {
                    if let Some(target) = &target {
                        let button = gtk::Button::builder()
                            .icon_name("go-next-symbolic")
                            .build();
                        i18n_ui::bind_format(&button,"tooltip-text","Open session {title} ({backend})", &[("title",&origin.title),("backend",&origin.backend)]);
                        button.add_css_class("flat");
                        let message = data.item.clone();
                        let target = target.clone();
                        button.connect_clicked(move |_| target(message.clone()));
                        header.append(&button);
                    }
                    if let Some(source) = &source {
                        let button = gtk::Button::builder()
                            .icon_name("go-jump-symbolic")
                            .build();
                        i18n_ui::bind_format(&button,"tooltip-text","Open response in {title} ({backend})", &[("title",&origin.title),("backend",&origin.backend)]);
                        button.add_css_class("flat");
                        let message = data.item.clone();
                        let source = source.clone();
                        button.connect_clicked(move |_| source(message.clone()));
                        header.append(&button);
                    }
                }
                if data.item.role == "Assistant" {
                    if let Some(favorite) = &favorite {
                        let button = gtk::Button::builder()
                            .icon_name("non-starred-symbolic")
                            .build()
                            .ui_text("tooltip-text","Save displayed response to favorites");
                        button.add_css_class("flat");
                        if let Some(origin) = &data.item.origin {
                            i18n_ui::bind_format(&button,"tooltip-text","Save response from {title} ({backend}) to favorites", &[("title",&origin.title),("backend",&origin.backend)]);
                        }
                        let message = data.item.clone();
                        let favorite = favorite.clone();
                        button.connect_clicked(move |_| favorite(message.clone(), None));
                        header.append(&button);
                    }
                }
                header.upcast()
            }
            Part::Plain => {
                if let Some(markup) = &data.item.markup {
                    content_widgets::rich(markup, open.clone())
                        .upcast()
                } else {
                    content_widgets::plain(&data.item.body,matches!(data.item.role.as_str(), "Command" | "Tool" | "Files"))
                }
            }
            Part::Attachment(index) => {
                if let Some(images) = &images {
                    images(
                        &data.item,
                        ImageContent::Attachment(&data.item.images[index]),
                        open.clone(),
                    )
                } else {
                    gtk::Label::new(Some(&data.item.images[index].name)).upcast()
                }
            }
            Part::Block(index) => {
                let block = &data.item.rich.as_ref().unwrap().rows[index];
                let inner: gtk::Widget = match &block.block.kind {
                    BlockKind::Paragraph(content) => {
                        if let Some(images) = &images {
                            images(&data.item, ImageContent::Markdown(content,data.paragraph.clone()), open.clone())
                        } else {
                            content_widgets::formatted_at(content, open.clone(),data.paragraph.clone())
                        }
                    }
                    BlockKind::Heading { level, content } => {
                        content_widgets::heading_at(content, *level, open.clone(),data.paragraph.clone())
                    }
                    BlockKind::Code { language, text } => styles.code(language, text).upcast(),
                    BlockKind::Table { header, rows, .. } => table(
                        data.item.clone(),
                        index,
                        header.len(),
                        rows.len(),
                        data.table.clone(),
                        open.clone(),
                        favorite.clone(),
                        comment.clone(),
                        images.clone(),
                    )
                    .upcast(),
                    BlockKind::Diagram { kind, source } => {
                        diagrams.widget(kind, source, styles.clone()).upcast()
                    }
                    BlockKind::Rule => gtk::Separator::new(gtk::Orientation::Horizontal).upcast(),
                    _ => gtk::Label::new(None).upcast(), // Container blocks are flattened on the worker.
                };
                let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                line.set_margin_start(i32::from(block.indent) * 16);
                if block.quote {
                    line.add_css_class("card");
                    line.set_margin_top(4);
                    line.set_margin_bottom(4);
                }
                if let Some(marker) = &block.marker {
                    let label = gtk::Label::builder()
                        .label(marker)
                        .valign(gtk::Align::Start)
                        .build();
                    line.append(&label);
                }
                inner.set_hexpand(true);
                line.append(&inner);
                crate::appearance_ui::watch_markdown_block(&line);
                line.upcast()
            }
        };
        if data.item.role == "Assistant" {
            if let Some(favorite) = &favorite {
                favorite_selection(&content, &data.item, favorite);
            }
        }
        if let Some(comment) = &comment {
            comment_selection::bind(&content, &data.item, comment);
        }
        row.append(&content);
    });
    factory.connect_unbind(|_, object| {
        let item = object.downcast_ref::<gtk::ListItem>().unwrap();
        if let Some(clamp) = item.child().and_downcast::<adw::Clamp>() {
            if let Some(row) = clamp.child().and_downcast::<gtk::Box>() {
                while let Some(child) = row.first_child() {
                    row.remove(&child);
                }
            }
        }
    });
    factory
}

fn favorite_selection(widget: &gtk::Widget, message: &Arc<Item>, favorite: &FavoriteAction) {
    let action = gio::SimpleAction::new("save-selection", None);
    let installed = if let Some(label) = widget
        .downcast_ref::<gtk::Label>()
        .filter(|label| label.is_selectable())
    {
        let weak = label.downgrade();
        let message = message.clone();
        let favorite = favorite.clone();
        action.connect_activate(move |_, _| {
            if let Some(label) = weak.upgrade() {
                if let Some((a, b)) = label.selection_bounds() {
                    let text = label
                        .text()
                        .chars()
                        .skip(a.min(b) as usize)
                        .take((a - b).unsigned_abs() as usize)
                        .collect::<String>();
                    if !text.trim().is_empty() {
                        favorite(message.clone(), Some(text));
                    }
                }
            }
        });
        let menu = gio::Menu::new();
        menu.append(
            Some("Favorite selected text"),
            Some("favorite.save-selection"),
        );
        if let Some(existing) = label.extra_menu() {
            menu.prepend_section(None, &existing);
        }
        crate::i18n_ui::menu_label(&menu, menu.n_items() - 1, "Favorite selected text");
        label.set_extra_menu(Some(&menu));
        true
    } else if let Some(view) = widget
        .downcast_ref::<gtk::TextView>()
        .filter(|view| !view.is_editable())
    {
        let weak = view.downgrade();
        let message = message.clone();
        let favorite = favorite.clone();
        action.connect_activate(move |_, _| {
            if let Some(view) = weak.upgrade() {
                let buffer = view.buffer();
                if let Some((a, b)) = buffer.selection_bounds() {
                    let text = buffer.text(&a, &b, false).to_string();
                    if !text.trim().is_empty() {
                        favorite(message.clone(), Some(text));
                    }
                }
            }
        });
        let menu = gio::Menu::new();
        menu.append(
            Some("Favorite selected text"),
            Some("favorite.save-selection"),
        );
        if let Some(existing) = view.extra_menu() {
            menu.prepend_section(None, &existing);
        }
        crate::i18n_ui::menu_label(&menu, menu.n_items() - 1, "Favorite selected text");
        view.set_extra_menu(Some(&menu));
        true
    } else {
        false
    };
    if installed {
        let group = gio::SimpleActionGroup::new();
        group.add_action(&action);
        widget.insert_action_group("favorite", Some(&group));
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        favorite_selection(&widget, message, favorite);
    }
}

struct Table {
    images: Option<ImageAction>,
    favorite: Option<FavoriteAction>,
    comment: Option<CommentAction>,
    data: Arc<Item>,
    block: usize,
    state: Rc<RefCell<TableState>>,
    total: usize,
    rows: gio::ListStore,
    label: gtk::Label,
    previous: gtk::Button,
    next: gtk::Button,
    view: gtk::ColumnView,
    wrap: gtk::ToggleButton,
    menu: gtk::MenuButton,
    open: OpenLink,
    labels: RefCell<Vec<glib::WeakRef<gtk::Label>>>,
}
impl Table {
    fn focus_target(&self) {
        let target = self.state.borrow_mut().pending_row.take();
        if let Some(target) = target {
            let row = self.state.borrow().row;
            self.view.scroll_to(
                target.saturating_sub(row) as u32,
                None,
                gtk::ListScrollFlags::NONE,
                None,
            );
        }
    }
    fn cells(&self, row: usize) -> &[MarkdownCell] {
        match &self.data.rich.as_ref().unwrap().rows[self.block].block.kind {
            BlockKind::Table { rows, .. } => &rows[row].cells,
            _ => &[],
        }
    }
    fn page(&self, requested: usize) {
        let count = self.state.borrow().columns.len().max(1);
        let size = (512 / count).clamp(1, 50);
        let start = requested.min(self.total.saturating_sub(1));
        let end = (start + size).min(self.total);
        self.state.borrow_mut().row = start;
        self.rows.splice(
            0,
            self.rows.n_items(),
            &(start..end)
                .map(glib::BoxedAnyObject::new)
                .collect::<Vec<_>>(),
        );
        crate::table_columns_ui::row_status(
            &self.label,
            if self.total == 0 { 0 } else { start + 1 },
            end,
            self.total,
        );
        self.previous.set_sensitive(start > 0);
        self.next.set_sensitive(end < self.total);
    }
    fn columns(self: &Rc<Self>) {
        while let Some(column) = self
            .view
            .columns()
            .item(0)
            .and_downcast::<gtk::ColumnViewColumn>()
        {
            self.view.remove_column(&column);
        }
        let selected = self.state.borrow().columns.clone();
        let header = match &self.data.rich.as_ref().unwrap().rows[self.block].block.kind {
            BlockKind::Table { header, .. } => header,
            _ => return,
        };
        for column in selected {
            let Some(cell) = header.get(column) else {
                continue;
            };
            let factory = gtk::SignalListItemFactory::new();
            let weak = Rc::downgrade(self);
            factory.connect_bind(move |_, item| {
                let item = item.downcast_ref::<gtk::ListItem>().unwrap();
                if let (Some(table), Some(data)) = (
                    weak.upgrade(),
                    item.item().and_downcast::<glib::BoxedAnyObject>(),
                ) {
                    let index = *data.borrow::<usize>();
                    if let Some(cell) = table.cells(index).get(column) {
                        let configure = table.comment.as_ref().map(|comment| {
                            let comment = comment.clone();
                            let data = table.data.clone();
                            Rc::new(move |widget: &gtk::Widget| {
                                comment_selection::bind(widget, &data, &comment);
                            }) as crate::table_cell::ConfigureText
                        });
                        let images = table
                            .images
                            .as_ref()
                            .filter(|_| !cell.content.images.is_empty())
                            .map(|images| {
                                let images = images.clone();
                                let data = table.data.clone();
                                let open = table.open.clone();
                                Rc::new(
                                    move |container: &gtk::Box,
                                          content: &studio_markdown_native::RichText,
                                          compact: bool| {
                                        container.append(&images(
                                            &data,
                                            ImageContent::Cell(content, compact),
                                            open.clone(),
                                        ));
                                    },
                                ) as crate::table_cell::RenderImages
                            });
                        let (widget, label) =
                            crate::table_cell::widget(cell, table.open.clone(), configure, images);
                        if table.data.role == "Assistant" {
                            if let Some(favorite) = &table.favorite {
                                favorite_selection(label.upcast_ref(), &table.data, favorite);
                            }
                        }
                        label.set_wrap(table.state.borrow().wrap);
                        table
                            .labels
                            .borrow_mut()
                            .retain(|label| label.upgrade().is_some());
                        table.labels.borrow_mut().push(label.downgrade());
                        item.set_child(Some(&widget));
                    } else {
                        item.set_child(None::<&gtk::Widget>);
                    }
                }
            });
            let col = gtk::ColumnViewColumn::new(Some(cell.title()), Some(factory));
            let configure = self.comment.as_ref().map(|comment| {
                let comment = comment.clone();
                let data = self.data.clone();
                Rc::new(move |widget: &gtk::Widget| {
                    comment_selection::bind(widget, &data, &comment)
                }) as crate::table_cell::ConfigureText
            });
            let images = self
                .images
                .as_ref()
                .filter(|_| !cell.content.images.is_empty())
                .map(|images| {
                    let images = images.clone();
                    let data = self.data.clone();
                    let open = self.open.clone();
                    Rc::new(
                        move |container: &gtk::Box,
                              content: &studio_markdown_native::RichText,
                              compact: bool| {
                            container.append(&images(
                                &data,
                                ImageContent::Cell(content, compact),
                                open.clone(),
                            ));
                        },
                    ) as crate::table_cell::RenderImages
                });
            crate::table_cell::header(
                &col,
                &self.view,
                cell,
                &format!("cell-header-{column}"),
                self.open.clone(),
                configure,
                images,
            );
            col.set_resizable(true);
            col.set_expand(false);
            let width = self
                .state
                .borrow()
                .widths
                .get(&column)
                .copied()
                .unwrap_or(220);
            col.set_fixed_width(width);
            let weak = Rc::downgrade(self);
            col.connect_fixed_width_notify(move |col| {
                if let Some(table) = weak.upgrade() {
                    table
                        .state
                        .borrow_mut()
                        .widths
                        .insert(column, col.fixed_width());
                }
            });
            self.view.append_column(&col);
        }
    }
}
fn table(
    data: Arc<Item>,
    block: usize,
    columns: usize,
    total: usize,
    state: Rc<RefCell<TableState>>,
    open: OpenLink,
    favorite: Option<FavoriteAction>,
    comment: Option<CommentAction>,
    images: Option<ImageAction>,
) -> gtk::Box {
    let widget = gtk::Box::new(gtk::Orientation::Vertical, 6);
    widget.add_css_class("card");
    let toolbar = crate::adaptive::toolbar(6);
    toolbar.set_margin_start(10);
    toolbar.set_margin_end(6);
    toolbar.set_margin_top(6);
    let label = gtk::Label::builder()
        .hexpand(true)
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .width_chars(1)
        .build();
    label.add_css_class("caption");
    toolbar.append(&label);
    let previous = gtk::Button::builder()
        .icon_name("go-previous-symbolic")
        .tooltip_text("Previous table page")
        .build();
    let next = gtk::Button::builder()
        .icon_name("go-next-symbolic")
        .tooltip_text("Next table page")
        .build();
    crate::i18n_ui::bind(&previous, "tooltip-text", "Previous table page");
    crate::i18n_ui::bind(&next, "tooltip-text", "Next table page");
    toolbar.append(&previous);
    toolbar.append(&next);
    let columns_menu = gtk::MenuButton::builder().label("Columns").build();
    toolbar.append(&columns_menu);
    let wrap = gtk::ToggleButton::builder()
        .icon_name("format-justify-left-symbolic")
        .build()
        .ui_text("tooltip-text", "Wrap table cells");
    wrap.set_active(state.borrow().wrap);
    toolbar.append(&wrap);
    widget.append(&toolbar);
    let rows = gio::ListStore::new::<glib::BoxedAnyObject>();
    let view = gtk::ColumnView::new(Some(gtk::NoSelection::new(Some(rows.clone()))));
    view.set_show_row_separators(true);
    view.set_show_column_separators(true);
    widget.append(
        &gtk::ScrolledWindow::builder()
            .child(&view)
            .propagate_natural_height(true)
            .min_content_height(if total > 8 {
                300
            } else {
                ((total + 1) * 40).clamp(72, 300) as i32
            })
            .max_content_height(420)
            .build(),
    );
    if state.borrow().columns.is_empty() {
        state.borrow_mut().columns = (0..columns.min(512)).collect();
    }
    // Streamed/reconciled messages can retain a table state after its header
    // changes. Keep the rendered columns and the picker's initial state equal.
    let selection =
        crate::table_columns::Selection::new(columns, state.borrow().columns.clone(), 512);
    state.borrow_mut().columns = selection.selected().to_vec();
    let table = Rc::new(Table {
        images,
        favorite,
        comment,
        data,
        block,
        state,
        total,
        rows,
        label,
        previous,
        next,
        view,
        wrap,
        menu: columns_menu,
        open,
        labels: RefCell::new(Vec::new()),
    });
    {
        table.state.borrow_mut().live = Rc::downgrade(&table);
    }
    // The root owns this component through signals. The component owns its
    // children, never the root; factories and child callbacks capture weak refs.
    let owned = table.clone();
    widget.connect_map(move |_| {
        let row = owned.state.borrow().row;
        owned.page(row);
        owned.focus_target();
    });
    let weak = Rc::downgrade(&table);
    table.previous.connect_clicked(move |_| {
        if let Some(table) = weak.upgrade() {
            let state = table.state.borrow();
            let size = (512 / state.columns.len().max(1)).clamp(1, 50);
            let row = state.row.saturating_sub(size);
            drop(state);
            table.page(row);
        }
    });
    let weak = Rc::downgrade(&table);
    table.next.connect_clicked(move |_| {
        if let Some(table) = weak.upgrade() {
            let state = table.state.borrow();
            let size = (512 / state.columns.len().max(1)).clamp(1, 50);
            let row = state.row + size;
            drop(state);
            table.page(row);
        }
    });
    let weak = Rc::downgrade(&table);
    table.wrap.connect_toggled(move |button| {
        if let Some(table) = weak.upgrade() {
            table.state.borrow_mut().wrap = button.is_active();
            table.labels.borrow_mut().retain(|weak| {
                if let Some(label) = weak.upgrade() {
                    label.set_wrap(button.is_active());
                    true
                } else {
                    false
                }
            });
        }
    });
    if let BlockKind::Table { header, .. } =
        &table.data.rich.as_ref().unwrap().rows[block].block.kind
    {
        let weak = Rc::downgrade(&table);
        crate::table_columns_ui::install(
            &table.menu,
            header.iter().map(|cell| cell.title().to_owned()).collect(),
            table.state.borrow().columns.clone(),
            512,
            Rc::new(move |columns| {
                let Some(table) = weak.upgrade() else {
                    return false;
                };
                let row = {
                    let mut state = table.state.borrow_mut();
                    state.columns = columns.to_vec();
                    state.row
                };
                table.columns();
                table.page(row);
                true
            }),
        );
    }
    table.columns();
    let start = table.state.borrow().row;
    table.page(start);
    widget
}

#[cfg(debug_assertions)]
pub async fn smoke_test(
    view: &gtk::ListView,
    store: &gio::ListStore,
    scroll: &gtk::ScrolledWindow,
    capture: impl Fn(&str),
) -> Result<serde_json::Value, String> {
    use sourceview::prelude::*;
    async fn until(test: impl Fn() -> bool) -> Result<(), String> {
        let started = std::time::Instant::now();
        while !test() {
            if started.elapsed() > std::time::Duration::from_secs(5) {
                return Err("Native chat widgets did not become ready".into());
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        Ok(())
    }
    fn find_code(widget: &gtk::Widget) -> Option<sourceview::Buffer> {
        if let Some(view) = widget.downcast_ref::<sourceview::View>() {
            if let Ok(buffer) = view.buffer().downcast::<sourceview::Buffer>() {
                if buffer
                    .text(&buffer.start_iter(), &buffer.end_iter(), false)
                    .contains("fn main()")
                {
                    return Some(buffer);
                }
            }
        }
        let mut child = widget.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            if let Some(buffer) = find_code(&widget) {
                return Some(buffer);
            }
        }
        None
    }
    fn checks(widget: &gtk::Widget, output: &mut Vec<gtk::CheckButton>) {
        if let Some(check) = widget.downcast_ref::<gtk::CheckButton>() {
            output.push(check.clone());
        }
        let mut child = widget.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            checks(&widget, output);
        }
    }
    let mut table_row = None;
    let mut code_row = None;
    for index in 0..store.n_items() {
        let object = store
            .item(index)
            .and_downcast::<glib::BoxedAnyObject>()
            .unwrap();
        let row = object.borrow::<Row>();
        if let Part::Block(block) = row.part {
            match row.item.rich.as_ref().unwrap().rows[block].block.kind {
                BlockKind::Table { .. } => table_row = Some((index, row.table.clone())),
                BlockKind::Code { .. } => code_row = Some(index),
                _ => {}
            }
        }
    }
    let (index, state) =
        table_row.ok_or("The conversation table was not parsed into a native block")?;
    view.scroll_to(index, gtk::ListScrollFlags::NONE, None);
    crate::window_capture::next_frame(view).await?;
    capture("chat-table.png");
    until(|| state.borrow().live.upgrade().is_some()).await?;
    let table = state.borrow().live.upgrade().unwrap();
    if table.rows.n_items() != 50 || table.total != 80 || table.view.columns().n_items() != 3 {
        return Err("The chat table has the wrong page or columns".into());
    }
    let column = table
        .view
        .columns()
        .item(0)
        .and_downcast::<gtk::ColumnViewColumn>()
        .unwrap();
    column.set_fixed_width(280);
    let before = scroll.vadjustment().value();
    table.next.grab_focus();
    table.next.emit_clicked();
    if table.state.borrow().row != 50 || table.rows.n_items() != 30 {
        return Err("Next chat table page did not preserve source rows".into());
    }
    if column.fixed_width() != 280 {
        return Err("Table paging reset the resized column".into());
    }
    table.previous.emit_clicked();
    if table.state.borrow().row != 0 {
        return Err("Previous chat table page failed".into());
    }
    if (scroll.vadjustment().value() - before).abs() > 2.0 {
        return Err("Chat table paging moved the conversation scroll position".into());
    }
    table.wrap.set_active(false);
    if table.state.borrow().wrap {
        return Err("Table wrap did not change".into());
    }
    table.wrap.set_active(true);
    let popover = table.menu.popover().unwrap();
    popover.popup();
    until(|| {
        let mut options = Vec::new();
        checks(popover.child().unwrap().upcast_ref(), &mut options);
        options.len() == 3 && options.iter().all(|check| check.is_mapped())
    })
    .await?;
    let mut options = Vec::new();
    checks(
        table.menu.popover().unwrap().child().unwrap().upcast_ref(),
        &mut options,
    );
    options[1].set_active(false);
    if table.view.columns().n_items() != 2 {
        return Err("Table column selection did not apply".into());
    }
    options[2].set_active(false);
    options[0].set_active(false);
    if table.state.borrow().columns != vec![0] || !options[0].is_active() {
        return Err("Column selection allowed an empty or duplicate selection".into());
    }
    options[1].set_active(true);
    options[2].set_active(true);
    popover.popdown();
    glib::timeout_future(std::time::Duration::from_millis(60)).await;
    crate::window_capture::next_frame(view).await?;
    capture("chat-table.png");
    view.scroll_to(
        code_row.ok_or("Native code block was not parsed")?,
        gtk::ListScrollFlags::NONE,
        None,
    );
    crate::window_capture::next_frame(view).await?;
    capture("chat-code.png");
    let buffer = find_code(view.upcast_ref())
        .ok_or("GtkSourceView did not render the conversation code block")?;
    if buffer
        .language()
        .is_none_or(|language| language.id() != "rust")
    {
        return Err("Rust syntax highlighting was not configured".into());
    }
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceLight);
    if buffer
        .style_scheme()
        .is_none_or(|scheme| scheme.id() != "Adwaita")
    {
        return Err("Chat code theme did not follow light appearance".into());
    }
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
    if buffer
        .style_scheme()
        .is_none_or(|scheme| scheme.id() != "Adwaita-dark")
    {
        return Err("Chat code theme did not follow dark appearance".into());
    }
    crate::window_capture::next_frame(view).await?;
    capture("chat-code.png");
    view.scroll_to(0, gtk::ListScrollFlags::NONE, None);
    crate::window_capture::next_frame(view).await?;
    capture("chat-top.png");
    Ok(
        serde_json::json!({"ok":true,"nativeTable":true,"stablePaging":true,"columns":true,"wrap":true,"syntaxHighlighting":true,"codeTheme":true,"flatBlocks":store.n_items()}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn message(id: &str, source: &str, rich: bool) -> Arc<Item> {
        let mut item =
            Item::from_value(&serde_json::json!({"id":id,"type":"agentMessage","text":source}));
        if rich {
            item.rich = Some(crate::chat_content::Content::parse(&item.body).unwrap());
        }
        Arc::new(item)
    }
    #[test]
    fn flat_rows_keep_unchanged_table_state_and_replace_completed_streams() {
        let store = gio::ListStore::new::<glib::BoxedAnyObject>();
        let source = "# Heading\n\n| A | B |\n|---|---|\n| 1 | 2 |\n";
        let item = message("first", source, true);
        reconcile(&store, &[item.clone()]);
        assert_eq!(store.n_items(), 3);
        let old = store
            .item(2)
            .and_downcast::<glib::BoxedAnyObject>()
            .unwrap();
        old.borrow::<Row>().table.borrow_mut().row = 50;
        old.borrow::<Row>().table.borrow_mut().columns = vec![1];
        old.borrow::<Row>().table.borrow_mut().wrap = false;
        old.borrow::<Row>().table.borrow_mut().widths.insert(1, 345);
        reconcile(&store, &[item.clone()]);
        assert_eq!(
            store
                .item(2)
                .and_downcast::<glib::BoxedAnyObject>()
                .unwrap(),
            old
        );
        let refreshed = message("first", source, true);
        reconcile(
            &store,
            &[refreshed.clone(), message("second", "waiting", false)],
        );
        let kept = store
            .item(2)
            .and_downcast::<glib::BoxedAnyObject>()
            .unwrap();
        assert_ne!(kept, old);
        assert!(Arc::ptr_eq(&kept.borrow::<Row>().item, &refreshed));
        assert_eq!(kept.borrow::<Row>().table.borrow().row, 50);
        assert_eq!(kept.borrow::<Row>().table.borrow().columns, vec![1]);
        assert!(!kept.borrow::<Row>().table.borrow().wrap);
        assert_eq!(
            kept.borrow::<Row>().table.borrow().widths.get(&1),
            Some(&345)
        );
        assert_eq!(store.n_items(), 5);
        reconcile(&store, &[item, message("second", "waiting", true)]);
        let completed = store
            .item(4)
            .and_downcast::<glib::BoxedAnyObject>()
            .unwrap();
        assert!(matches!(completed.borrow::<Row>().part, Part::Block(0)));
        reconcile(&store, &[]);
        assert_eq!(store.n_items(), 0);
    }
}
