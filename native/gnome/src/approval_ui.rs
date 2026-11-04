//! GTK presentation of worker-prepared approval choices.
use crate::i18n_ui::UiText;
use crate::{approval, protocol::ReplyError};
use adw::prelude::*;
use serde_json::Value;
use std::{cell::Cell, rc::Rc, sync::Arc};

pub struct Choices {
    request: Result<Arc<approval::Request>, String>,
    policy: Option<gtk::DropDown>,
    permissions: Vec<gtk::CheckButton>,
    selection_buttons: Vec<gtk::Button>,
    error: gtk::Label,
}
pub struct Input {
    request: Result<Arc<approval::Request>, String>,
    response: String,
    policy: Option<usize>,
    selected: Vec<usize>,
}
impl Input {
    pub fn resolve(self) -> Result<Result<Value, ReplyError>, String> {
        let request = match self.request {
            Ok(r) => r,
            Err(message) => {
                return Ok(Err(ReplyError {
                    code: -32601,
                    message,
                }))
            }
        };
        Ok(request
            .answer(&self.response, self.policy, &self.selected)?
            .ok_or_else(|| ReplyError {
                code: -32001,
                message: "Approval closed without an available decision".into(),
            }))
    }
}
impl Choices {
    pub fn capture(&self, response: &str) -> Input {
        Input {
            request: self.request.clone(),
            response: response.into(),
            policy: self
                .policy
                .as_ref()
                .and_then(|p| p.selected().checked_sub(1))
                .map(|i| i as usize),
            selected: self
                .permissions
                .iter()
                .enumerate()
                .filter_map(|(i, c)| c.is_active().then_some(i))
                .collect(),
        }
    }
    pub fn cancellation(response: &str) -> bool {
        matches!(response, "deny" | "cancel" | "dismiss")
    }
    pub fn busy(&self, dialog: &adw::MessageDialog, busy: bool) {
        self.error.set_visible(false);
        if let Ok(r) = &self.request {
            for a in r.actions.iter().filter(|a| !a.persistent) {
                dialog.set_response_enabled(&a.id, !busy || Self::cancellation(&a.id));
            }
        }
        if let Some(policy) = &self.policy {
            policy.set_sensitive(!busy);
            dialog.set_response_enabled(
                "policy",
                !busy && policy.selected() > 0 && policy.selected() != gtk::INVALID_LIST_POSITION,
            );
        }
        for c in &self.permissions {
            c.set_sensitive(!busy);
        }
        for b in &self.selection_buttons {
            b.set_sensitive(!busy);
        }
    }
    pub fn report(&self, message: &str) {
        self.error.set_label(message);
        self.error.set_visible(true);
    }
}
pub fn create(
    parent: &impl IsA<gtk::Window>,
    backend: &str,
    prepared: Option<&Result<Arc<approval::Request>, String>>,
    fallback: &str,
) -> (adw::MessageDialog, Choices) {
    let request = prepared
        .cloned()
        .unwrap_or_else(|| Err("Approval data is unavailable; reconnect before responding".into()));
    let dialog =
        adw::MessageDialog::new(Some(parent), Some("Agent needs your input"), Some(backend));
    crate::i18n_ui::bind(&dialog, "heading", "Agent needs your input");
    dialog.set_default_size(520, -1);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let details = request
        .as_ref()
        .map(|r| r.details.as_str())
        .unwrap_or(fallback);
    let body = gtk::Label::builder()
        .label(details)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .selectable(true)
        .xalign(0.0)
        .build();
    body.add_css_class("monospace");
    content.append(
        &gtk::ScrolledWindow::builder()
            .child(&body)
            .max_content_height(280)
            .propagate_natural_height(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build(),
    );
    let mut policy = None;
    let mut permissions = Vec::new();
    let mut selection_buttons = Vec::new();
    let error = gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .visible(false)
        .build();
    error.add_css_class("error");
    if let Ok(request) = &request {
        for action in request.actions.iter().filter(|a| !a.persistent) {
            dialog.add_response(&action.id, &action.label);
            crate::i18n_ui::response(&dialog, &action.id, &action.label);
            if action.id == "accept" {
                dialog.set_response_appearance(&action.id, adw::ResponseAppearance::Suggested);
            }
        }
        if request.close == "dismiss" {
            dialog.add_response("dismiss", "Close");
            crate::i18n_ui::response(&dialog, "dismiss", "Close");
        }
        dialog.set_close_response(&request.close);
        if let Some(profile) = &request.permissions {
            let title = gtk::Label::builder()
                .label("Select permissions to grant")
                .xalign(0.0)
                .build()
                .ui_text("label", "Select permissions to grant");
            title.add_css_class("heading");
            content.append(&title);
            let rows = gtk::Box::new(gtk::Orientation::Vertical, 10);
            for grant in &profile.grants {
                let details = gtk::Box::new(gtk::Orientation::Vertical, 3);
                let label = gtk::Label::builder()
                    .label(&grant.label)
                    .xalign(0.0)
                    .wrap(true)
                    .build();
                crate::i18n_ui::bind(&label, "label", &grant.label);
                label.add_css_class("heading");
                details.append(&label);
                let description = gtk::Label::builder()
                    .label(&grant.description)
                    .xalign(0.0)
                    .wrap(true)
                    .wrap_mode(gtk::pango::WrapMode::WordChar)
                    .selectable(true)
                    .build();
                details.append(&description);
                let check = gtk::CheckButton::builder()
                    .child(&details)
                    .active(true)
                    .build();
                check.update_relation(&[
                    gtk::accessible::Relation::LabelledBy(&[label.upcast_ref()]),
                    gtk::accessible::Relation::DescribedBy(&[description.upcast_ref()]),
                ]);
                rows.append(&check);
                permissions.push(check);
            }
            for restriction in &profile.restrictions {
                let label = gtk::Label::builder()
                    .label(restriction)
                    .xalign(0.0)
                    .wrap(true)
                    .wrap_mode(gtk::pango::WrapMode::WordChar)
                    .selectable(true)
                    .build();
                label.add_css_class("dim-label");
                rows.append(&label);
            }
            // A separate label avoids retaining checkbox objects through their
            // own signal handlers. Signal closures use weak checkbox handles.
            let weak_controls: Vec<_> = permissions.iter().map(|c| c.downgrade()).collect();
            let status = gtk::Label::builder().xalign(0.0).build();
            status.add_css_class("dim-label");
            let count = Rc::new(Cell::new(permissions.len()));
            let total = permissions.len();
            for c in &permissions {
                let count = count.clone();
                let previous = Cell::new(c.is_active());
                let status = status.clone();
                let error = error.clone();
                c.connect_toggled(move |c| {
                    let active = c.is_active();
                    if previous.replace(active) != active {
                        count.set(if active {
                            count.get() + 1
                        } else {
                            count.get().saturating_sub(1)
                        });
                    }
                    permission_count(&status, count.get(), total);
                    error.set_visible(false);
                });
            }
            permission_count(&status, permissions.len(), permissions.len());
            content.append(&status);
            if !permissions.is_empty() {
                let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                for (text, active) in [("Select all", true), ("Clear selection", false)] {
                    let b = gtk::Button::with_label(text);
                    crate::i18n_ui::bind(&b, "label", text);
                    let controls = weak_controls.clone();
                    b.connect_clicked(move |_| {
                        for c in controls.iter().filter_map(|c| c.upgrade()) {
                            c.set_active(active);
                        }
                    });
                    buttons.append(&b);
                    selection_buttons.push(b);
                }
                content.append(&buttons);
            }
            if profile.grants.is_empty() && profile.restrictions.is_empty() {
                rows.append(
                    &gtk::Label::builder()
                        .label("No additional permissions requested.")
                        .xalign(0.0)
                        .build()
                        .ui_text("label", "No additional permissions requested."),
                );
            }
            content.append(
                &gtk::ScrolledWindow::builder()
                    .child(&rows)
                    .max_content_height(240)
                    .propagate_natural_height(true)
                    .hscrollbar_policy(gtk::PolicyType::Never)
                    .build(),
            );
            content.append(&gtk::Label::builder().wrap(true).xalign(0.0).build().ui_text("label", "Unchecked grants are omitted. Requested restrictions are preserved when granting permissions."));
            content.append(&gtk::Label::builder().wrap(true).xalign(0.0).build().ui_text("label", "Allow for turn with review asks Codex to review every subsequent command in this turn."));
        }
        let rules: Vec<_> = request.actions.iter().filter(|a| a.persistent).collect();
        if !rules.is_empty() {
            let title = gtk::Label::builder()
                .label("Remember a rule")
                .xalign(0.0)
                .build()
                .ui_text("label", "Remember a rule");
            title.add_css_class("heading");
            content.append(&title);
            let labels: Vec<_> = std::iter::once("Choose a policy…".to_owned())
                .chain(
                    rules
                        .iter()
                        .enumerate()
                        .map(|(i, a)| format!("{}. {}", i + 1, a.label)),
                )
                .collect();
            let values: Vec<_> = labels.iter().map(String::as_str).collect();
            let select = gtk::DropDown::from_strings(&values);
            select.update_relation(&[gtk::accessible::Relation::LabelledBy(&[title.upcast_ref()])]);
            crate::i18n_ui::bind(
                &select,
                "tooltip-text",
                "Select a server-proposed policy and review its exact scope before applying it",
            );
            let keys = Arc::new(
                rules
                    .iter()
                    .map(|action| action.label.clone())
                    .collect::<Vec<_>>(),
            );
            select.set_factory(Some(&policy_factory(keys.clone())));
            select.set_list_factory(Some(&policy_factory(keys)));
            content.append(&select);
            let description = gtk::Label::builder()
                .label("No persistent policy selected.")
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .selectable(true)
                .xalign(0.0)
                .build()
                .ui_text("label", "No persistent policy selected.");
            select.update_relation(&[gtk::accessible::Relation::DescribedBy(&[
                description.upcast_ref()
            ])]);
            content.append(
                &gtk::ScrolledWindow::builder()
                    .child(&description)
                    .max_content_height(180)
                    .propagate_natural_height(true)
                    .hscrollbar_policy(gtk::PolicyType::Never)
                    .build(),
            );
            dialog.add_response("policy", "Apply selected policy");
            crate::i18n_ui::response(&dialog, "policy", "Apply selected policy");
            dialog.set_response_enabled("policy", false);
            let weak = dialog.downgrade();
            let source = request.clone();
            select.connect_selected_notify(move |select| {
                let chosen = select.selected().checked_sub(1).and_then(|i| {
                    source
                        .actions
                        .iter()
                        .filter(|a| a.persistent)
                        .nth(i as usize)
                });
                if let Some(action) = chosen {
                    // The exact worker-prepared rule remains literal.
                    crate::i18n_ui::literal(&description, "label", &action.explanation);
                } else {
                    crate::i18n_ui::bind(&description, "label", "No persistent policy selected.");
                }
                if let Some(dialog) = weak.upgrade() {
                    dialog.set_response_enabled("policy", chosen.is_some());
                }
            });
            policy = Some(select);
        }
    } else if let Err(error) = &request {
        content.append(
            &gtk::Label::builder()
                .label(error)
                .wrap(true)
                .xalign(0.0)
                .build(),
        );
        dialog.add_response("deny", "Close");
        crate::i18n_ui::response(&dialog, "deny", "Close");
        dialog.set_close_response("deny");
    }
    content.append(&error);
    dialog.set_extra_child(Some(&content));
    (
        dialog,
        Choices {
            request,
            policy,
            permissions,
            selection_buttons,
            error,
        },
    )
}
fn permission_count(label: &gtk::Label, selected: usize, total: usize) {
    crate::i18n_ui::bind_format(
        label,
        "label",
        "{selected} of {total} requested permissions selected",
        &[
            ("selected", &selected.to_string()),
            ("total", &total.to_string()),
        ],
    );
}
fn policy_factory(keys: Arc<Vec<String>>) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, item| {
        item.downcast_ref::<gtk::ListItem>()
            .unwrap()
            .set_child(Some(
                &gtk::Label::builder()
                    .xalign(0.0)
                    .wrap(true)
                    .max_width_chars(42)
                    .build(),
            ));
    });
    factory.connect_bind(move |_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().unwrap();
        let Some(label) = item.child().and_downcast::<gtk::Label>() else {
            return;
        };
        let index = item.position() as usize;
        if index == 0 {
            crate::i18n_ui::bind(&label, "label", "Choose a policy…");
        } else if let Some(key) = keys.get(index - 1) {
            // Keys are fixed UI labels from the normalized decision enum. The
            // model and server-supplied decision objects stay unchanged.
            crate::i18n_ui::bind_format(
                &label,
                "label",
                &format!("{{number}}. {key}"),
                &[("number", &index.to_string())],
            );
        }
    });
    factory
}
