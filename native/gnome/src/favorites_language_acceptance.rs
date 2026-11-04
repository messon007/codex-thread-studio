//! Live binding/data checks driven by the existing real language controller.
use super::*;
use std::path::Path;

const NOTE: &str = "Note Save <b>Settings</b> 中文😀 {count}\n";
pub struct LanguageProbe {
    alert: adw::AlertDialog,
    original: Favorite,
}

fn button(root: &gtk::Widget, caption: &str) -> bool {
    if root
        .downcast_ref::<gtk::Button>()
        .is_some_and(|b| b.label().as_deref() == Some(caption))
    {
        return true;
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if button(&widget, caption) {
            return true;
        }
    }
    false
}

impl Favorites {
    pub async fn smoke_language_prepare(
        self: &Rc<Self>,
        root: &Path,
    ) -> Result<LanguageProbe, String> {
        let original = Favorite {
            id: "favorite-language-probe".into(),
            scope: "message".into(),
            backend: "codex".into(),
            thread_id: "target".into(),
            thread_title: "Settings".into(),
            project_path: root.join("project").to_string_lossy().into_owned(),
            turn_id: "favorite-language-turn".into(),
            item_id: "favorite-language-answer".into(),
            title: "Save".into(),
            question: "Question Source 中文😀".into(),
            content: "# Save\r\n\r\nSource Settings 中文😀 {count}\r\n".into(),
            note: "Original note".into(),
            tags: vec!["Settings".into()],
            created_at: "2026-10-26T00:00:00Z".into(),
        };
        self.draft(original.clone());
        Self::until(|| {
            self.new.get()
                && self.current.borrow().is_some()
                && self.summary.text() == "0 of 0 favorites"
        })
        .await?;
        self.title.set_text("Send");
        self.tags.set_text("Save, Settings");
        self.note.buffer().set_text(NOTE);
        self.views.set_visible_child_name("source");
        self.close_attempt();
        let parent = self
            .dialog
            .root()
            .and_downcast::<adw::ApplicationWindow>()
            .ok_or("Favorite language parent missing")?;
        let alert = Self::alert(&parent).await?;
        let probe = LanguageProbe { alert, original };
        self.smoke_language_check(&probe, false)?;
        Ok(probe)
    }

    pub fn smoke_language_check(&self, probe: &LanguageProbe, chinese: bool) -> Result<(), String> {
        let expected = |english, chinese_text| if chinese { chinese_text } else { english };
        if self.dialog.title() != expected("Favorites", "收藏")
            || self.save.label().as_deref() != Some(expected("Save", "保存"))
            || self.source.label().as_deref() != Some(expected("Open source", "打开来源"))
            || self.export.tooltip_text().as_deref()
                != Some(expected(
                    "Export all favorites as Markdown",
                    "将全部收藏导出为 Markdown",
                ))
            || self.summary.text() != expected("0 of 0 favorites", "显示 0 / 0 条收藏")
        {
            return Err("Existing Favorites controls/counter did not follow the language".into());
        }
        let page = self
            .views
            .child_by_name("source")
            .ok_or("Favorite Source page missing")?;
        if self.views.page(&page).title().as_deref() != Some(expected("Source", "源码"))
            || self.views.visible_child_name().as_deref() != Some("source")
        {
            return Err("Favorites language changed the view identity or lost its title".into());
        }
        if probe.alert.heading().as_deref()
            != Some(expected("Save favorite changes?", "保存收藏更改？"))
            || probe.alert.body() != expected("Changes have not been saved.", "更改尚未保存。")
            || probe.alert.default_response().as_deref() != Some("save")
            || probe.alert.close_response() != "cancel"
            || !button(probe.alert.upcast_ref(), expected("Cancel", "取消"))
            || !button(probe.alert.upcast_ref(), expected("Discard", "放弃"))
            || !button(probe.alert.upcast_ref(), expected("Save", "保存"))
        {
            return Err(
                "Open favorite prompt lost translated labels or stable response IDs".into(),
            );
        }
        let buffer = self.note.buffer();
        if self.title.text() != "Send"
            || self.tags.text() != "Save, Settings"
            || buffer.text(&buffer.start_iter(), &buffer.end_iter(), true) != NOTE
            || !self.dirty.get()
            || !self.new.get()
            || self.write_serial.get() != 0
            || self.current.borrow().as_ref().is_none_or(|v| {
                v.content != probe.original.content
                    || v.title != probe.original.title
                    || v.question != probe.original.question
            })
        {
            return Err(
                "Live Favorites language translated, saved or discarded literal draft data".into(),
            );
        }
        Ok(())
    }

    pub async fn smoke_language_finish(
        self: &Rc<Self>,
        probe: &LanguageProbe,
        root: &Path,
        parent: &adw::ApplicationWindow,
    ) -> Result<(), String> {
        self.smoke_language_check(probe, true)?;
        glib::timeout_future(std::time::Duration::from_millis(350)).await;
        crate::window_capture::capture(parent, &root.join("language-favorites-prompt.png")).await?;
        Self::respond(&probe.alert, "取消")?;
        Self::until(|| !self.prompt.get() && !probe.alert.is_mapped()).await?;
        crate::window_capture::capture(parent, &root.join("language-favorites-editor.png")).await?;
        self.save.emit_clicked();
        Self::until(|| self.write_serial.get() == 0 && !self.new.get() && !self.dirty.get())
            .await?;
        let saved = self
            .current
            .borrow()
            .clone()
            .ok_or("Localized favorite save lost its result")?;
        if saved.title != "Send"
            || saved.note != NOTE
            || saved.tags != ["Save", "Settings"]
            || saved.content != probe.original.content
            || saved.backend != probe.original.backend
            || saved.thread_id != probe.original.thread_id
            || saved.turn_id != probe.original.turn_id
            || saved.item_id != probe.original.item_id
        {
            return Err(
                "Localized explicit favorite save changed content or source identity".into(),
            );
        }
        self.dialog.close();
        Ok(())
    }
}
