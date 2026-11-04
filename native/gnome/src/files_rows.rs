//! Presentation for recycled Files rows; model values stay literal.
use super::*;
pub(super) fn present(
    data: &Data,
    root: bool,
    icon: &gtk::Image,
    label: &gtk::Label,
    size: &gtk::Label,
) {
    match data {
        Data::Entry(e) => {
            icon.set_icon_name(Some(if e.kind == "directory" {
                "folder-symbolic"
            } else {
                "text-x-generic-symbolic"
            }));
            if root && e.path.is_empty() {
                i18n_ui::bind(label, "label", "Project");
            } else if root && e.read_only {
                // Only synthetic shared roots have this prefix;
                // ordinary files with the same name stay literal.
                if let Some(name) = e.name.strip_prefix("Shared · ") {
                    i18n_ui::bind_format(label, "label", "Shared · {name}", &[("name", name)]);
                } else {
                    i18n_ui::literal(label, "label", &e.name);
                }
            } else {
                i18n_ui::literal(label, "label", &e.name);
            }
            i18n_ui::literal(label, "tooltip-text", &e.path);
            let bytes = e.size.map(|n| glib::format_size(n).to_string());
            match (bytes, e.read_only) {
                (Some(n), true) => {
                    i18n_ui::bind_format(size, "label", "{size} · Read only", &[("size", &n)])
                }
                (None, true) => i18n_ui::bind(size, "label", "Read only"),
                (Some(n), false) => i18n_ui::literal(size, "label", &n),
                _ => i18n_ui::literal(size, "label", ""),
            }
        }
        Data::More { .. } => {
            icon.set_icon_name(Some("view-more-symbolic"));
            i18n_ui::bind(label, "label", "Load more files");
            i18n_ui::literal(label, "tooltip-text", "");
            i18n_ui::literal(size, "label", "");
        }
        Data::Notice { message, error, .. } => {
            icon.set_icon_name(Some("dialog-information-symbolic"));
            for property in ["label", "tooltip-text"] {
                if let Some(error) = error {
                    i18n_ui::bind_format(label, property, message, &[("error", error)]);
                } else {
                    i18n_ui::bind(label, property, message);
                }
            }
            i18n_ui::literal(size, "label", "");
        }
    }
}
