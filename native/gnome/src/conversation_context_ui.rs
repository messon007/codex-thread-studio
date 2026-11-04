//! Read-only historical context, separate from the live conversation model.
use crate::{
    chat_diagrams::Renderer,
    content_widgets::{OpenLink, Styles},
    conversation_focus::Window,
    session_search::Context,
};
use adw::prelude::*;
use gtk::{gio, glib};
use std::rc::Rc;
pub const DIAGRAM_KEY: u64 = 5;
pub struct View {
    pub dialog: adw::Dialog,
    pub context: Context,
    pub serial: u64,
    pub diagrams: Rc<Renderer>,
    model: gio::ListStore,
    list: gtk::ListView,
    stack: gtk::Stack,
    status: gtk::Label,
    notice: gtk::Label,
}
impl View {
    pub fn new(
        parent: &adw::ApplicationWindow,
        context: Context,
        serial: u64,
        title: &str,
        sender: tokio::sync::mpsc::Sender<crate::document::Message>,
        open: OpenLink,
        images: Option<crate::chat_ui::ImageAction>,
    ) -> Rc<Self> {
        let dialog = adw::Dialog::builder()
            .title("Conversation context")
            .content_width(1100)
            .content_height(720)
            .build();
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&adw::WindowTitle::new("Conversation context", title)));
        toolbar.add_top_bar(&header);
        let stack = gtk::Stack::new();
        let status = gtk::Label::builder()
            .label("Loading older conversation context…")
            .wrap(true)
            .margin_start(24)
            .margin_end(24)
            .build();
        stack.add_named(&status, Some("status"));
        let diagrams = Renderer::new_with_key(sender, DIAGRAM_KEY);
        let model = gio::ListStore::new::<glib::BoxedAnyObject>();
        let list = gtk::ListView::new(
            Some(gtk::NoSelection::new(Some(model.clone()))),
            Some(crate::chat_ui::factory(
                open,
                Styles::new(),
                diagrams.clone(),
                None,
                images,
            )),
        );
        stack.add_named(
            &gtk::ScrolledWindow::builder()
                .child(&list)
                .vexpand(true)
                .build(),
            Some("context"),
        );
        let notice = gtk::Label::builder()
            .label("Read-only history window")
            .xalign(0.0)
            .wrap(true)
            .margin_start(12)
            .margin_end(12)
            .margin_top(6)
            .margin_bottom(6)
            .build();
        notice.add_css_class("caption");
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&notice);
        content.append(&stack);
        toolbar.set_content(Some(&content));
        dialog.set_child(Some(&toolbar));
        let view = Rc::new(Self {
            dialog,
            context,
            serial,
            diagrams,
            model,
            list,
            stack,
            status,
            notice,
        });
        view.dialog.present(Some(parent));
        view
    }
    pub fn apply(&self, result: Result<Window, String>) {
        match result {
            Ok(window) => {
                self.notice.set_text(&format!(
                    "Read-only · {} messages of surrounding context{}",
                    window.items.len(),
                    if window.excerpt {
                        " · large output shown around the match"
                    } else {
                        ""
                    }
                ));
                crate::chat_ui::reconcile(&self.model, &window.items);
                self.stack.set_visible_child_name("context");
                let target = (0..self.model.n_items()).find(|index| {
                    self.model
                        .item(*index)
                        .and_downcast::<glib::BoxedAnyObject>()
                        .is_some_and(|object| {
                            let row = object.borrow::<crate::chat_ui::Row>();
                            row.item.id == window.target
                        })
                });
                if let Some(target) = target {
                    self.list
                        .scroll_to(target, gtk::ListScrollFlags::NONE, None);
                }
            }
            Err(error) => {
                self.status.set_text(&error);
                self.stack.set_visible_child_name("status");
            }
        }
    }
    #[cfg(debug_assertions)]
    pub fn contains(&self, id: &str) -> bool {
        (0..self.model.n_items()).any(|index| {
            self.model
                .item(index)
                .and_downcast::<glib::BoxedAnyObject>()
                .is_some_and(|object| object.borrow::<crate::chat_ui::Row>().item.id == id)
        })
    }
}
impl Drop for View {
    fn drop(&mut self) {
        self.diagrams.reset();
    }
}
