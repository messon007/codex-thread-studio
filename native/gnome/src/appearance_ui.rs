//! Native appearance application and explicit-save editor.
use crate::i18n_ui::UiText;
use crate::{
    appearance::{self, Plan, Reply, Snapshot},
    protocol::Command,
};
use adw::prelude::*;
use gtk::{gdk, glib};
use serde_json::{json, Value};
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::{Rc, Weak},
};
use tokio::sync::mpsc;
use vte::TerminalExt;
#[cfg(debug_assertions)]
#[path = "appearance_diagram_acceptance.rs"]
mod diagram_acceptance;
#[cfg(debug_assertions)]
#[path = "appearance_inline_acceptance.rs"]
mod inline_acceptance;
#[cfg(debug_assertions)]
#[path = "appearance_markdown_acceptance.rs"]
mod markdown_acceptance;
thread_local! {static ACTIVE:RefCell<Weak<Runtime>>=RefCell::new(Weak::new());}
pub struct Runtime {
    provider: gtk::CssProvider,
    display: gdk::Display,
    plan: RefCell<Option<Plan>>,
    pub snapshot: RefCell<Option<Snapshot>>,
    clamps: RefCell<Vec<glib::WeakRef<adw::Clamp>>>,
    terminals: RefCell<Vec<glib::WeakRef<vte::Terminal>>>,
    inline_labels: RefCell<Vec<Weak<InlineLabel>>>,
    inline_tags: RefCell<Vec<glib::WeakRef<gtk::TextTag>>>,
    markdown_blocks: RefCell<Vec<(glib::WeakRef<gtk::Widget>, i32, i32)>>,
    markdown_lines: RefCell<Vec<(glib::WeakRef<gtk::TextTag>, bool)>>,
    diagrams: RefCell<Vec<Weak<dyn Fn()>>>,
    dark: RefCell<Option<glib::SignalHandlerId>>,
}
impl Runtime {
    pub fn install() -> Rc<Self> {
        let display = gdk::Display::default().expect("GNOME display");
        let provider = gtk::CssProvider::new();
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
        );
        let runtime = Rc::new(Self {
            provider,
            display,
            plan: RefCell::new(None),
            snapshot: RefCell::new(None),
            clamps: RefCell::new(Vec::new()),
            terminals: RefCell::new(Vec::new()),
            inline_labels: RefCell::new(Vec::new()),
            inline_tags: RefCell::new(Vec::new()),
            markdown_blocks: RefCell::new(Vec::new()),
            markdown_lines: RefCell::new(Vec::new()),
            diagrams: RefCell::new(Vec::new()),
            dark: RefCell::new(None),
        });
        ACTIVE.with(|r| r.replace(Rc::downgrade(&runtime)));
        let weak = Rc::downgrade(&runtime);
        runtime
            .dark
            .replace(Some(adw::StyleManager::default().connect_dark_notify(
                move |_| {
                    if let Some(r) = weak.upgrade() {
                        r.css();
                    }
                },
            )));
        runtime
    }
    fn css(&self) {
        let Some(plan) = self.plan.borrow().clone() else {
            return;
        };
        let mut css = plan.css;
        css.push_str(".studio-flow-object-selected { outline:2px solid @accent_bg_color; outline-offset:-2px; }\n");
        if plan.contrast {
            let (bg, fg) = if adw::StyleManager::default().is_dark() {
                ("#000000", "#ffffff")
            } else {
                ("#ffffff", "#000000")
            };
            css.push_str(&format!("window {{ --window-bg-color:{bg}; --window-fg-color:{fg}; --view-bg-color:{bg}; --view-fg-color:{fg}; --card-bg-color:{bg}; --card-fg-color:{fg}; --popover-bg-color:{bg}; --popover-fg-color:{fg}; --dim-opacity:100%; --border-opacity:80%; }}\n.dim-label {{ opacity:1; }}\n.card, .boxed-list {{ outline:1px solid currentColor; }}\n"));
        }
        self.provider.load_from_string(&css);
    }
    pub fn apply(&self, reply: &Reply) {
        let diagrams_changed =
            self.plan.borrow().as_ref().map(|p| &p.diagram) != Some(&reply.plan.diagram);
        self.plan.replace(Some(reply.plan.clone()));
        self.snapshot.replace(Some(reply.snapshot.clone()));
        self.css();
        self.clamps.borrow_mut().retain(|w| {
            if let Some(c) = w.upgrade() {
                c.set_maximum_size(reply.plan.maximum);
                c.set_tightening_threshold(reply.plan.threshold);
                true
            } else {
                false
            }
        });
        self.terminals.borrow_mut().retain(|w| {
            if let Some(t) = w.upgrade() {
                Self::font(&t, &reply.plan);
                true
            } else {
                false
            }
        });
        self.inline_labels.borrow_mut().retain(|weak| {
            if let Some(label) = weak.upgrade().filter(|b| b.widget.upgrade().is_some()) {
                Self::inline_label(&label, Some(&reply.plan));
                true
            } else {
                false
            }
        });
        self.inline_tags.borrow_mut().retain(|weak| {
            if let Some(tag) = weak.upgrade() {
                Self::inline_tag(&tag, Some(&reply.plan));
                true
            } else {
                false
            }
        });
        self.markdown_blocks
            .borrow_mut()
            .retain(|(weak, top, bottom)| {
                if let Some(widget) = weak.upgrade() {
                    Self::block_spacing(&widget, *top, *bottom, reply.plan.markdown);
                    true
                } else {
                    false
                }
            });
        self.markdown_lines.borrow_mut().retain(|(weak, code)| {
            if let Some(tag) = weak.upgrade() {
                tag.set_line_height(if *code {
                    reply.plan.markdown.code_line_height()
                } else {
                    reply.plan.markdown.line_height()
                });
                true
            } else {
                false
            }
        });
        if diagrams_changed {
            let callbacks = {
                let mut listeners = self.diagrams.borrow_mut();
                listeners.retain(|w| w.strong_count() > 0);
                listeners
                    .iter()
                    .filter_map(Weak::upgrade)
                    .collect::<Vec<_>>()
            };
            for callback in callbacks {
                callback();
            }
        }
    }
    fn block_spacing(widget: &gtk::Widget, top: i32, bottom: i32, mode: appearance::MarkdownMode) {
        widget.set_margin_top((top + mode.spacing()).max(0));
        widget.set_margin_bottom((bottom + mode.spacing()).max(0));
    }
    fn code_font(plan: Option<&Plan>) -> Option<gtk::pango::FontDescription> {
        plan.and_then(Plan::code_font)
    }
    fn font(terminal: &vte::Terminal, plan: &Plan) {
        terminal.set_font(Self::code_font(Some(plan)).as_ref());
    }
    fn inline_tag(tag: &gtk::TextTag, plan: Option<&Plan>) {
        if let Some(font) = Self::code_font(plan) {
            tag.set_font_desc(Some(&font));
        } else {
            tag.set_family(Some("monospace"));
            tag.set_property("size-set", false);
            tag.set_property("weight-set", false);
        }
    }
    fn inline_label(binding: &InlineLabel, plan: Option<&Plan>) -> bool {
        let Some(label) = binding.widget.upgrade() else {
            return false;
        };
        let source = binding.source.borrow();
        let Some(source) = source.as_ref() else {
            return false;
        };
        let markup = plan.map_or_else(|| source.clone(), |p| p.inline_markup(source));
        if label.label().as_str() != markup {
            let selection = label.selection_bounds();
            // GObject can defer the same notify signal until the outer emission
            // finishes. Keep the applied value as well as the synchronous guard.
            binding.applied.replace(Some(markup.clone()));
            binding.setting.set(true);
            label.set_markup(&markup);
            binding.setting.set(false);
            if let Some((start, end)) = selection {
                label.select_region(start, end);
            }
        }
        true
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        if let Some(h) = self.dark.get_mut().take() {
            adw::StyleManager::default().disconnect(h);
        }
        gtk::style_context_remove_provider_for_display(&self.display, &self.provider);
    }
}
/// Shared transcript/composer bounds; gutters sit outside the content limit.
pub fn reading_clamp(child: &impl IsA<gtk::Widget>) -> adw::Clamp {
    let clamp = adw::Clamp::builder()
        .maximum_size(960)
        .tightening_threshold(960)
        .unit(adw::LengthUnit::Px)
        .margin_start(24)
        .margin_end(24)
        .child(child)
        .build();
    watch_clamp(&clamp);
    clamp
}

