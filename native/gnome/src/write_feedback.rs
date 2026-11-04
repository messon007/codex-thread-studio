//! Window-owned write failures outlive the viewer that admitted the operation.
use adw::prelude::*;
use gtk::{gio, glib};
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    rc::Rc,
};
#[cfg(debug_assertions)]
#[path = "write_feedback_acceptance.rs"]
mod acceptance;

struct Failure {
    source: String,
    destination: String,
    error: String,
}
pub struct Queue {
    parent: glib::WeakRef<adw::ApplicationWindow>,
    waiting: RefCell<VecDeque<Failure>>,
    visible: RefCell<Option<adw::AlertDialog>>,
    overlay: glib::WeakRef<adw::ToastOverlay>,
    progress: RefCell<HashMap<&'static str, adw::Toast>>,
}
impl Queue {
    pub fn new(parent: &adw::ApplicationWindow, overlay: &adw::ToastOverlay) -> Rc<Self> {
        Rc::new(Self {
            parent: parent.downgrade(),
            waiting: RefCell::new(VecDeque::new()),
            visible: RefCell::new(None),
            overlay: overlay.downgrade(),
            progress: RefCell::new(HashMap::new()),
        })
    }
    pub fn export_failed(self: &Rc<Self>, source: &str, file: &gio::File, error: &str) {
        let (destination, error) = describe(file.path().as_deref(), &file.uri(), error);
        self.waiting.borrow_mut().push_back(Failure {
            source: bounded(source, 200),
            destination,
            error,
        });
        self.advance();
    }
    pub fn blocks_close(&self) -> bool {
        self.visible.borrow().is_some() || !self.waiting.borrow().is_empty()
    }
    pub fn waiting(&self, key: &'static str, message: &'static str) {
        self.finished(key);
        if let Some(overlay) = self.overlay.upgrade() {
            let toast = adw::Toast::new(message);
            toast.set_use_markup(false);
            crate::i18n_ui::bind(&toast, "title", message);
            overlay.add_toast(toast.clone());
            self.progress.borrow_mut().insert(key, toast);
        }
    }
    pub fn finished(&self, key: &'static str) {
        let toast = self.progress.borrow_mut().remove(key);
        if let Some(toast) = toast {
            toast.dismiss();
        }
    }
    #[cfg(debug_assertions)]
    pub fn smoke_waiting(&self, key: &'static str) -> bool {
        self.progress.borrow().contains_key(key)
    }
    fn advance(self: &Rc<Self>) {
        if self.visible.borrow().is_some() {
            return;
        }
        let Some(parent) = self.parent.upgrade() else {
            self.waiting.borrow_mut().clear();
            return;
        };
        let Some(failure) = self.waiting.borrow_mut().pop_front() else {
            return;
        };
        let details = gtk::Label::builder()
            .use_markup(false)
            .selectable(true)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .width_chars(1)
            .hexpand(true)
            .xalign(0.0)
            .yalign(0.0)
            .build();
        crate::i18n_ui::bind_format(
            &details,
            "label",
            "Destination: {destination}\n\n{error}",
            &[
                ("destination", &failure.destination),
                ("error", &failure.error),
            ],
        );
        details.set_widget_name("studio-write-failure-details");
        let scroll = gtk::ScrolledWindow::builder()
            .child(&details)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .min_content_height(48)
            .max_content_height(240)
            .propagate_natural_height(true)
            .build();
        let alert = adw::AlertDialog::builder()
            .heading("Export failed")
            .body_use_markup(false)
            .extra_child(&scroll)
            .prefer_wide_layout(true)
            .build();
        crate::i18n_ui::bind(&alert, "heading", "Export failed");
        crate::i18n_ui::bind_format(
            &alert,
            "body",
            "Could not export “{source}”.",
            &[("source", &failure.source)],
        );
        alert.add_response("close", "Close");
        crate::i18n_ui::response(&alert, "close", "Close");
        alert.set_close_response("close");
        alert.set_default_response(Some("close"));
        let weak = Rc::downgrade(self);
        alert.connect_closed(move |alert| {
            if let Some(queue) = weak.upgrade() {
                let current = queue.visible.borrow().as_ref() == Some(alert);
                if current {
                    queue.visible.borrow_mut().take();
                    queue.advance();
                }
            }
        });
        self.visible.replace(Some(alert.clone()));
        alert.present(Some(&parent));
    }
}
impl Drop for Queue {
    fn drop(&mut self) {
        for (_, toast) in self.progress.get_mut().drain() {
            toast.dismiss();
        }
        if let Some(alert) = self.visible.get_mut().take() {
            alert.force_close();
        }
    }
}
fn bounded(text: &str, count: usize) -> String {
    let mut chars = text.chars();
    let mut value = chars.by_ref().take(count).collect::<String>();
    if chars.next().is_some() {
        value.push('…');
    }
    value
}
fn describe(path: Option<&std::path::Path>, uri: &str, error: &str) -> (String, String) {
    if let Some(path) = path {
        return (bounded(&path.to_string_lossy(), 4096), bounded(error, 4096));
    }
    // Preserve the actual GFile for I/O. Only its presentation is redacted.
    if uri.len() > 64 * 1024 {
        return (
            "Nonlocal file".into(),
            "The remote destination could not be written".into(),
        );
    }
    let Ok(mut url) = url::Url::parse(uri) else {
        return (
            "Nonlocal file".into(),
            "The remote destination could not be written".into(),
        );
    };
    let mut secrets = url
        .query_pairs()
        .take(33)
        .map(|(_, value)| value.into_owned())
        .collect::<Vec<_>>();
    let excessive_query = secrets.len() > 32;
    if let Some(password) = url.password() {
        secrets.push(password.into());
        secrets.push(
            percent_encoding::percent_decode_str(password)
                .decode_utf8_lossy()
                .into_owned(),
        );
    }
    if let Some(fragment) = url.fragment() {
        secrets.push(fragment.into());
    }
    let _ = url.set_password(None);
    let _ = url.set_username("");
    url.set_query(None);
    url.set_fragment(None);
    let destination = bounded(url.as_str(), 4096);
    if excessive_query {
        return (
            destination,
            "The remote destination could not be written".into(),
        );
    }
    // Replace the whole known URI before clipping. Otherwise a URI crossing
    // the display boundary could retain a partial user-info/query credential.
    let mut error = if error.len() <= 64 * 1024 {
        error.replace(uri, &destination)
    } else {
        "The destination could not be written (diagnostic exceeds display limit)".into()
    };
    for secret in secrets.iter().filter(|secret| !secret.is_empty()) {
        // Deletion cannot grow the string or repeatedly match a marker that
        // another URI value also contains. Work stays bounded by the inputs.
        error = error.replace(secret, "");
    }
    (destination, bounded(&error, 4096))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_destination_is_not_interpreted_as_markup_or_a_uri() {
        let path = std::path::Path::new("/tmp/a 中文 # <tag>.md");
        let (destination, error) = describe(Some(path), "", "Permission denied");
        assert_eq!(destination, path.to_string_lossy());
        assert_eq!(error, "Permission denied");
    }
    #[test]
    fn remote_destination_and_known_uri_credentials_are_redacted() {
        let uri =
            "sftp://alice:p%40ssword@example.test/favorite.md?token=secret-token#private-fragment";
        let error = format!("Cannot replace {uri}: p@ssword secret-token private-fragment");
        let (destination, error) = describe(None, uri, &error);
        assert_eq!(destination, "sftp://example.test/favorite.md");
        for secret in [
            "alice",
            "p%40ssword",
            "p@ssword",
            "secret-token",
            "private-fragment",
        ] {
            assert!(!destination.contains(secret) && !error.contains(secret));
        }
    }
    #[test]
    fn error_details_and_titles_have_unicode_safe_bounds() {
        assert_eq!(bounded("中文😀abcd", 3), "中文😀…");
        assert_eq!(bounded("中文😀", 3), "中文😀");
        let (_, error) = describe(Some(std::path::Path::new("/tmp/a")), "", &"😀".repeat(5000));
        assert_eq!(error.chars().count(), 4097);
    }
    #[test]
    fn a_uri_crossing_the_display_boundary_is_redacted_before_clipping() {
        let uri = "sftp://alice:private-password@example.test/a?token=private-token";
        let error = format!("{}{uri}", "x".repeat(4080));
        let (_, error) = describe(None, uri, &error);
        assert!(!error.contains("alice") && !error.contains("private"));
        let (_, error) = describe(None, uri, &"😀".repeat(20000));
        assert!(error.contains("diagnostic exceeds display limit"));
    }
    #[test]
    fn overlapping_secret_values_cannot_expand_the_display_message() {
        let uri = "sftp://host.test/a?p=redacted&q=e&r=d";
        let (_, error) = describe(None, uri, "redacted error redacted");
        assert!(error.len() <= "redacted error redacted".len());
    }
}
