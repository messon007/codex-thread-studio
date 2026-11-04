//! Full-cell math uses bounded worker-prepared label pages. Selection keeps the
//! original TeX bytes; labels share the normal native formula lane/cache.
use super::*;
use std::{
    cell::Cell as Counter,
    sync::{Arc, OnceLock},
};

pub fn widget(
    content: Arc<studio_markdown_native::RichText>,
    open: OpenLink,
    configure: Option<ConfigureText>,
    reading: Rc<ParagraphState>,
) -> gtk::Widget {
    let result = gtk::Box::new(gtk::Orientation::Vertical, 8);
    result.set_widget_name("studio-cell-math-pages");
    result.set_vexpand(true);
    let loading = gtk::Spinner::builder()
        .spinning(true)
        .halign(gtk::Align::Center)
        .build();
    crate::i18n_ui::accessible_label(&loading, "Loading page…");
    result.append(&loading);
    let weak = result.downgrade();
    glib::spawn_future_local(async move {
        if weak.upgrade().is_none_or(|w| !w.is_mapped()) {
            return;
        }
        static LIMIT: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
        let permit = LIMIT
            .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(2)))
            .clone()
            .try_acquire_owned();
        let source = content.clone();
        let pages = match permit {
            Ok(permit) => gio::spawn_blocking(move || {
                let _permit = permit;
                studio_markdown_native::rich_pages::prepare(&source)
            })
            .await
            .map_err(|_| "Formatted cell worker stopped".to_string())
            .and_then(|r| r),
            Err(_) => Err("Formatted cell workers are busy".into()),
        };
        let Some(result) = weak.upgrade().filter(|w| w.is_mapped()) else {
            return;
        };
        while let Some(child) = result.first_child() {
            result.remove(&child);
        }
        let pages = match pages {
            Ok(pages) if !pages.pages.is_empty() => Arc::new(pages),
            pages => {
                // Keep the original bounded TextView readable when formatting
                // cannot be prepared. Never hide source text behind an error.
                let formatted = content_widgets::formatted_at(&content, open, reading.clone());
                if let Some(configure) = configure {
                    configure(&formatted);
                }
                if let Err(error) = pages {
                    formatted.set_tooltip_text(Some(&error));
                }
                result.append(&formatted);
                return;
            }
        };
        let toolbar = crate::adaptive::toolbar(4);
        let status = gtk::Label::builder().hexpand(true).xalign(0.0).build();
        status.add_css_class("caption");
        toolbar.append(&status);
        let scroll = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .min_content_height(320)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        result.append(&toolbar);
        result.append(&scroll);
        let buttons = [
            ("go-first-symbolic", "First text page"),
            ("go-previous-symbolic", "Previous text page"),
            ("go-next-symbolic", "Next text page"),
            ("go-last-symbolic", "Last text page"),
        ]
        .map(|(icon, tip)| {
            let button = gtk::Button::builder()
                .icon_name(icon)
                .tooltip_text(tip)
                .build();
            crate::i18n_ui::bind(&button, "tooltip-text", tip);
            crate::i18n_ui::accessible_label(&button, tip);
            button.add_css_class("flat");
            toolbar.append(&button);
            button
        });
        let page = Rc::new(Counter::new(0usize));
        let weak = scroll.downgrade();
        let status = status.downgrade();
        let controls = buttons.each_ref().map(|b| b.downgrade());
        let count = pages.pages.len();
        let current = page.clone();
        let focus = reading.clone();
        let render: Rc<dyn Fn(usize)> = Rc::new(move |index| {
            let Some(scroll) = weak.upgrade().filter(|s| s.is_mapped()) else {
                return;
            };
            let target = focus.target.borrow().clone();
            let index = target
                .as_ref()
                .map_or(index, |range| {
                    pages
                        .characters
                        .partition_point(|p| p.start <= range.start)
                        .saturating_sub(1)
                })
                .min(count - 1);
            current.set(index);
            focus.page.set(index);
            let content = &pages.pages[index];
            let label = content_widgets::rich(&content.markup, open.clone());
            label.set_widget_name("studio-cell-math-page");
            label.set_hexpand(true);
            label.set_valign(gtk::Align::Start);
            crate::inline_math::decorate(&label, content);
            if let Some(configure) = &configure {
                configure(label.upcast_ref());
            }
            scroll.set_child(Some(&label));
            scroll.vadjustment().set_value(0.0);
            let range = &pages.characters[index];
            let continued = target.as_ref().is_some_and(|t| t.end > range.end);
            if let Some(target) = target.filter(|t| t.start < range.end && t.end > range.start) {
                let mut selection = target.start.saturating_sub(range.start)
                    ..target.end.min(range.end).saturating_sub(range.start);
                // A formula is one visual shape; retain its complete original
                // TeX in the selected range even for a partial-source match.
                for math in &content.math {
                    if selection.start < math.characters.end
                        && selection.end > math.characters.start
                    {
                        selection.start = selection.start.min(math.characters.start);
                        selection.end = selection.end.max(math.characters.end);
                    }
                }
                super::find::reveal(&label, selection);
            }
            if let Some(status) = status.upgrade() {
                crate::i18n_ui::bind_format(
                    &status,
                    "label",
                    if continued {
                        "Page {page}/{pages} · match continues on next page"
                    } else {
                        "Page {page}/{pages}"
                    },
                    &[
                        ("page", &(index + 1).to_string()),
                        ("pages", &count.to_string()),
                    ],
                );
            }
            for (i, button) in controls.iter().enumerate() {
                if let Some(button) = button.upgrade() {
                    button.set_sensitive(if i < 2 { index > 0 } else { index + 1 < count });
                }
            }
        });
        reading.renderer.replace(Some(Rc::downgrade(&render)));
        for (i, button) in buttons.into_iter().enumerate() {
            let render = render.clone();
            let page = page.clone();
            let reading = reading.clone();
            button.connect_clicked(move |b| {
                if !b.is_mapped() {
                    return;
                }
                reading.target.replace(None);
                render(match i {
                    0 => 0,
                    1 => page.get().saturating_sub(1),
                    2 => (page.get() + 1).min(count - 1),
                    _ => count - 1,
                });
            });
        }
        render(reading.page.get());
    });
    result.upcast()
}