#[cfg(debug_assertions)]
fn width_allocation_acceptance() -> Result<Value, String> {
    let mut results = Vec::new();
    for (mode, limit) in [("comfortable", 960), ("wide", 1280), ("full", i32::MAX)] {
        let snapshot: Snapshot = serde_json::from_value(json!({
            "revision":"a".repeat(64), "values":{"contentWidth":mode},
            "fonts":{"ui":[],"content":[],"code":[],"workspace":[]}
        }))
        .map_err(|e| e.to_string())?;
        let plan = snapshot.plan()?;
        for available in [480, 4000] {
            let host = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let mut children = Vec::new();
            for text in ["Transcript", "Composer"] {
                let child = gtk::Label::builder()
                    .label(text)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .build();
                let clamp = reading_clamp(&child);
                clamp.set_maximum_size(plan.maximum);
                clamp.set_tightening_threshold(plan.threshold);
                host.append(&clamp);
                children.push(child);
            }
            host.measure(gtk::Orientation::Horizontal, -1);
            host.allocate(available, 160, -1, None);
            let widths: Vec<i32> = children.iter().map(|child| child.width()).collect();
            let expected = (available - 48).min(limit);
            if widths != [expected, expected] {
                return Err(format!(
                    "Reading width {mode} at {available}: {widths:?}, expected {expected}"
                ));
            }
            results.push(json!({"mode":mode,"available":available,"widths":widths}));
        }
    }
    Ok(results.into())
}

pub fn watch_clamp(clamp: &adw::Clamp) {
    ACTIVE.with(|a| {
        if let Some(r) = a.borrow().upgrade() {
            if let Some(p) = r.plan.borrow().as_ref() {
                clamp.set_maximum_size(p.maximum);
                clamp.set_tightening_threshold(p.threshold);
            }
            let mut list = r.clamps.borrow_mut();
            list.retain(|w| w.upgrade().is_some());
            list.push(clamp.downgrade());
        }
    });
}
pub fn watch_terminal(terminal: &vte::Terminal) {
    ACTIVE.with(|a| {
        if let Some(r) = a.borrow().upgrade() {
            if let Some(p) = r.plan.borrow().as_ref() {
                Runtime::font(terminal, p);
            }
            let mut list = r.terminals.borrow_mut();
            list.retain(|w| w.upgrade().is_some());
            list.push(terminal.downgrade());
        }
    });
}

