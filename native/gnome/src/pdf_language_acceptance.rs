//! Live language changes in mapped PDF controls, without document translation.
use super::*;
impl Pdf {
    pub(super) async fn smoke_language(
        self: &Rc<Self>,
        capture: &impl Fn(&str),
    ) -> Result<(), String> {
        let hit = self
            .hits
            .row_at_index(0)
            .and_downcast::<adw::ActionRow>()
            .ok_or("PDF search row missing")?;
        let snippet = hit.subtitle();
        let outline = self
            .outline
            .row_at_index(0)
            .and_then(|r| r.child())
            .and_downcast::<gtk::Button>()
            .ok_or("PDF outline missing")?;
        let heading = outline.label();
        let title = self.title.title();
        let subtitle = self.title.subtitle();
        let buffer = self.text.buffer();
        let body = buffer.text(&buffer.start_iter(), &buffer.end_iter(), true);
        let selection = buffer
            .selection_bounds()
            .map(|(a, b)| (a.offset(), b.offset()));
        let before = {
            let s = self.state.borrow();
            (
                s.generation,
                s.serial,
                s.id.clone(),
                s.page,
                s.query.clone(),
            )
        };
        // The actual link-list factory must localize destinations and preserve URLs.
        let page_link = crate::pdf_navigation::Link {
            rects: vec![],
            target: crate::pdf_navigation::Target::Page { page: 1 },
        };
        let url = "https://example.invalid/Search%20complete?locale=中文";
        let url_link = crate::pdf_navigation::Link {
            rects: vec![],
            target: crate::pdf_navigation::Target::Url { url: url.into() },
        };
        self.links
            .append(&glib::BoxedAnyObject::new(page_link.clone()));
        self.links
            .append(&glib::BoxedAnyObject::new(url_link.clone()));
        self.link_button.set_active(true);
        self.hover_link(Some(&page_link));
        for locale in ["zh-CN", "en-US", "zh-CN"] {
            crate::language::set(locale, "en-US")?;
            crate::i18n_ui::apply();
            crate::window_capture::next_frame(&self.widget).await?;
            let tr = crate::language::tr;
            if self
                .picture
                .animation_picture()
                .alternative_text()
                .as_deref()
                != Some(tr("PDF page").as_str())
                || self.preview.label().as_deref() != Some(tr("Preview").as_str())
                || self.text_mode.label().as_deref() != Some(tr("Text").as_str())
                || self.fit.label().as_deref() != Some(tr("Fit width").as_str())
                || self.more.label().as_deref() != Some(tr("Next results").as_str())
                || self.search_bar.placeholder_text().as_deref() != Some(tr("Search PDF…").as_str())
                || self.copy.tooltip_text().as_deref()
                    != Some(tr("Copy selected or page text").as_str())
                || self.next.tooltip_text().as_deref() != Some(tr("Next page").as_str())
                || self.status.text() != tr("Comment added")
                || hit.title() != crate::language::format("Page {value}", &[("value", "1")])
                || self.picture.tooltip_text().as_deref()
                    != Some(crate::language::format("Go to page {page}", &[("page", "2")]).as_str())
            {
                return Err(format!("PDF controls did not follow {locale}"));
            }
            fn labels(w: &gtk::Widget, out: &mut Vec<String>) {
                if let Some(label) = w.downcast_ref::<gtk::Label>() {
                    out.push(label.text().into());
                }
                let mut child = w.first_child();
                while let Some(w) = child {
                    child = w.next_sibling();
                    labels(&w, out);
                }
            }
            let mut values = Vec::new();
            labels(self.link_view.upcast_ref(), &mut values);
            if !values.contains(&crate::language::format(
                "Go to page {page}",
                &[("page", "2")],
            )) || !values.iter().any(|s| s == url)
            {
                return Err(format!(
                    "PDF link factory lost destinations or URL: {values:?}"
                ));
            }
            let s = self.state.borrow();
            if (
                s.generation,
                s.serial,
                s.id.clone(),
                s.page,
                s.query.clone(),
            ) != before
                || self.title.title() != title
                || self.title.subtitle() != subtitle
                || hit.subtitle() != snippet
                || outline.label() != heading
                || buffer.text(&buffer.start_iter(), &buffer.end_iter(), true) != body
                || buffer
                    .selection_bounds()
                    .map(|(a, b)| (a.offset(), b.offset()))
                    != selection
                || !self.text_mode.is_active()
            {
                return Err("PDF language change modified document or interaction state".into());
            }
        }
        capture("pdf-language-zh.png");
        // Retire equal-valued bindings before model values replace UI messages.
        crate::language::set("en-US", "en-US")?;
        crate::i18n_ui::apply();
        let recycled = gtk::Label::new(None);
        link_text(&recycled, "label", &page_link.target);
        crate::i18n_ui::literal(&recycled, "label", "Go to page 2");
        self.notice("Search complete");
        self.diagnostic("Search complete");
        self.hover_link(None);
        crate::language::set("zh-CN", "en-US")?;
        crate::i18n_ui::apply();
        if recycled.text() != "Go to page 2"
            || self.status.text() != "Search complete"
            || self.status.tooltip_text().as_deref() != Some("Search complete")
            || self.picture.tooltip_text().is_some()
        {
            return Err("PDF stale translation replaced literal data or cleared hover".into());
        }
        self.hover_link(Some(&url_link));
        crate::language::set("en-US", "en-US")?;
        crate::i18n_ui::apply();
        if self.picture.tooltip_text().as_deref() != Some(url) {
            return Err("PDF URL hover was translated".into());
        }
        // New backend replies must use the current locale too, rather than
        // merely updating widgets which existed before the switch.
        crate::language::set("zh-CN", "en-US")?;
        crate::i18n_ui::apply();
        self.load(2);
        crate::acceptance_wait::until("localized fresh PDF page", || {
            self.state.borrow().page == 2 && !self.state.borrow().busy
        })
        .await?;
        if self.status.text() != crate::language::tr("Read only") {
            return Err(format!(
                "Fresh page status was not localized: {}",
                self.status.text()
            ));
        }
        self.search_bar.set_text("Searchable");
        crate::acceptance_wait::until("localized fresh PDF search", || {
            self.state.borrow().query == "Searchable"
                && self.more.is_sensitive()
                && self.hits.row_at_index(2).is_some()
        })
        .await?;
        if self.status.text() != crate::language::tr("Search complete")
            || self
                .hits
                .row_at_index(2)
                .and_downcast::<adw::ActionRow>()
                .is_none_or(|r| {
                    r.title() != crate::language::format("Page {value}", &[("value", "3")])
                })
        {
            return Err("Fresh PDF search result was not localized".into());
        }
        capture("pdf-search-language-zh.png");
        crate::language::set("en-US", "en-US")?;
        crate::i18n_ui::apply();
        self.hover_link(None);
        self.links.remove_all();
        self.sidebar.set_reveal_child(false);
        self.notice("Comment added");
        Ok(())
    }
    pub(super) async fn smoke_password_language(
        &self,
        dialog: &adw::AlertDialog,
    ) -> Result<(), String> {
        let entry = Self::smoke_prompt_entry(dialog)?;
        entry.set_text("private-language-password");
        let serial = self.state.borrow().serial;
        for locale in ["zh-CN", "en-US"] {
            crate::language::set(locale, "en-US")?;
            crate::i18n_ui::apply();
            crate::window_capture::next_frame(dialog).await?;
            let tr = crate::language::tr;
            if dialog.heading().as_deref() != Some(tr("Unlock PDF").as_str())
                || dialog.body() != tr("Enter the document password.")
                || dialog.response_label("cancel") != tr("Cancel")
                || dialog.response_label("open") != tr("Open")
                || entry.text() != "private-language-password"
                || self.state.borrow().serial != serial
                || dialog.close_response() != "cancel"
                || dialog.default_response().as_deref() != Some("open")
            {
                return Err("Live PDF unlock translation modified the prompt or password".into());
            }
            if locale == "zh-CN" {
                if let Some(root) = std::env::var_os("STUDIO_GNOME_SMOKE_DIR") {
                    crate::window_capture::capture_dialog(
                        &self.parent.upgrade().ok_or("PDF parent missing")?,
                        dialog,
                        &std::path::PathBuf::from(root).join("pdf-password-language-zh.png"),
                    )
                    .await?;
                }
            }
        }
        entry.set_text("");
        Ok(())
    }
}
