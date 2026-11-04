//! Native supervision menu. GTK displays worker state; it never evaluates tasks.
use crate::{i18n_ui::UiText, supervision_state::Source};
use adw::prelude::*;
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub source: Source,
    pub status: String,
    pub action: &'static str,
}
pub struct Menu {
    pub button: gtk::MenuButton,
    body: gtk::Box,
    rows: RefCell<(String, Vec<Row>)>,
    controls: RefCell<BTreeMap<String, Controls>>,
    pub on_action: RefCell<Option<Box<dyn Fn(Source, bool)>>>,
}
struct Controls {
    group: gtk::Box,
    title: gtk::Label,
    status: gtk::Label,
    button: gtk::Button,
    data: Rc<RefCell<Row>>,
}
impl Menu {
    pub fn new() -> Rc<Self> {
        let button = gtk::MenuButton::builder()
            .icon_name("system-run-symbolic")
            .build()
            .ui_text("tooltip-text", "Turn supervision");
        crate::i18n_ui::accessible_label(&button, "Turn supervision");
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.set_margin_start(12);
        body.set_margin_end(12);
        body.set_margin_top(12);
        body.set_margin_bottom(12);
        let scroll = gtk::ScrolledWindow::builder()
            .child(&body)
            .min_content_width(360)
            .max_content_height(460)
            .propagate_natural_height(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        let popup = gtk::Popover::new();
        popup.set_child(Some(&scroll));
        button.set_popover(Some(&popup));
        Rc::new(Self {
            button,
            body,
            rows: RefCell::default(),
            controls: RefCell::default(),
            on_action: RefCell::new(None),
        })
    }
    pub fn render(self: &Rc<Self>, rows: Vec<Row>) {
        let language = crate::language::tr("Supervise this turn");
        if *self.rows.borrow() == (language.clone(), rows.clone()) {
            return;
        }
        self.rows.replace((language, rows.clone()));
        let keys = rows.iter().map(|r| r.source.key()).collect::<BTreeSet<_>>();
        self.controls.borrow_mut().retain(|key, c| {
            if keys.contains(key) {
                true
            } else {
                self.body.remove(&c.group);
                false
            }
        });
        if self.controls.borrow().is_empty() {
            while let Some(child) = self.body.first_child() {
                self.body.remove(&child);
            }
        }
        if rows.is_empty() {
            self.body.append(
                &gtk::Label::new(Some("No running turn to supervise"))
                    .ui_text("label", "No running turn to supervise"),
            );
        }
        for row in rows {
            let key = row.source.key();
            if let Some(c) = self.controls.borrow().get(&key) {
                c.title.set_text(&row.source.title);
                c.title.set_tooltip_text(Some(&format!(
                    "{}\n{}",
                    row.source.key(),
                    row.source.directory
                )));
                c.status.set_text(&row.status);
                crate::i18n_ui::bind(
                    &c.button,
                    "label",
                    if row.action == "start" {
                        "Supervise this turn"
                    } else {
                        "Stop supervision"
                    },
                );
                c.button
                    .set_sensitive(matches!(row.action, "start" | "stop"));
                c.data.replace(row);
                continue;
            }
            let group = gtk::Box::new(gtk::Orientation::Vertical, 6);
            let title = gtk::Label::builder()
                .label(&row.source.title)
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .max_width_chars(45)
                .build();
            title.add_css_class("heading");
            title.set_tooltip_text(Some(&format!(
                "{}\n{}",
                row.source.key(),
                row.source.directory
            )));
            group.append(&title);
            let status = gtk::Label::builder()
                .label(&row.status)
                .xalign(0.0)
                .wrap(true)
                .max_width_chars(45)
                .build();
            status.add_css_class("caption");
            status.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            group.append(&status);
            let label = if row.action == "start" {
                "Supervise this turn"
            } else {
                "Stop supervision"
            };
            let button = gtk::Button::with_label(label).ui_text("label", label);
            button.set_halign(gtk::Align::Start);
            button.set_sensitive(matches!(row.action, "start" | "stop"));
            let weak = Rc::downgrade(self);
            let data = Rc::new(RefCell::new(row));
            let selected = data.clone();
            button.connect_clicked(move |_| {
                let row = selected.borrow().clone();
                if let Some(menu) = weak.upgrade() {
                    if let Some(action) = menu.on_action.borrow().as_ref() {
                        action(row.source.clone(), row.action == "stop");
                    }
                }
            });
            group.append(&button);
            self.body.append(&group);
            self.controls.borrow_mut().insert(
                key,
                Controls {
                    group,
                    title,
                    status,
                    button,
                    data,
                },
            );
        }
    }
}
