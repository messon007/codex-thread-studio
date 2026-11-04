//! Real Source selection/button semantics; immutable fake-file snapshots only.
use super::*;
use serde_json::{json, Value};
impl Source {
    pub fn smoke_status_language(&self) -> Result<(), String> {
        let original = self
            .text
            .borrow()
            .as_ref()
            .ok_or("No source snapshot")?
            .clone();
        for flags in 0..8 {
            let mut text = original.clone();
            // A file name equal to an interface key must remain literal even
            // when it replaces that same visible, registered interface text.
            crate::i18n_ui::bind(&self.title, "label", "Source");
            text.path = "Source".into();
            text.binary = flags & 1 != 0;
            text.truncated = flags & 2 != 0;
            if flags & 4 != 0 {
                let mut index =
                    studio_markdown_native::unified_diff::Index::new("@@ -1 +1 @@\n-old\n+new\n");
                index.limited = true;
                text.diff = Some(index);
            }
            self.show_prepared(text);
            let buffer = self.view.buffer();
            buffer.select_range(&buffer.iter_at_offset(3), &buffer.iter_at_offset(9));
            let before = self.visible_text();
            let window = self.window.borrow().as_ref().unwrap().clone();
            for (locale, details) in [
                (
                    "zh-CN",
                    ["二进制内容", "Git 输出已截断", "差异行号映射受限"],
                ),
                (
                    "en-US",
                    [
                        "Binary content",
                        "Truncated by Git",
                        "Diff line mapping limited",
                    ],
                ),
            ] {
                crate::language::set(locale, "en-US")?;
                crate::i18n_ui::apply();
                let mut expected = if locale == "zh-CN" {
                    format!(
                        "{} 行 · 第 {}–{} 行",
                        original.document.lines(),
                        window.line,
                        window.end_line
                    )
                } else {
                    format!(
                        "{} lines · Lines {}–{}",
                        original.document.lines(),
                        window.line,
                        window.end_line
                    )
                };
                for (bit, detail) in details.iter().enumerate() {
                    if flags & (1 << bit) != 0 {
                        expected.push_str(" · ");
                        expected.push_str(detail);
                    }
                }
                if self.status.text() != expected
                    || self.title.text() != "Source"
                    || self.visible_text() != before
                    || buffer
                        .selection_bounds()
                        .map(|(a, b)| (a.offset(), b.offset()))
                        != Some((3, 9))
                    || self.window.borrow().as_ref().map(|w| (w.start, w.end))
                        != Some((window.start, window.end))
                {
                    return Err(format!("Source status translation changed content/selection or missed flags {flags}: {}", self.status.text()));
                }
            }
        }
        // Literal errors can equal an earlier UI summary. Register removal must
        // still prevent a locale change from treating that value as UI text.
        let literal = self.status.text().to_string();
        crate::i18n_ui::literal(&self.status, "label", &literal);
        crate::language::set("zh-CN", "en-US")?;
        crate::i18n_ui::apply();
        if self.status.text() != literal {
            return Err("Source literal status was translated".into());
        }
        crate::language::set("en-US", "en-US")?;
        crate::i18n_ui::apply();
        self.show_prepared(original.clone());
        self.clear();
        crate::language::set("zh-CN", "en-US")?;
        crate::i18n_ui::apply();
        if !self.status.text().is_empty() || !self.title.text().is_empty() {
            return Err("Cleared source labels returned after language change".into());
        }
        crate::language::set("en-US", "en-US")?;
        crate::i18n_ui::apply();
        self.show_prepared(original);
        Ok(())
    }
    pub fn smoke_error(&self, expected: &str) -> bool {
        self.text.borrow().is_none()
            && self.status.text().contains(expected)
            && !self.comment.is_sensitive()
    }
    pub fn smoke_ready(&self) -> bool {
        self.text.borrow().is_some() && self.window.borrow().is_some()
    }
    pub fn smoke_first_line_visible(&self) -> bool {
        let buffer = self.view.buffer();
        let first = self.view.iter_location(&buffer.start_iter());
        let visible = self.view.visible_rect();
        buffer.iter_at_mark(&buffer.get_insert()).offset() == 0
            && first.y() >= visible.y()
            && first.y() + first.height() <= visible.y() + visible.height()
    }
    pub fn smoke_comment_location(&self) -> (usize, usize, bool) {
        let window = self.window.borrow();
        let window = window.as_ref().unwrap();
        (window.line, window.start, self.notice.is_visible())
    }
    pub fn smoke_select(&self, excerpt: &str) -> Result<(), String> {
        let text = self.visible_text();
        let start = text
            .find(excerpt)
            .ok_or("Source quote is not in the mounted window")?;
        let a = text[..start].chars().count() as i32;
        let b = a + excerpt.chars().count() as i32;
        let buffer = self.view.buffer();
        buffer.select_range(&buffer.iter_at_offset(a), &buffer.iter_at_offset(b));
        Ok(())
    }
    pub fn smoke_capture(&self, excerpt: &str) -> Result<crate::annotations::Capture, String> {
        self.smoke_select(excerpt)?;
        if !self.comment.is_sensitive() {
            return Err("Source comment button did not follow selection".into());
        }
        let capture = self
            .capture()
            .ok_or("Source selection could not be captured")?;
        self.comment.emit_clicked();
        Ok(capture)
    }
    pub fn smoke_reject_non_file(&self) -> Result<Value, String> {
        let original = self
            .text
            .borrow()
            .as_ref()
            .ok_or("No source snapshot")?
            .clone();
        for flag in 0..4 {
            let mut text = original.clone();
            match flag {
                0 => text.file = None,
                1 => text.binary = true,
                2 => text.truncated = true,
                _ => {
                    text.diff = Some(studio_markdown_native::unified_diff::Index::new(
                        "@@ -1 +1 @@\n-old\n+new\n",
                    ))
                }
            }
            self.show_prepared(text);
            let buffer = self.view.buffer();
            buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
            if self.capture().is_some() || self.comment.is_sensitive() {
                return Err("Non-file data acquired a source comment".into());
            }
        }
        self.show_prepared(original);
        let selected = self.visible_text().chars().take(6).collect::<String>();
        self.smoke_select(&selected)?;
        self.widget.set_visible(false);
        if self.capture().is_some() {
            return Err("Hidden Source captured stale text".into());
        }
        self.widget.set_visible(true);
        self.clear();
        if self.capture().is_some() || self.comment.is_sensitive() {
            return Err("Closed source retained a comment target".into());
        }
        Ok(
            json!({"binary":true,"truncated":true,"diff":true,"missingIdentity":true,"hidden":true,"closed":true}),
        )
    }
}
