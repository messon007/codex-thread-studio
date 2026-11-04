//! Existing language controller drives these actual Comments widgets/prompts.
#![allow(deprecated)]
use super::*;
use std::{path::Path, time::Duration};
const QUOTE: &str = "Select text in a document and choose Comment to add it here.";
const NOTE: &str = "Saved comments <b>Source</b> 中文😀 {value}\n";
const INSTRUCTIONS: &str = "Keep comments and close / Saved comments";
pub struct LanguageProbe {
    source: Value,
    locations: Vec<(gtk::Label, String, String)>,
    prompt: RefCell<Option<adw::MessageDialog>>,
}

async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !test() {
        if start.elapsed() > Duration::from_secs(15) {
            return Err(format!("Comments language timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(25)).await;
    }
    Ok(())
}
fn prompt() -> Option<adw::MessageDialog> {
    gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|w| w.downcast::<adw::MessageDialog>().ok())
        .find(|w| {
            w.is_visible()
                && w.heading().as_deref() == Some(&crate::language::tr("Save comment changes?"))
        })
}

impl Panel {
    pub async fn smoke_language_prepare(&self) -> Result<LanguageProbe, String> {
        until("initial snapshot", || {
            !self.busy.get() && self.snapshot.borrow().is_some()
        })
        .await?;
        self.choose(None);
        let source = json!({"provider":"chat","version":1,"anchor":{"sessionKey":"codex:target","turnId":"Saved comments","itemId":"line {value}<b>Source</b>","future":{"literal":"Comments"}}});
        self.add(Capture {
            excerpt: QUOTE.into(),
            source: source.clone(),
        });
        self.note.buffer().set_text(NOTE);
        self.additional.buffer().set_text(INSTRUCTIONS);
        let samples = vec![
            (
                source.clone(),
                "session codex:target / turn Saved comments / item line {value}<b>Source</b>",
                "会话 codex:target / 轮次 Saved comments / 条目 line {value}<b>Source</b>",
            ),
            (
                json!({"provider":"document","anchor":{"filePath":"Source","startLine":2,"endLine":4,"baseHash":"Saved comments"}}),
                "Source / line 2–4 / base Saved comments",
                "Source / 第 2–4 行 / 基准 Saved comments",
            ),
            (
                json!({"provider":"table","anchor":{"filePath":"Source","sheet":"Saved comments","range":"B2:C4","sql":"SELECT 1"}}),
                "Source · Saved comments!B2:C4 · SQL result",
                "Source · Saved comments!B2:C4 · SQL 结果",
            ),
            (
                json!({"provider":"pdf","anchor":{"filePath":"Source","page":4,"rects":[{},{}]}}),
                "Source · Page 4 · 2 text regions",
                "Source · 第 4 页 · 2 个文字区域",
            ),
            (
                json!({"provider":"epub","anchor":{"filePath":"Source","chapterLabel":"Saved comments","cfiRange":"{value}<b>Source</b>","bookHash":"Comments"}}),
                "Source / chapter Saved comments / EPUB CFI {value}<b>Source</b> / base Comments",
                "Source / 章节 Saved comments / EPUB CFI {value}<b>Source</b> / 基准 Comments",
            ),
            (
                json!({"provider":"future","anchor":{"url":"Chat comment"}}),
                "Chat comment",
                "Chat comment",
            ),
        ];
        let locations = samples
            .into_iter()
            .map(|(source, en, zh)| {
                let label = gtk::Label::new(None);
                // Model assignment must retire an earlier interface/parts binding,
                // even when the new literal is itself a known interface key.
                Self::bind_location(&label, &json!({"provider":"chat","anchor":{}}));
                Self::bind_location(&label, &source);
                (label, en.into(), zh.into())
            })
            .collect();
        let probe = LanguageProbe {
            source,
            locations,
            prompt: RefCell::new(None),
        };
        self.smoke_language_check(&probe, false)?;
        Ok(probe)
    }

    pub fn smoke_language_check(&self, probe: &LanguageProbe, chinese: bool) -> Result<(), String> {
        let expected = |en, zh| if chinese { zh } else { en };
        if self.dialog.title() != expected("Comments", "批注")
            || self.save.label().as_deref() != Some(expected("Save", "保存"))
            || self.send.label().as_deref() != Some(expected("Send comments", "发送批注"))
            || self.insert.label().as_deref() != Some(expected("Insert into message", "插入消息"))
            || self.status.text() != expected("Unsaved comments", "未保存的批注")
            || self.excerpt.text() != QUOTE
            || text(&self.note.buffer()) != NOTE
            || text(&self.additional.buffer()) != INSTRUCTIONS
            || !self.dirty.get()
            || self.busy.get()
            || !self.save.is_sensitive()
        {
            return Err("Live Comments language lost labels, literal drafts or save state".into());
        }
        let snapshot = self.snapshot.borrow();
        if snapshot.as_ref().is_none_or(|s| {
            s.drafts.len() != 1
                || annotations::source(&s.drafts[0]) != probe.source
                || annotations::note(&s.drafts[0]) != NOTE
                || s.additional != INSTRUCTIONS
        }) {
            return Err("Language changed comment records or source extensions".into());
        }
        for (label, en, zh) in &probe.locations {
            let expected = if chinese { zh } else { en };
            if label.text() != expected.as_str() {
                return Err(format!("Comment location lost its locale/literal boundary: expected {expected:?}, actual {:?}",label.text()));
            }
        }
        if let Some(dialog) = probe.prompt.borrow().as_ref() {
            if dialog.heading().as_deref()
                != Some(expected("Save comment changes?", "保存批注更改？"))
                || dialog.body()
                    != expected(
                        "Unsaved comment edits will be lost if you discard them.",
                        "放弃后将丢失未保存的批注更改。",
                    )
                || dialog.default_response().as_deref() != Some("save")
                || dialog.close_response() != "cancel"
                || !dialog.is_response_enabled("save")
            {
                return Err("Open comment prompt lost live labels or response semantics".into());
            }
        }
        Ok(())
    }

    pub async fn smoke_language_prompt(
        self: &Rc<Self>,
        probe: &LanguageProbe,
    ) -> Result<(), String> {
        self.close_guard();
        until("mapped dirty prompt", || prompt().is_some()).await?;
        *probe.prompt.borrow_mut() = prompt();
        self.smoke_language_check(probe, true)
    }
    pub async fn smoke_language_finish(
        self: &Rc<Self>,
        probe: &LanguageProbe,
        root: &Path,
    ) -> Result<(), String> {
        self.smoke_language_check(probe, true)?;
        let dialog = probe
            .prompt
            .borrow()
            .clone()
            .ok_or("Comments language prompt missing")?;
        glib::timeout_future(Duration::from_millis(350)).await;
        crate::window_capture::capture(&dialog, &root.join("language-comments-prompt.png")).await?;
        dialog.response("cancel");
        until("cancelled dirty prompt", || !self.guard.get()).await?;
        self.save.emit_clicked();
        until("explicit save", || !self.busy.get() && !self.dirty.get()).await?;
        if self.status.text() != crate::language::tr("Saved comments") {
            return Err("Saved Comments feedback lost its language".into());
        }
        self.dialog.close();
        Ok(())
    }
}
