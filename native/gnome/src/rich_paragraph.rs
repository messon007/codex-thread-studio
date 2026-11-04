//! Source-preserving native object layout for bounded, paged math paragraphs.
use adw::prelude::*;
use gtk::{gio, glib, subclass::prelude::*};
use std::{
    cell::{Cell, OnceCell, RefCell},
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock,
    },
};
type Ready = Rc<dyn Fn(&gtk::Widget)>;
mod imp {
    use super::*;
    #[derive(Default)]
    pub struct Paragraph {
        pub content: OnceCell<Arc<studio_markdown_native::rich_page::Source>>,
        pub open: RefCell<Option<crate::content_widgets::OpenLink>>,
        pub reading: OnceCell<Rc<crate::content_widgets::ParagraphState>>,
        pub render: RefCell<Option<Rc<dyn Fn(usize)>>>,
        pub child: RefCell<Option<gtk::Widget>>,
        pub view: RefCell<Option<crate::text_flow::View>>,
        pub callbacks: RefCell<Vec<Ready>>,
        pub epoch: Cell<u64>,
        pub cancel: RefCell<Option<Arc<AtomicBool>>>,
        pub theme: RefCell<Option<glib::SignalHandlerId>>,
        pub status: OnceCell<gtk::Label>,
        pub controls: RefCell<Vec<gtk::Button>>,
        pub displayed: Cell<usize>,
        pub focus: Cell<bool>,
    }
    #[glib::object_subclass]
    impl ObjectSubclass for Paragraph {
        const NAME: &'static str = "StudioRichParagraph";
        type Type = super::Paragraph;
        type ParentType = gtk::Box;
    }
    impl ObjectImpl for Paragraph {
        fn dispose(&self) {
            self.cancel();
            if let Some(id) = self.theme.borrow_mut().take() {
                adw::StyleManager::default().disconnect(id);
            }
            self.view.borrow_mut().take();
        }
    }
    impl WidgetImpl for Paragraph {}
    impl BoxImpl for Paragraph {}
    impl Paragraph {
        pub fn cancel(&self) {
            self.epoch.set(self.epoch.get().wrapping_add(1));
            if let Some(cancel) = self.cancel.borrow_mut().take() {
                cancel.store(true, Ordering::Relaxed);
            }
        }
    }
}
glib::wrapper! {pub struct Paragraph(ObjectSubclass<imp::Paragraph>) @extends gtk::Widget,gtk::Box, @implements gtk::Accessible,gtk::Buildable,gtk::ConstraintTarget,gtk::Orientable;}
impl Paragraph {
    pub fn new(
        content: &studio_markdown_native::RichText,
        open: crate::content_widgets::OpenLink,
        reading: Rc<crate::content_widgets::ParagraphState>,
    ) -> Self {
        let result: Self = glib::Object::builder()
            .property("orientation", gtk::Orientation::Vertical)
            .build();
        result
            .imp()
            .content
            .set(Arc::new(
                studio_markdown_native::rich_page::Source::new(content).unwrap(),
            ))
            .ok()
            .unwrap();
        result.imp().open.replace(Some(open));
        result
            .imp()
            .focus
            .set(reading.view.upgrade().is_some_and(|v| v.has_focus()));
        result.imp().reading.set(reading).ok().unwrap();
        let weak = result.downgrade();
        result.imp().render.replace(Some(Rc::new(move |page| {
            if let Some(root) = weak.upgrade() {
                root.navigate(page);
            }
        })));
        result.bind_renderer();
        result.toolbar();
        result.fallback(None);
        result.connect_map(|root| {
            root.bind_renderer();
            if root.imp().child.borrow().is_none() {
                root.fallback(None);
            }
            root.prepare();
        });
        result.connect_unmap(|root| {
            root.imp().cancel();
            let owned = root.owns_reading();
            if owned {
                root.imp().reading.get().unwrap().scroll.freeze();
            }
            root.imp().view.borrow_mut().take();
            if owned {
                root.fallback(None);
            } else if let Some(child) = root.imp().child.borrow_mut().take() {
                root.remove(&child);
            }
        });
        let weak = result.downgrade();
        *result.imp().theme.borrow_mut() =
            Some(adw::StyleManager::default().connect_dark_notify(move |_| {
                if let Some(root) = weak.upgrade().filter(|r| r.is_mapped() && r.owns_reading()) {
                    root.prepare();
                }
            }));
        result
    }
    fn toolbar(&self) {
        let count = self.imp().content.get().unwrap().attributes.pages.len();
        if count <= 1 {
            return;
        }
        let header = crate::adaptive::toolbar(4);
        let status = gtk::Label::builder()
            .hexpand(true)
            .xalign(0.0)
            .wrap(true)
            .width_chars(1)
            .build();
        status.add_css_class("caption");
        header.append(&status);
        self.imp().status.set(status).unwrap();
        let actions = gio::SimpleActionGroup::new();
        for (i, (name, icon, tip)) in [
            ("first-page", "go-first-symbolic", "First text page"),
            (
                "previous-page",
                "go-previous-symbolic",
                "Previous text page",
            ),
            ("next-page", "go-next-symbolic", "Next text page"),
            ("last-page", "go-last-symbolic", "Last text page"),
        ]
        .into_iter()
        .enumerate()
        {
            let action = gio::SimpleAction::new(name, None);
            let weak = self.downgrade();
            action.connect_activate(move |_, _| {
                let Some(root) = weak.upgrade().filter(|r| r.is_mapped() && r.owns_reading())
                else {
                    return;
                };
                let reading = root.imp().reading.get().unwrap();
                reading.select(match i {
                    0 => 0,
                    1 => reading.page.get().saturating_sub(1),
                    2 => (reading.page.get() + 1).min(count - 1),
                    _ => count - 1,
                });
            });
            actions.add_action(&action);
            let button = gtk::Button::builder()
                .icon_name(icon)
                .tooltip_text(tip)
                .action_name(format!("rich-page.{name}"))
                .build();
            button.add_css_class("flat");
            crate::i18n_ui::bind(&button, "tooltip-text", tip);
            crate::i18n_ui::accessible_label(&button, tip);
            header.append(&button);
            self.imp().controls.borrow_mut().push(button);
        }
        self.insert_action_group("rich-page", Some(&actions));
        self.append(&header);
        self.update_toolbar();
    }
    fn update_toolbar(&self) {
        let Some(status) = self.imp().status.get() else {
            return;
        };
        let reading = self.imp().reading.get().unwrap();
        let count = self.imp().content.get().unwrap().attributes.pages.len();
        let index = reading.page.get().min(count - 1);
        let mut parts = vec![crate::i18n_ui::Part::message(
            "Page {page}/{pages}",
            &[
                ("page", &(index + 1).to_string()),
                ("pages", &count.to_string()),
            ],
        )];
        let notice = if reading.source_only.get() {
            Some("Match is in Markdown source")
        } else if reading.continued.get() {
            Some("Match continues on next page")
        } else {
            None
        };
        if let Some(notice) = notice {
            parts.push(crate::i18n_ui::Part::literal(" · "));
            parts.push(crate::i18n_ui::Part::message(notice, &[]));
        }
        crate::i18n_ui::bind_parts(status, "label", parts, "");
        for (i, button) in self.imp().controls.borrow().iter().enumerate() {
            button.set_sensitive(if i < 2 { index > 0 } else { index + 1 < count });
        }
    }
    fn navigate(&self, index: usize) {
        if !self.owns_reading() {
            return;
        }
        let reading = self.imp().reading.get().unwrap();
        let count = self.imp().content.get().unwrap().attributes.pages.len();
        let index = index.min(count - 1);
        reading.page.set(index);
        if self.imp().displayed.get() == index && self.imp().view.borrow().is_some() {
            if reading.target.borrow().is_some() {
                self.apply_target(false);
            } else if let Some(view) = reading.view.upgrade() {
                view.buffer().place_cursor(&view.buffer().start_iter());
                view.scroll_to_mark(&view.buffer().get_insert(), 0.0, true, 0.0, 0.0);
            }
            self.update_toolbar();
            return;
        }
        self.imp().cancel();
        self.fallback(None);
        if self.is_mapped() {
            self.prepare();
        }
    }
    fn replace(&self, child: gtk::Widget) {
        let old = self.imp().child.borrow_mut().take();
        if let Some(old) = old {
            self.remove(&old);
        }
        self.append(&child);
        self.imp().child.replace(Some(child.clone()));
        let callbacks = self.imp().callbacks.borrow().clone();
        for callback in callbacks {
            callback(&child);
        }
    }
    fn fallback(&self, error: Option<&str>) {
        if !self.owns_reading() {
            return;
        }
        self.imp().view.borrow_mut().take();
        let content = self.imp().content.get().unwrap();
        // Force the existing exact-text formatter, rather than recursively
        // selecting this object layout or normalizing CRLF through markup.
        let raw = studio_markdown_native::RichText {
            source_only: false,
            plain: String::new(),
            markup: String::new(),
            images: Vec::new(),
            math: Vec::new(),
            attributes: Some(content.attributes.clone()),
        };
        let widget = crate::content_widgets::formatted_at(
            &raw,
            self.imp().open.borrow().as_ref().unwrap().clone(),
            self.imp().reading.get().unwrap().clone(),
        );
        if let Some(error) = error {
            widget.set_tooltip_text(Some(error));
        }
        fn compact(widget: &gtk::Widget, short: bool, wide: bool) {
            if let Some(scroll) = widget.downcast_ref::<gtk::ScrolledWindow>() {
                scroll.set_min_content_height(if short { 80 } else { 320 });
                if wide {
                    scroll.set_hscrollbar_policy(gtk::PolicyType::Automatic);
                    scroll.set_overlay_scrolling(false);
                }
            }
            if wide {
                if let Some(view) = widget.downcast_ref::<gtk::TextView>() {
                    view.set_wrap_mode(gtk::WrapMode::None);
                }
            }
            let mut child = widget.first_child();
            while let Some(widget) = child {
                child = widget.next_sibling();
                compact(&widget, short, wide);
            }
        }
        if let Some(header) = widget.first_child() {
            header.set_visible(false);
        }
        // The exact-text child's toolbar is hidden. Its scoped link actions
        // remain usable; the persistent parent owns the visible page controls.
        let part = &content.attributes.pages[self.imp().reading.get().unwrap().page.get()];
        let visible = if error.is_some() {
            part.characters
        } else {
            content
                .math
                .iter()
                .filter(|m| {
                    m.characters.start >= part.character
                        && m.characters.end <= part.character + part.characters
                })
                .fold(part.characters, |n, m| {
                    n.saturating_sub(m.characters.len().saturating_sub(1))
                })
        };
        compact(
            &widget,
            content.attributes.pages.len() <= 1 && visible <= 96,
            part.end - part.start > studio_markdown_native::inline::PAGE_BYTES,
        );
        self.replace(widget);
        self.bind_renderer();
        self.update_toolbar();
    }
    fn bind_renderer(&self) {
        self.imp()
            .reading
            .get()
            .unwrap()
            .renderer
            .replace(self.imp().render.borrow().as_ref().map(Rc::downgrade));
    }
    fn owns_reading(&self) -> bool {
        let render = self.imp().render.borrow();
        let Some(render) = render.as_ref() else {
            return false;
        };
        self.imp()
            .reading
            .get()
            .unwrap()
            .renderer
            .borrow()
            .as_ref()
            .and_then(std::rc::Weak::upgrade)
            .is_some_and(|current| Rc::ptr_eq(&current, render))
    }
    fn apply_target(&self, silent: bool) {
        if !self.owns_reading() {
            return;
        }
        let reading = self.imp().reading.get().unwrap();
        let Some(view) = reading.view.upgrade() else {
            return;
        };
        let target = reading.target.borrow().clone();
        if let Some(mut range) = target {
            let part = &self.imp().content.get().unwrap().attributes.pages[reading.page.get()];
            reading
                .continued
                .set(range.end > part.character + part.characters);
            range = range
                .start
                .saturating_sub(part.character)
                .min(part.characters)
                ..range
                    .end
                    .saturating_sub(part.character)
                    .min(part.characters);
            if let Some(projection) = self.imp().view.borrow().as_ref() {
                let Some(mapped) = projection
                    .document
                    .quick
                    .as_ref()
                    .and_then(|m| m.projected_range(range))
                else {
                    return;
                };
                range = mapped;
            }
            let buffer = view.buffer();
            let a = buffer.iter_at_offset(range.start as i32);
            let b = buffer.iter_at_offset(range.end as i32);
            if silent {
                crate::text_flow::restore_without_primary(&view, &a, &b);
            } else {
                buffer.select_range(&a, &b);
            }
            view.scroll_to_mark(&buffer.get_insert(), 0.1, true, 0.0, 0.35);
        }
        self.update_toolbar();
    }
    fn prepare(&self) {
        if !self.owns_reading() {
            return;
        }
        self.imp().cancel();
        let epoch = self.imp().epoch.get();
        let cancel = Arc::new(AtomicBool::new(false));
        self.imp().cancel.replace(Some(cancel.clone()));
        let content = self.imp().content.get().unwrap().clone();
        let index = self
            .imp()
            .reading
            .get()
            .unwrap()
            .page
            .get()
            .min(content.attributes.pages.len() - 1);
        let dark = adw::StyleManager::default().is_dark();
        let weak = self.downgrade();
        glib::spawn_future_local(async move {
            static QUEUED: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
            static CPU: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
            let Ok(_queued) = QUEUED
                .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(128)))
                .clone()
                .try_acquire_owned()
            else {
                if let Some(root) = weak
                    .upgrade()
                    .filter(|r| r.is_mapped() && r.owns_reading() && r.imp().epoch.get() == epoch)
                {
                    root.fallback(Some("Too many math paragraphs are visible"));
                }
                return;
            };
            let Ok(permit) = CPU
                .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(2)))
                .clone()
                .acquire_owned()
                .await
            else {
                return;
            };
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            let prepared = gio::spawn_blocking(move || {
                let _permit = permit;
                let content = content.page(index)?;
                // Existing document/chat image renderers keep their independent
                // previews. Alt text stays literal in this paragraph projection.
                let document = studio_markdown_native::flow::project_rich(content)?;
                crate::text_flow::prepare_projection(
                    Arc::new(document),
                    "rich-paragraph".into(),
                    cancel,
                    dark,
                )
            })
            .await
            .map_err(|_| "Rich paragraph worker stopped".to_owned())
            .and_then(|r| r);
            let Some(root) = weak
                .upgrade()
                .filter(|r| r.is_mapped() && r.owns_reading() && r.imp().epoch.get() == epoch)
            else {
                return;
            };
            let reading = root.imp().reading.get().unwrap().clone();
            let position = reading.scroll.freeze();
            let inherited_focus = root.imp().focus.replace(false);
            let focused = reading.view.upgrade().is_some_and(|v| v.has_focus()) || inherited_focus;
            root.fallback(None);
            let result = prepared.and_then(|p| {
                crate::text_flow::view(p, root.imp().open.borrow().as_ref().unwrap().clone())
            });
            match result {
                Ok(view) => {
                    view.widget.set_widget_name("studio-multiline-rich-text");
                    view.widget.set_left_margin(8);
                    view.widget.set_right_margin(8);
                    view.widget.set_top_margin(8);
                    view.widget.set_bottom_margin(8);
                    let scroll = gtk::ScrolledWindow::builder()
                        .child(&view.widget)
                        .hexpand(true)
                        .hscrollbar_policy(gtk::PolicyType::Automatic)
                        .overlay_scrolling(false)
                        .min_content_height(
                            if root.imp().content.get().unwrap().attributes.pages.len() > 1
                                || view.document.characters > 96
                            {
                                320
                            } else {
                                80
                            },
                        )
                        .max_content_height(320)
                        .propagate_natural_height(true)
                        .build();
                    reading.scroll.attach_mapped(
                        &view.widget,
                        &scroll,
                        view.document.quick.clone(),
                    );
                    reading.view.set(Some(&view.widget));
                    root.imp().displayed.set(index);
                    let widget = view.widget.clone();
                    root.imp().view.replace(Some(view));
                    root.replace(scroll.upcast());
                    root.bind_renderer();
                    if focused {
                        widget.grab_focus();
                    }
                    let position = if reading.target.borrow().is_some() {
                        root.apply_target(
                            position.is_some_and(|p| p.primary_owner() == Some(false)),
                        );
                        None
                    } else {
                        position
                    };
                    reading.scroll.freeze();
                    reading.scroll.settle(&widget, position);
                    root.update_toolbar();
                }
                Err(error) => {
                    root.fallback(Some(&error));
                    if focused {
                        if let Some(view) = reading.view.upgrade() {
                            view.grab_focus();
                        }
                    }
                }
            }
        });
    }
}
/// Dynamic leaves keep the same source-bound callbacks when worker output or a
/// theme change replaces their view. All callers retain weak document owners.
pub fn when_ready(widget: &gtk::Widget, callback: Ready) -> bool {
    let Some(root) = widget.downcast_ref::<Paragraph>() else {
        return false;
    };
    root.imp().callbacks.borrow_mut().push(callback.clone());
    let child = root.imp().child.borrow().clone();
    if let Some(child) = child {
        callback(&child);
    }
    true
}