/// Weak bindings update realized inline code after Save without rebuilding rows.
/// A label can later change between markup and plain text; its override follows it.
struct InlineLabel {
    widget: glib::WeakRef<gtk::Label>,
    source: RefCell<Option<String>>,
    applied: RefCell<Option<String>>,
    setting: Cell<bool>,
    registered: Cell<bool>,
}
pub fn watch_inline_label(label: &gtk::Label) {
    ACTIVE.with(|active| {
        if let Some(runtime) = active.borrow().upgrade() {
            let binding = Rc::new(InlineLabel {
                widget: label.downgrade(),
                source: RefCell::new(None),
                applied: RefCell::new(None),
                setting: Cell::new(false),
                registered: Cell::new(false),
            });
            let update = Rc::new(move |runtime: &Runtime, label: &gtk::Label| {
                if binding.setting.get() {
                    return;
                }
                let markup = label.label();
                if label.uses_markup()
                    && binding.applied.borrow().as_deref() == Some(markup.as_str())
                {
                    return;
                }
                binding.applied.replace(None);
                binding.source.replace(
                    (label.uses_markup() && markup.contains("<tt>")).then(|| markup.to_string()),
                );
                if Runtime::inline_label(&binding, runtime.plan.borrow().as_ref())
                    && !binding.registered.replace(true)
                {
                    let mut labels = runtime.inline_labels.borrow_mut();
                    if labels.len() % 64 == 0 {
                        labels.retain(|w| w.upgrade().is_some());
                    }
                    labels.push(Rc::downgrade(&binding));
                }
            });
            update(&runtime, label);
            let weak = Rc::downgrade(&runtime);
            let changed = update.clone();
            label.connect_label_notify(move |label| {
                if let Some(runtime) = weak.upgrade() {
                    changed(&runtime, label);
                }
            });
            let weak = Rc::downgrade(&runtime);
            label.connect_use_markup_notify(move |label| {
                if let Some(runtime) = weak.upgrade() {
                    update(&runtime, label);
                }
            });
        }
    });
}

pub fn watch_inline_tag(tag: &gtk::TextTag) {
    ACTIVE.with(|active| {
        if let Some(runtime) = active.borrow().upgrade() {
            Runtime::inline_tag(tag, runtime.plan.borrow().as_ref());
            let mut tags = runtime.inline_tags.borrow_mut();
            if tags.len() % 64 == 0 {
                tags.retain(|w| w.upgrade().is_some());
            }
            tags.push(tag.downgrade());
        }
    });
}
pub fn watch_markdown_block(widget: &impl IsA<gtk::Widget>) {
    let widget = widget.upcast_ref::<gtk::Widget>();
    widget.add_css_class("studio-markdown-block");
    ACTIVE.with(|active| {
        if let Some(runtime) = active.borrow().upgrade() {
            let (top, bottom) = (widget.margin_top(), widget.margin_bottom());
            if let Some(plan) = runtime.plan.borrow().as_ref() {
                Runtime::block_spacing(widget, top, bottom, plan.markdown);
            }
            let mut blocks = runtime.markdown_blocks.borrow_mut();
            if blocks.len() % 64 == 0 {
                blocks.retain(|(w, _, _)| w.upgrade().is_some());
            }
            blocks.push((widget.downgrade(), top, bottom));
        }
    });
}
pub fn markdown_line_tag(buffer: &gtk::TextBuffer, code: bool) -> gtk::TextTag {
    let tag = gtk::TextTag::builder()
        .name("studio-markdown-lines")
        .line_height(if code { 1.65 } else { 1.72 })
        .build();
    buffer.tag_table().add(&tag);
    ACTIVE.with(|active| {
        if let Some(runtime) = active.borrow().upgrade() {
            if let Some(plan) = runtime.plan.borrow().as_ref() {
                tag.set_line_height(if code {
                    plan.markdown.code_line_height()
                } else {
                    plan.markdown.line_height()
                });
            }
            let mut tags = runtime.markdown_lines.borrow_mut();
            if tags.len() % 64 == 0 {
                tags.retain(|(tag, _)| tag.upgrade().is_some());
            }
            tags.push((tag.downgrade(), code));
        }
    });
    tag
}
pub fn diagram_style() -> crate::diagram_style::Style {
    ACTIVE.with(|active| {
        active
            .borrow()
            .upgrade()
            .and_then(|runtime| runtime.plan.borrow().as_ref().map(|p| p.diagram.clone()))
            .unwrap_or_default()
    })
}
pub fn watch_diagrams(callback: &Rc<dyn Fn()>) {
    ACTIVE.with(|active| {
        if let Some(runtime) = active.borrow().upgrade() {
            let mut listeners = runtime.diagrams.borrow_mut();
            listeners.retain(|w| w.strong_count() > 0);
            listeners.push(Rc::downgrade(callback));
        }
    });
}

struct FontRows {
    family: adw::EntryRow,
    size: adw::SpinRow,
    weight: adw::ComboRow,
}
pub struct Panel {
    pub id: u64,
    pub dialog: adw::PreferencesDialog,
    #[cfg(debug_assertions)]
    controls_page: adw::PreferencesPage,
    parent: adw::ApplicationWindow,
    sender: mpsc::Sender<Command>,
    runtime: Rc<Runtime>,
    snapshot: RefCell<Option<Snapshot>>,
    serial: Cell<u64>,
    busy: Cell<bool>,
    dirty: Cell<bool>,
    updating: Cell<bool>,
    changed: RefCell<HashSet<String>>,
    custom: adw::SwitchRow,
    content_inherit: adw::SwitchRow,
    width: adw::ComboRow,
    markdown: adw::ComboRow,
    diagram_size: adw::SpinRow,
    diagram_density: adw::ComboRow,
    diagram_curve: adw::ComboRow,
    diagram_style: adw::ComboRow,
    diagram_layout: adw::ComboRow,
    contrast: adw::SwitchRow,
    fonts: Vec<FontRows>,
    groups: Vec<adw::PreferencesGroup>,
    save: gtk::Button,
    reload: gtk::Button,
    status: gtk::Label,
    error_toast: RefCell<Option<adw::Toast>>,
}
impl Panel {
    pub fn new(
        parent: &adw::ApplicationWindow,
        sender: mpsc::Sender<Command>,
        runtime: Rc<Runtime>,
    ) -> Rc<Self> {
        let dialog = adw::PreferencesDialog::builder()
            .title("Fonts and reading layout")
            .content_width(680)
            .content_height(720)
            .build()
            .ui_text("title", "Fonts and reading layout");
        let page = adw::PreferencesPage::builder()
            .title("Typography")
            .build()
            .ui_text("title", "Typography");
        let controls = adw::PreferencesGroup::new();
        let width = adw::ComboRow::builder()
            .title("Reading width")
            .model(&gtk::StringList::new(&[
                "Comfortable",
                "Wide",
                "Full width",
            ]))
            .build()
            .ui_text("title", "Reading width");
        controls.add(&width);
        let markdown = adw::ComboRow::builder()
            .title("Markdown layout")
            .subtitle("Reading adds space; Compact fits more text. Applies after Save.")
            .model(&gtk::StringList::new(&["Technical", "Reading", "Compact"]))
            .build()
            .ui_text("title", "Markdown layout")
            .ui_text(
                "subtitle",
                "Reading adds space; Compact fits more text. Applies after Save.",
            );
        controls.add(&markdown);
        let custom = adw::SwitchRow::builder()
            .title("Customize typography")
            .subtitle(
                "Share saved fonts with Studio clients. When off, this app uses GNOME defaults.",
            )
            .build()
            .ui_text("title", "Customize typography")
            .ui_text(
                "subtitle",
                "Share saved fonts with Studio clients. When off, this app uses GNOME defaults.",
            );
        controls.add(&custom);
        let content_inherit = adw::SwitchRow::builder()
            .title("Content follows interface typography")
            .build()
            .ui_text("title", "Content follows interface typography");
        controls.add(&content_inherit);
        let contrast = adw::SwitchRow::builder()
            .title("Increase contrast")
            .subtitle(
                "Use stronger text and panel boundaries. System high contrast remains supported.",
            )
            .build()
            .ui_text("title", "Increase contrast")
            .ui_text(
                "subtitle",
                "Use stronger text and panel boundaries. System high contrast remains supported.",
            );
        controls.add(&contrast);
        page.add(&controls);
        let mut fonts = Vec::new();
        let mut groups = Vec::new();
        for (title, max) in [
            ("Interface", 20.0),
            ("Reading and conversations", 24.0),
            ("Workspace tables and files", 20.0),
            ("Code and terminal", 20.0),
        ] {
            let group = adw::PreferencesGroup::builder().title(title).build();
            crate::i18n_ui::bind(&group, "title", title);
            let family = adw::EntryRow::builder()
                .title("Font family or fallback list")
                .build()
                .ui_text("title", "Font family or fallback list");
            let pick = gtk::Button::builder()
                .icon_name("font-select-symbolic")
                .tooltip_text("Choose font family")
                .valign(gtk::Align::Center)
                .build()
                .ui_text("tooltip-text", "Choose font family");
            family.add_suffix(&pick);
            let entry = family.downgrade();
            let parent = parent.downgrade();
            pick.connect_clicked(move |_| {
                let Some(parent) = parent.upgrade() else {
                    return;
                };
                let entry = entry.clone();
                gtk::FontDialog::new().choose_family(
                    Some(&parent),
                    None::<&gtk::pango::FontFamily>,
                    None::<&gtk::gio::Cancellable>,
                    move |result| {
                        if let (Ok(font), Some(entry)) = (result, entry.upgrade()) {
                            if entry.is_mapped() && entry.is_sensitive() {
                                entry.set_text(&format!(
                                    "\"{}\"",
                                    font.name().replace('\\', "\\\\").replace('"', "\\\"")
                                ));
                            }
                        }
                    },
                );
            });
            let size = adw::SpinRow::with_range(11.0, max, 0.5);
            crate::i18n_ui::bind(&size, "title", "Size (pixels)");
            size.set_digits(1);
            let weight = adw::ComboRow::builder()
                .title("Weight")
                .model(&gtk::StringList::new(&["Regular", "Medium", "Semibold"]))
                .build()
                .ui_text("title", "Weight");
            weight.set_factory(Some(&crate::i18n_ui::contextual_choice_factory(
                "font-weight",
            )));
            weight.set_list_factory(Some(&crate::i18n_ui::contextual_choice_factory(
                "font-weight",
            )));
            group.add(&family);
            group.add(&size);
            if title != "Workspace tables and files" {
                group.add(&weight);
            }
            page.add(&group);
            groups.push(group);
            fonts.push(FontRows {
                family,
                size,
                weight,
            });
        }
        let diagrams=adw::PreferencesGroup::builder().title("Mermaid diagrams")
            .description("Style and layout are shared with other Studio clients. Source directives can override diagram defaults.").build().ui_text("title","Mermaid diagrams").ui_text("description","Style and layout are shared with other Studio clients. Source directives can override diagram defaults.");
        let diagram_style = adw::ComboRow::builder()
            .title("Diagram style")
            .model(&gtk::StringList::new(&[
                "Auto",
                "Classic",
                "Neo",
                "Hand drawn",
                "Document",
            ]))
            .build()
            .ui_text("title", "Diagram style");
        let diagram_layout = adw::ComboRow::builder()
            .title("Diagram layout")
            .model(&gtk::StringList::new(&["Auto", "Dagre", "ELK"]))
            .build()
            .ui_text("title", "Diagram layout");
        let diagram_size = adw::SpinRow::with_range(12.0, 20.0, 1.0);
        crate::i18n_ui::bind(&diagram_size, "title", "Diagram text size (pixels)");
        diagram_size.set_digits(0);
        let diagram_density = adw::ComboRow::builder()
            .title("Flowchart spacing")
            .model(&gtk::StringList::new(&["Compact", "Standard", "Loose"]))
            .build()
            .ui_text("title", "Flowchart spacing");
        let diagram_curve = adw::ComboRow::builder()
            .title("Flowchart curves")
            .model(&gtk::StringList::new(&[
                "Rounded", "Straight", "Step", "Basis",
            ]))
            .build()
            .ui_text("title", "Flowchart curves");
        for row in [&width, &diagram_density, &diagram_curve] {
            row.set_factory(Some(&crate::i18n_ui::choice_factory()));
            row.set_list_factory(Some(&crate::i18n_ui::choice_factory()));
        }
        for (row, context) in [
            (&diagram_style, "diagram-style"),
            (&diagram_layout, "diagram-layout"),
        ] {
            row.set_factory(Some(&crate::i18n_ui::contextual_choice_factory(context)));
            row.set_list_factory(Some(&crate::i18n_ui::contextual_choice_factory(context)));
        }
        markdown.set_factory(Some(&crate::i18n_ui::contextual_choice_factory(
            "markdown-layout",
        )));
        markdown.set_list_factory(Some(&crate::i18n_ui::contextual_choice_factory(
            "markdown-layout",
        )));
        diagrams.add(&diagram_style);
        diagrams.add(&diagram_layout);
        diagrams.add(&diagram_size);
        diagrams.add(&diagram_density);
        diagrams.add(&diagram_curve);
        page.add(&diagrams);
        let actions = adw::PreferencesGroup::new();
        let row = adw::ActionRow::builder()
            .title("Shared appearance")
            .subtitle("Changes apply after Save.")
            .build()
            .ui_text("title", "Shared appearance")
            .ui_text("subtitle", "Changes apply after Save.");
        let reload = gtk::Button::with_label("Reload").ui_text("label", "Reload");
        reload.set_valign(gtk::Align::Center);
        let save = gtk::Button::with_label("Save").ui_text("label", "Save");
        save.set_valign(gtk::Align::Center);
        save.add_css_class("suggested-action");
        row.add_suffix(&reload);
        row.add_suffix(&save);
        actions.add(&row);
        let status = gtk::Label::builder()
            .wrap(true)
            .xalign(0.0)
            .label("Loading appearance…")
            .build()
            .ui_text("label", "Loading appearance…");
        actions.add(&status);
        page.add(&actions);
        dialog.add(&page);
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let panel = Rc::new(Self {
            id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            dialog,
            #[cfg(debug_assertions)]
            controls_page: page,
            parent: parent.clone(),
            sender,
            runtime,
            snapshot: RefCell::new(None),
            serial: Cell::new(0),
            busy: Cell::new(false),
            dirty: Cell::new(false),
            updating: Cell::new(false),
            changed: RefCell::new(HashSet::new()),
            custom,
            content_inherit,
            width,
            markdown,
            diagram_size,
            diagram_density,
            diagram_curve,
            diagram_style,
            diagram_layout,
            contrast,
            fonts,
            groups,
            save,
            reload,
            status,
            error_toast: RefCell::new(None),
        });
        for (i, prefix) in ["ui", "content", "workspace", "code"].iter().enumerate() {
            let weak = Rc::downgrade(&panel);
            let key = format!("{prefix}FontFamily");
            panel.fonts[i].family.connect_changed(move |_| {
                if let Some(p) = weak.upgrade() {
                    p.change(&key);
                }
            });
            let weak = Rc::downgrade(&panel);
            let key = format!("{prefix}FontSize");
            panel.fonts[i].size.connect_value_notify(move |_| {
                if let Some(p) = weak.upgrade() {
                    p.change(&key);
                }
            });
            let weak = Rc::downgrade(&panel);
            let key = format!("{prefix}FontWeight");
            panel.fonts[i].weight.connect_selected_notify(move |_| {
                if let Some(p) = weak.upgrade() {
                    p.change(&key);
                }
            });
        }
        for (row, key) in [
            (&panel.custom, "custom"),
            (&panel.content_inherit, "inherit"),
            (&panel.contrast, "highContrast"),
        ] {
            let weak = Rc::downgrade(&panel);
            row.connect_active_notify(move |_| {
                if let Some(p) = weak.upgrade() {
                    p.change(key);
                }
            });
        }
        let weak = Rc::downgrade(&panel);
        panel.width.connect_selected_notify(move |_| {
            if let Some(p) = weak.upgrade() {
                p.change("contentWidth");
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.markdown.connect_selected_notify(move |_| {
            if let Some(panel) = weak.upgrade() {
                panel.change("markdownMode");
            }
        });
        for (row, key) in [
            (&panel.diagram_density, "mermaidDensity"),
            (&panel.diagram_curve, "mermaidCurve"),
            (&panel.diagram_style, "mermaidStyle"),
            (&panel.diagram_layout, "mermaidLayout"),
        ] {
            let weak = Rc::downgrade(&panel);
            row.connect_selected_notify(move |_| {
                if let Some(panel) = weak.upgrade() {
                    panel.change(key);
                }
            });
        }
        let weak = Rc::downgrade(&panel);
        panel.diagram_size.connect_value_notify(move |_| {
            if let Some(panel) = weak.upgrade() {
                panel.change("mermaidFontSize");
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.save.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.save();
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.reload.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.reload();
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.dialog.connect_close_attempt(move |_| {
            if let Some(p) = weak.upgrade() {
                if p.busy.get() {
                    p.dialog.add_toast(adw::Toast::new(&crate::language::tr(
                        "Wait for appearance settings to finish",
                    )));
                } else if p.dirty.get() {
                    p.discard(true);
                }
            }
        });
        panel.request(appearance::Request::Read);
        panel
    }
    pub fn may_close(&self) -> bool {
        !self.busy.get() && !self.dirty.get()
    }
    fn change(&self, key: &str) {
        if self.updating.get() || self.snapshot.borrow().is_none() {
            return;
        }
        self.changed.borrow_mut().insert(key.into());
        self.dirty.set(true);
        crate::i18n_ui::bind(&self.status, "label", "Unsaved appearance changes");
        self.controls();
    }
    fn controls(&self) {
        let ready = !self.busy.get() && self.snapshot.borrow().is_some();
        self.save.set_sensitive(ready && self.dirty.get());
        self.reload.set_sensitive(!self.busy.get());
        self.custom.set_sensitive(ready);
        self.width.set_sensitive(ready);
        self.markdown.set_sensitive(ready);
        self.diagram_size.set_sensitive(ready);
        self.diagram_density.set_sensitive(ready);
        self.diagram_curve.set_sensitive(ready);
        self.content_inherit
            .set_sensitive(ready && self.custom.is_active());
        self.contrast
            .set_sensitive(ready && self.custom.is_active());
        for (i, g) in self.groups.iter().enumerate() {
            g.set_sensitive(
                ready && self.custom.is_active() && (i != 1 || !self.content_inherit.is_active()),
            );
        }
        self.dialog
            .set_can_close(!self.busy.get() && !self.dirty.get());
    }
    fn request(&self, request: appearance::Request) {
        if let Some(toast) = self.error_toast.borrow_mut().take() {
            toast.dismiss();
        }
        self.status.set_text(match &request {
            appearance::Request::Read => "Loading appearance…",
            appearance::Request::Save { .. } => "Saving appearance…",
        });
        self.serial.set(self.serial.get() + 1);
        self.busy.set(true);
        self.controls();
        if self
            .sender
            .try_send(Command::Appearance {
                dialog: self.id,
                serial: self.serial.get(),
                request,
            })
            .is_err()
        {
            self.busy.set(false);
            crate::i18n_ui::bind(
                &self.status,
                "label",
                "Appearance request queue is unavailable",
            );
            self.controls();
        }
    }
    pub fn event(&self, serial: u64, result: Result<Reply, String>) {
        if serial != self.serial.get() {
            return;
        }
        self.busy.set(false);
        match result {
            Ok(reply) => {
                self.runtime.apply(&reply);
                self.show(reply.snapshot);
            }
            Err(e) => {
                self.status.set_text(&e);
                let toast = adw::Toast::new(&e);
                self.dialog.add_toast(toast.clone());
                self.error_toast.replace(Some(toast));
            }
        }
        self.controls();
    }
    fn show(&self, snapshot: Snapshot) {
        self.updating.set(true);
        self.diagram_size
            .set_value(snapshot.values["mermaidFontSize"].as_f64().unwrap_or(15.0));
        self.diagram_density
            .set_selected(match snapshot.values["mermaidDensity"].as_str() {
                Some("compact") => 0,
                Some("loose") => 2,
                _ => 1,
            });
        self.diagram_curve
            .set_selected(match snapshot.values["mermaidCurve"].as_str() {
                Some("linear") => 1,
                Some("step") => 2,
                Some("basis") => 3,
                _ => 0,
            });
        self.diagram_style
            .set_selected(match snapshot.values["mermaidStyle"].as_str() {
                Some("classic") => 1,
                Some("neo") => 2,
                Some("handDrawn") => 3,
                Some("document") => 4,
                _ => 0,
            });
        self.diagram_layout
            .set_selected(match snapshot.values["mermaidLayout"].as_str() {
                Some("dagre") => 1,
                Some("elk") => 2,
                _ => 0,
            });
        self.markdown
            .set_selected(match snapshot.values["markdownMode"].as_str() {
                Some("reading") => 1,
                Some("compact") => 2,
                _ => 0,
            });
        let t = snapshot.values["typography"].clone();
        self.custom.set_active(!t.is_null());
        self.width
            .set_selected(match snapshot.values["contentWidth"].as_str() {
                Some("wide") => 1,
                Some("full") => 2,
                _ => 0,
            });
        self.contrast.set_active(t["highContrast"] == true);
        self.content_inherit.set_active(
            ["contentFontFamily", "contentFontSize", "contentFontWeight"]
                .iter()
                .all(|k| t[*k].is_null()),
        );
        for (i, prefix) in ["ui", "content", "workspace", "code"].iter().enumerate() {
            let family = t[format!("{prefix}FontFamily")]
                .as_str()
                .or_else(|| {
                    if i == 1 {
                        t["uiFontFamily"].as_str()
                    } else {
                        None
                    }
                })
                .unwrap_or(if i == 3 { "monospace" } else { "sans-serif" });
            self.fonts[i].family.set_text(family);
            let size = t[format!("{prefix}FontSize")]
                .as_f64()
                .or_else(|| {
                    if i == 1 {
                        t["uiFontSize"].as_f64()
                    } else {
                        None
                    }
                })
                .unwrap_or(14.0);
            self.fonts[i].size.set_value(size);
            let weight = t[format!("{prefix}FontWeight")]
                .as_u64()
                .or_else(|| {
                    if i == 1 {
                        t["uiFontWeight"].as_u64()
                    } else {
                        None
                    }
                })
                .unwrap_or(400);
            self.fonts[i].weight.set_selected(match weight {
                500 => 1,
                600 => 2,
                _ => 0,
            });
        }
        self.snapshot.replace(Some(snapshot));
        self.changed.borrow_mut().clear();
        self.dirty.set(false);
        self.updating.set(false);
        crate::i18n_ui::bind(&self.status, "label", "Saved appearance");
        self.controls();
    }
    fn save(&self) {
        let Some(snapshot) = self.snapshot.borrow().clone() else {
            return;
        };
        let mut values = snapshot.values;
        values["mermaidStyle"] = json!(
            ["auto", "classic", "neo", "handDrawn", "document"]
                [self.diagram_style.selected().min(4) as usize]
        );
        values["mermaidLayout"] =
            json!(["auto", "dagre", "elk"][self.diagram_layout.selected().min(2) as usize]);
        values["mermaidFontSize"] = json!(self.diagram_size.value().round() as u8);
        values["mermaidDensity"] = json!(
            ["compact", "standard", "loose"][self.diagram_density.selected().min(2) as usize]
        );
        values["mermaidCurve"] = json!(
            ["rounded", "linear", "step", "basis"][self.diagram_curve.selected().min(3) as usize]
        );
        values["markdownMode"] =
            json!(["technical", "reading", "compact"][self.markdown.selected().min(2) as usize]);
        values["contentWidth"] =
            json!(["comfortable", "wide", "full"][self.width.selected().min(2) as usize]);
        if !self.custom.is_active() {
            values["typography"] = Value::Null;
        } else {
            let fresh = values["typography"].is_null();
            if fresh {
                values["typography"] = json!({});
            }
            let keys = self.changed.borrow();
            for (i, prefix) in ["ui", "content", "workspace", "code"].iter().enumerate() {
                for (suffix, value) in [
                    ("FontFamily", json!(self.fonts[i].family.text().to_string())),
                    ("FontSize", json!(self.fonts[i].size.value())),
                    (
                        "FontWeight",
                        json!([400, 500, 600][self.fonts[i].weight.selected().min(2) as usize]),
                    ),
                ] {
                    if i == 2 && suffix == "FontWeight" {
                        continue;
                    }
                    let key = format!("{prefix}{suffix}");
                    if fresh || keys.contains(&key) || i == 1 && keys.contains("inherit") {
                        values["typography"][&key] = value;
                    }
                }
            }
            if self.content_inherit.is_active() {
                for key in ["contentFontFamily", "contentFontSize", "contentFontWeight"] {
                    values["typography"].as_object_mut().unwrap().remove(key);
                }
            }
            values["typography"]["highContrast"] = json!(self.contrast.is_active());
        }
        self.request(appearance::Request::Save {
            revision: snapshot.revision,
            values,
        });
    }
    fn reload(self: &Rc<Self>) {
        if self.dirty.get() {
            self.discard(false);
        } else {
            self.request(appearance::Request::Read);
        }
    }
    fn discard(self: &Rc<Self>, close: bool) {
        let d = adw::MessageDialog::builder()
            .transient_for(&self.parent)
            .heading(if close {
                "Discard appearance changes?"
            } else {
                "Reload saved appearance?"
            })
            .body("Your unsaved font and reading layout changes will be discarded.")
            .build();
        d.add_responses(&[("cancel", "Keep editing"), ("discard", "Discard")]);
        d.set_close_response("cancel");
        d.set_default_response(Some("cancel"));
        d.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        let weak = Rc::downgrade(self);
        d.connect_response(None, move |_, answer| {
            if answer == "discard" {
                if let Some(p) = weak.upgrade() {
                    if close {
                        p.dirty.set(false);
                        p.dialog.force_close();
                    } else {
                        p.request(appearance::Request::Read);
                    }
                }
            }
        });
        d.present();
    }
    #[cfg(debug_assertions)]
    async fn smoke_capture(&self, directory: &std::path::Path, name: &str) -> Result<(), String> {
        crate::window_capture::capture_dialog(&self.parent, &self.dialog, &directory.join(name))
            .await
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_test(
        self: &Rc<Self>,
        directory: &std::path::Path,
        restore: bool,
    ) -> Result<Value, String> {
        async fn until(test: impl Fn() -> bool) -> Result<(), String> {
            let start = std::time::Instant::now();
            while !test() {
                if start.elapsed() > std::time::Duration::from_secs(10) {
                    return Err("Appearance controls timed out".into());
                }
                glib::timeout_future(std::time::Duration::from_millis(25)).await;
            }
            Ok(())
        }
        fn confirm(heading: &str, answer: &str) -> Result<(), String> {
            let d = gtk::Window::list_toplevels()
                .into_iter()
                .filter_map(|w| w.downcast::<adw::MessageDialog>().ok())
                .find(|d| d.heading().as_deref() == Some(heading))
                .ok_or("Appearance discard confirmation missing")?;
            d.response(answer);
            Ok(())
        }
        until(|| !self.busy.get() && self.snapshot.borrow().is_some()).await?;
        let width_allocations = width_allocation_acceptance()?;
        let errors = Rc::new(RefCell::new(Vec::new()));
        let observed = errors.clone();
        let handler = self
            .runtime
            .provider
            .connect_parsing_error(move |_, _, error| {
                observed.borrow_mut().push(error.to_string());
            });
        // Existing and newly constructed widgets must both follow a saved plan.
        let clamp = adw::Clamp::new();
        watch_clamp(&clamp);
        let inline = inline_acceptance::Probe::new().await?;
        if restore {
            let snapshot = self.snapshot.borrow().clone().unwrap();
            if snapshot.values["typography"]["contentFontSize"].as_f64() != Some(19.0)
                || snapshot.values["contentWidth"] != "wide"
                || clamp.maximum_size() != 1280
                || snapshot.values["markdownMode"] != "reading"
                || self.markdown.selected() != 1
            {
                return Err("Saved shared appearance did not survive restart".into());
            }
            inline.verify(true)?;
            self.smoke_capture(directory, "appearance-restored.png")
                .await?;
            self.runtime.provider.disconnect(handler);
            return Ok(
                json!({"ok":true,"restored":true,"widthAllocations":width_allocations,"width":clamp.maximum_size(),"inlineCodeTypography":true}),
            );
        }
        inline.verify(false)?;
        self.custom.set_active(true);
        self.content_inherit.set_active(false);
        self.width.set_selected(1);
        for (i, size) in [13.0, 19.0, 16.0, 15.0].into_iter().enumerate() {
            self.fonts[i].family.set_text(if i == 3 {
                "\"DejaVu Sans Mono\", monospace"
            } else {
                "\"DejaVu Sans\", sans-serif"
            });
            self.fonts[i].size.set_value(size);
        }
        self.fonts[0].weight.set_selected(1);
        self.fonts[3].weight.set_selected(2);
        self.contrast.set_active(true);
        if clamp.maximum_size() != 960
            || !self.dirty.get()
            || !self.save.is_sensitive()
            || self.may_close()
        {
            return Err("Appearance changed before Save or lost its dirty guard".into());
        }
        // Invalid input stays editable; no style is applied before acknowledgement.
        self.fonts[0].family.set_text("sans-serif; color:red");
        self.save.emit_clicked();
        until(|| !self.busy.get()).await?;
        if !self.dirty.get() || clamp.maximum_size() != 960 {
            return Err("Rejected typography was applied or discarded".into());
        }
        self.fonts[0].family.set_text("\"DejaVu Sans\", sans-serif");
        self.save.emit_clicked();
        until(|| !self.busy.get()).await?;
        if self.dirty.get() || clamp.maximum_size() != 1280 {
            return Err(format!("Appearance save failed: {}", self.status.text()));
        }
        inline.verify(true)?;
        inline.selection_round_trip(&self.runtime)?;
        let modes = self.smoke_markdown_modes(directory, true).await?;
        let late_inline = inline_acceptance::Probe::new().await?;
        late_inline.verify(true)?;
        let sample = inline.page();
        self.dialog.add(&sample);
        self.dialog.set_visible_page(&sample);
        self.smoke_capture(directory, "appearance-inline-code.png")
            .await?;
        self.dialog.remove(&sample);
        let late = adw::Clamp::new();
        watch_clamp(&late);
        if late.maximum_size() != 1280 {
            return Err("New reading views lost the saved width".into());
        }
        let terminal = vte::Terminal::new();
        watch_terminal(&terminal);
        let font = terminal.font().ok_or("Terminal font missing")?;
        if font.size() != 15 * gtk::pango::SCALE || font.weight() != gtk::pango::Weight::Semibold {
            return Err(format!("Terminal did not follow code typography: {font}"));
        }
        let style = adw::StyleManager::default();
        style.set_color_scheme(adw::ColorScheme::ForceLight);
        self.smoke_capture(directory, "appearance-light.png")
            .await?;
        style.set_color_scheme(adw::ColorScheme::ForceDark);
        self.smoke_capture(directory, "appearance-dark.png").await?;
        style.set_color_scheme(adw::ColorScheme::ForceLight);
        // Edit conflict: an external appearance update must not erase local input.
        let path = directory.join("config/codex-thread-studio/settings.json");
        if std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from)
            != Some(directory.join("config"))
        {
            return Err("Appearance fixture needs a private profile".into());
        }
        let mut saved: Value =
            serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        saved["contentWidth"] = json!("comfortable");
        saved["futureAppearanceFixture"] = json!({"keep":true});
        std::fs::write(&path, serde_json::to_vec(&saved).unwrap()).map_err(|e| e.to_string())?;
        self.fonts[1].size.set_value(20.0);
        self.save.emit_clicked();
        until(|| !self.busy.get()).await?;
        if !self.dirty.get()
            || self.fonts[1].size.value() != 20.0
            || !self.status.text().contains("another client")
        {
            return Err("Appearance conflict did not retain local edits".into());
        }
        self.reload.emit_clicked();
        confirm("Reload saved appearance?", "cancel")?;
        if !self.dirty.get() {
            return Err("Cancel discarded typography edits".into());
        }
        self.reload.emit_clicked();
        confirm("Reload saved appearance?", "discard")?;
        until(|| !self.busy.get() && !self.dirty.get()).await?;
        if self.fonts[1].size.value() != 19.0 || clamp.maximum_size() != 960 {
            return Err("Reload did not apply the external appearance".into());
        }
        // Turning customization off restores GNOME font inheritance and VTE defaults.
        self.custom.set_active(false);
        self.save.emit_clicked();
        until(|| !self.busy.get()).await?;
        if self.dirty.get() || self.runtime.plan.borrow().as_ref().unwrap().code.is_some() {
            return Err("GNOME default font reset failed".into());
        }
        self.smoke_markdown_modes(directory, false).await?;
        inline.verify(false).map_err(|e| {
            format!(
                "Existing reset: {e}; bindings={}, weakAlive={}",
                self.runtime.inline_labels.borrow().len(),
                self.runtime
                    .inline_labels
                    .borrow()
                    .iter()
                    .filter(|w| w.upgrade().is_some())
                    .count()
            )
        })?;
        late_inline
            .verify(false)
            .map_err(|e| format!("Late reset: {e}"))?;
        self.custom.set_active(true);
        self.content_inherit.set_active(false);
        self.width.set_selected(1);
        for (i, size) in [13.0, 19.0, 16.0, 15.0].into_iter().enumerate() {
            self.fonts[i].family.set_text(if i == 3 {
                "\"DejaVu Sans Mono\", monospace"
            } else {
                "\"DejaVu Sans\", sans-serif"
            });
            self.fonts[i].size.set_value(size);
        }
        self.fonts[3].weight.set_selected(2);
        self.markdown.set_selected(1);
        self.save.emit_clicked();
        until(|| !self.busy.get() && !self.dirty.get()).await?;
        self.width.set_selected(2);
        self.dialog.close();
        confirm("Discard appearance changes?", "cancel")?;
        if !self.dirty.get() || self.width.selected() != 2 {
            return Err("Close cancellation lost appearance changes".into());
        }
        self.smoke_capture(directory, "appearance-edited.png")
            .await?;
        self.dialog.close();
        confirm("Discard appearance changes?", "discard")?;
        self.runtime.provider.disconnect(handler);
        if !errors.borrow().is_empty() {
            return Err(format!(
                "Native appearance CSS failed: {:?}",
                errors.borrow()
            ));
        }
        Ok(
            json!({"ok":true,"explicitSave":true,"invalidRetained":true,"conflictRetained":true,"reloadCancellation":true,"closeCancellation":true,"nativeFontReset":true,"terminalFont":font.to_string(),"existingAndNewWidth":true,"widthAllocations":width_allocations,"inlineCodeTypography":true,"inlineCodeReset":true,"inlineCodeReuse":true,"inlineCodeSelection":true,"inlineCodeEmphasis":true,"markdownModes":modes,"defaultFontModes":true,"cssErrors":errors.borrow().len()}),
        )
    }
}
