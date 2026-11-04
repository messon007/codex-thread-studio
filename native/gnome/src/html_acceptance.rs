//! Actual file actor and native controls, on copied/generated private files.
use super::*;
use gtk::glib;
use std::{
    path::Path,
    time::{Duration, Instant},
};
use webkit6::prelude::*;
impl Source {
    /// Select actual rendered DOM text, then exercise the native Comment button.
    pub async fn smoke_html_capture(
        &self,
        project: &str,
        paragraph: usize,
        expected: &str,
        comment: bool,
    ) -> Result<crate::annotations::Capture, String> {
        self.open(project, "actions.html");
        crate::acceptance_wait::until("HTML action document", || {
            self.html_active() && self.editor.html.ready.get()
        })
        .await?;
        let web = self.editor.html.smoke_web().ok_or("HTML surface absent")?;
        let script = format!("(()=>{{const p=document.querySelectorAll('p')[{paragraph}];const r=document.createRange();r.selectNodeContents(p);const s=getSelection();s.removeAllRanges();s.addRange(r);return String(s)}})()");
        let selected = web
            .evaluate_javascript_future(&script, Some("studio-static-html-preview"), None)
            .await
            .map_err(|e| e.to_string())?
            .to_str()
            .to_string();
        if selected != expected {
            return Err(format!("HTML rendered quote changed: {selected:?}"));
        }
        crate::acceptance_wait::until("HTML action selection", || {
            self.editor.html.comment_selection().as_deref() == Some(expected)
                && self.comment.is_sensitive()
        })
        .await?;
        let capture = self.capture().ok_or("HTML Comment capture absent")?;
        if comment {
            self.comment.emit_clicked();
        }
        Ok(capture)
    }
    pub async fn smoke_html_translate(&self, root: &Path, menu: bool) -> Result<(), String> {
        let window = self
            .widget
            .root()
            .and_downcast::<adw::ApplicationWindow>()
            .ok_or("HTML native window missing")?;
        crate::acceptance_wait::until("HTML comment dialog dismissed", || {
            window.visible_dialog().is_none()
        })
        .await?;
        crate::smoke_input::request(root, serde_json::json!({"action":"focus"})).await?;
        self.editor.html.focus();
        let web = self
            .editor
            .html
            .smoke_web()
            .ok_or("HTML native content missing")?;
        crate::acceptance_wait::until("HTML content keyboard focus", || {
            gtk::prelude::GtkWindowExt::focus(&window)
                .is_some_and(|f| f == web || f.is_ancestor(&web))
        })
        .await?;
        if menu {
            window.fullscreen();
            crate::acceptance_wait::until("HTML owned fullscreen monitor", || {
                window.is_fullscreen() && window.width() == 1600 && window.height() == 1000
            })
            .await?;
            crate::window_capture::ready(&web).await?;
            let bounds = web
                .compute_bounds(&window)
                .ok_or("HTML pointer bounds missing")?;
            let point = web.evaluate_javascript_future("(()=>{const p=document.querySelectorAll('p')[1];const r=p.getBoundingClientRect();return JSON.stringify({x:r.x+8,y:r.y+r.height/2})})()",Some("studio-static-html-preview"),None).await.map_err(|e|e.to_string())?;
            let point: serde_json::Value =
                serde_json::from_str(&point.to_str()).map_err(|e| e.to_string())?;
            let x = bounds.x() as f64 + point["x"].as_f64().ok_or("HTML pointer X missing")?;
            let y = bounds.y() as f64 + point["y"].as_f64().ok_or("HTML pointer Y missing")?;
            let picked = window
                .pick(x, y, gtk::PickFlags::DEFAULT)
                .ok_or("HTML pointer target absent")?;
            if picked != web && !picked.is_ancestor(&web) {
                return Err(format!(
                    "HTML pointer target is {} at {x},{y}",
                    picked.type_().name()
                ));
            }
            crate::window_capture::capture(&window, &root.join("html-menu-before-pointer.png"))
                .await?;
            crate::smoke_input::request(
                root,
                serde_json::json!({"action":"pointer","button":273,"x":x,"y":y}),
            )
            .await?;
            crate::window_capture::capture(&window, &root.join("html-menu-after-pointer.png"))
                .await?;
            crate::acceptance_wait::until("HTML real context menu shown", || {
                self.editor.html.smoke_context().is_some()
            })
            .await?;
            crate::window_capture::capture(&window, &root.join("html-context-menu.png")).await?;
            fn find(widget: &gtk::Widget) -> Option<gtk::Button> {
                if let Some(b) = widget.downcast_ref::<gtk::Button>().filter(|b| {
                    b.action_name().as_deref() == Some("translation.translate") && b.is_sensitive()
                }) {
                    return Some(b.clone());
                }
                let mut child = widget.first_child();
                while let Some(w) = child {
                    if let Some(b) = find(&w) {
                        return Some(b);
                    }
                    child = w.next_sibling();
                }
                None
            }
            let popup = self
                .editor
                .html
                .smoke_context()
                .ok_or("HTML native context menu missing")?;
            let (anchored, anchor) = popup.pointing_to();
            let expected_x = point["x"].as_f64().ok_or("HTML local pointer X missing")?;
            let expected_y = point["y"].as_f64().ok_or("HTML local pointer Y missing")?;
            if !anchored
                || (anchor.x() as f64 - expected_x).abs() > 2.0
                || (anchor.y() as f64 - expected_y).abs() > 2.0
            {
                return Err(format!(
                    "HTML menu anchor {:?} differs from local pointer {expected_x},{expected_y}",
                    anchor
                ));
            }
            find(popup.upcast_ref())
                .ok_or("HTML native Translate button missing")?
                .emit_clicked();
        } else {
            crate::smoke_input::request(
                root,
                serde_json::json!({"action":"key","key":"translate"}),
            )
            .await?;
        }
        Ok(())
    }
    pub async fn smoke_html_clear_selection(&self) -> Result<(), String> {
        let web = self.editor.html.smoke_web().ok_or("HTML surface absent")?;
        web.evaluate_javascript_future(
            "getSelection().removeAllRanges()",
            Some("studio-static-html-preview"),
            None,
        )
        .await
        .map_err(|e| e.to_string())?;
        crate::acceptance_wait::until("HTML empty action disabled", || {
            self.editor.html.selection().is_none()
        })
        .await
    }
    pub fn smoke_html_reopened(&self, warning: bool, literal: Option<&str>) -> bool {
        !self.reading.get()
            && self.smoke_ready()
            && !self.preview_active()
            && !self.editor.editing.get()
            && self.notice.is_visible() == warning
            && literal.is_none_or(|s| self.visible_text().starts_with(s))
    }
    pub async fn smoke_html(&self, root: &Path) -> Result<serde_json::Value, String> {
        async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
            crate::acceptance_wait::until(step, test).await
        }
        async fn read(path: std::path::PathBuf) -> Result<Vec<u8>, String> {
            gtk::gio::spawn_blocking(move || std::fs::read(path).map_err(|e| e.to_string()))
                .await
                .map_err(|_| "HTML fixture read worker stopped".to_owned())?
        }
        async fn data(
            source: &Source,
            script: &str,
            world: Option<&str>,
        ) -> Result<serde_json::Value, String> {
            let web = source
                .editor
                .html
                .smoke_web()
                .ok_or("HTML content was not created")?;
            let value = web
                .evaluate_javascript_future(script, world, None)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::from_str(&value.to_str()).map_err(|e| e.to_string())
        }
        async fn shot(source: &Source, root: &Path, name: &str) -> Result<(), String> {
            let window = source
                .widget
                .root()
                .and_downcast::<gtk::Window>()
                .ok_or("No native window")?;
            crate::window_capture::capture(&window, &root.join(name)).await
        }
        let project = root.join("project");
        let project = project.to_str().unwrap();
        let original = read(root.join("project/passive.html")).await?;
        let started = Instant::now();
        self.open(project, "passive.html");
        until("native HTML Preview", || {
            self.html_active() && self.editor.html.ready.get()
        })
        .await?;
        let opening_ms = started.elapsed().as_millis();
        let baseline = data(self, "JSON.stringify({text:document.body.innerText,pwned:typeof globalThis.__studio_attack,isolated:typeof globalThis.StudioHtml,bad:!!document.body.querySelector('script,style,img,iframe,object,embed,form,input,svg,math,canvas'),attrs:[...document.body.querySelectorAll('*')].flatMap(n=>[...n.attributes].map(a=>a.name)).filter(n=>/^on/i.test(n)||['style','src','href','srcset','srcdoc','action'].includes(n))})", None).await?;
        if baseline["pwned"] != "undefined"
            || baseline["isolated"] != "undefined"
            || baseline["bad"] != false
            || baseline["attrs"] != serde_json::json!([])
        {
            return Err(format!(
                "HTML passive/isolation boundary failed: {baseline}"
            ));
        }
        if !baseline["text"].as_str().is_some_and(|s| {
            s.contains("Original title")
                && s.contains("Table value 中文😀")
                && !s.contains("Attack secret")
                && !s.contains("Fixture")
        }) {
            return Err(format!("HTML readable content changed: {baseline}"));
        }
        shot(self, root, "html-preview.png").await?;
        let canary = String::from_utf8(read(root.join("canary-url.txt")).await?)
            .map_err(|e| e.to_string())?;
        let probe = format!("globalThis.__studio_csp_probe='pending';fetch({}).then(()=>globalThis.__studio_csp_probe='allowed',()=>globalThis.__studio_csp_probe='denied');JSON.stringify(true)", serde_json::to_string(&canary).unwrap());
        data(self, &probe, None).await?;
        glib::timeout_future(Duration::from_millis(100)).await;
        if data(self, "JSON.stringify(globalThis.__studio_csp_probe)", None).await? != "denied" {
            return Err("HTML CSP did not independently deny resource requests".into());
        }
        let selected = data(self, "(()=>{const p=document.querySelector('pre');const r=document.createRange();r.selectNodeContents(p);const s=getSelection();s.removeAllRanges();s.addRange(r);return JSON.stringify(String(s))})()", Some("studio-static-html-preview")).await?;
        until("HTML selection captured", || {
            self.editor.html.comment_selection().is_some()
        })
        .await?;
        if self.editor.html.selection().as_deref() != selected.as_str() {
            return Err("HTML selection cache changed the quote".into());
        }
        crate::smoke_input::request(root, serde_json::json!({"action":"focus"})).await?;
        self.editor.html.focus();
        crate::smoke_input::request(root, serde_json::json!({"action":"key","key":"copy"})).await?;
        let copied =
            crate::smoke_input::request(root, serde_json::json!({"action":"read"})).await?;
        if copied["text"] != selected {
            return Err(format!(
                "Native HTML Copy changed selection: {copied} vs {selected}"
            ));
        }
        let comment = self
            .capture()
            .ok_or("HTML selection had no Comment capture")?;
        if comment.excerpt != selected.as_str().unwrap()
            || comment.source["anchor"]["textKind"] != "preview"
            || !comment.source["anchor"]["startLine"].is_null()
        {
            return Err("HTML Comment invented source coordinates".into());
        }
        self.smoke_find_query("Keyword");
        until("native HTML Find", || self.smoke_find_count().contains('2')).await?;
        self.smoke_find_query("missing query");
        until("native HTML Find empty", || {
            self.smoke_find_count().contains('0')
        })
        .await?;
        self.smoke_find_close();
        fn activate_outline(source: &Source, index: u32) -> Result<(), String> {
            source.editor.html.contents.popup();
            let list = source
                .editor
                .html
                .contents
                .popover()
                .and_then(|p| p.child())
                .and_downcast::<gtk::ScrolledWindow>()
                .and_then(|s| s.child())
                .and_downcast::<gtk::ListView>()
                .ok_or("Native HTML outline list missing")?;
            list.emit_by_name::<()>("activate", &[&index]);
            Ok(())
        }
        activate_outline(self, 1)?;
        let original_target = self
            .editor
            .html
            .heading_original(1)
            .ok_or("Missing mapped second heading")?;
        until("HTML outline position", || {
            self.editor.html.original() == Some(original_target)
        })
        .await?;
        self.editor.source.set_active(true);
        if self.preview_active() || !self.visible_text().contains("<html") {
            return Err("HTML Source did not retain original tags".into());
        }
        activate_outline(self, 1)?;
        let target = self
            .editor
            .html
            .heading_original(1)
            .ok_or("HTML heading lost original byte mapping")?;
        if self.window.borrow().as_ref().map(|w| w.start) != Some(target) {
            return Err("Source outline target is inaccurate".into());
        }
        // Return to the original first page before editing; edits/save preserve
        // CRLF and never serialize sanitized output over the original HTML.
        self.show(1, None, false);
        self.editor.edit.set_active(true);
        until("HTML edit page", || {
            self.editor.page.borrow().is_some() && !self.editor.busy.get()
        })
        .await?;
        self.edit_insert("<h1>Saved addition 中文😀</h1>\n");
        self.editor.preview.button.set_active(true);
        if read(root.join("project/passive.html")).await? != original {
            return Err("HTML view switch wrote original file".into());
        }
        until("retained HTML reading snapshot", || {
            self.html_active() && self.editor.html.ready.get()
        })
        .await?;
        let old = data(
            self,
            "JSON.stringify(document.body.innerText.includes('Saved addition'))",
            None,
        )
        .await?;
        if old != false {
            return Err("Dirty HTML replaced its reading snapshot before Save".into());
        }
        self.editor.save.emit_clicked();
        until("HTML original Save", || {
            !self.editor.busy.get() && !self.editor.dirty.get() && self.editor.html.ready.get()
        })
        .await?;
        let saved = read(root.join("project/passive.html")).await?;
        if saved
            != [
                "<h1>Saved addition 中文😀</h1>\r\n".as_bytes(),
                original.as_slice(),
            ]
            .concat()
        {
            return Err("HTML save changed original spellings or CRLF".into());
        }
        if data(
            self,
            "JSON.stringify(document.body.innerText.includes('Saved addition'))",
            None,
        )
        .await?
            != true
        {
            return Err("HTML save did not refresh Preview".into());
        }
        self.editor
            .html
            .smoke_web()
            .unwrap()
            .terminate_web_process();
        until("owned HTML renderer stopped", || {
            !self.editor.html.ready.get()
        })
        .await?;
        self.editor.source.set_active(true);
        self.editor.preview.button.set_active(true);
        until("HTML renderer recreated on explicit view return", || {
            self.editor.html.ready.get()
        })
        .await?;
        if data(
            self,
            "JSON.stringify(document.body.innerText.includes('Saved addition'))",
            None,
        )
        .await?
            != true
        {
            return Err("HTML renderer recovery lost the saved document".into());
        }
        // Native outline navigation owns a stable heading target across the
        // save guard, even when editing inserts an earlier heading.
        self.editor.source.set_active(true);
        self.show(1, None, false);
        self.editor.edit.set_active(true);
        until("second HTML edit page", || {
            !self.editor.busy.get() && self.editor.page.borrow().is_some()
        })
        .await?;
        self.edit_insert("<h1>Inserted before outline</h1>\n");
        self.editor.source.set_active(true);
        activate_outline(self, 2)?;
        if self.editor.guard.borrow().is_none() {
            return Err("Dirty outline navigation missed the save guard".into());
        }
        self.edit_response("save").await?;
        until("saved outline navigation", || {
            !self.editor.busy.get() && !self.editor.dirty.get()
        })
        .await?;
        let target = self
            .text
            .borrow()
            .as_ref()
            .and_then(|t| t.html.as_ref())
            .and_then(|h| h.headings.iter().find(|h| h.label == "Second chapter"))
            .and_then(|h| h.original)
            .ok_or("Saved target missing")?;
        if self.window.borrow().as_ref().map(|w| w.start) != Some(target) {
            return Err("Save guard jumped to an obsolete heading ordinal".into());
        }
        self.editor.preview.button.set_active(true);
        until("HTML after guarded outline save", || {
            self.editor.html.ready.get()
        })
        .await?;
        crate::language::set("zh-CN", "en-US")?;
        crate::i18n_ui::apply();
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
        glib::timeout_future(Duration::from_millis(300)).await;
        let dark = data(
            self,
            "JSON.stringify(document.documentElement.dataset.theme)",
            None,
        )
        .await?;
        if dark != "dark" || self.editor.preview.button.label().as_deref() != Some("预览") {
            return Err("HTML live language/theme failed".into());
        }
        shot(self, root, "html-dark.png").await?;
        crate::language::set("en-US", "en-US")?;
        crate::i18n_ui::apply();
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceLight);
        let shared = root.join("readonly/shared.html");
        self.open(project, shared.to_str().unwrap());
        until("read-only HTML Preview", || {
            self.html_active()
                && self.editor.html.ready.get()
                && self
                    .text
                    .borrow()
                    .as_ref()
                    .is_some_and(|t| t.path.ends_with("shared.html"))
        })
        .await?;
        if self.writable_source() || self.editor.edit.is_visible() || self.editor.save.is_visible()
        {
            return Err("Read-only HTML exposed editing".into());
        }
        self.open(project, "large-heading.html");
        until("bounded long HTML heading", || {
            self.html_active()
                && self.editor.html.ready.get()
                && self
                    .text
                    .borrow()
                    .as_ref()
                    .is_some_and(|t| t.path.ends_with("large-heading.html"))
        })
        .await?;
        if self
            .text
            .borrow()
            .as_ref()
            .and_then(|t| t.html.as_ref())
            .is_none_or(|h| {
                h.headings.len() != 1
                    || h.headings[0].label.len() > 4099
                    || h.headings[0].original.is_some()
            })
        {
            return Err("Long HTML heading escaped native outline bounds or invented a verified source mapping".into());
        }
        self.editor.html.contents.popup();
        glib::timeout_future(Duration::from_millis(100)).await;
        self.editor.html.contents.popdown();
        self.open(project, "oversize.html");
        until("bounded HTML Preview error", || {
            self.html_active()
                && self
                    .text
                    .borrow()
                    .as_ref()
                    .is_some_and(|t| t.path.ends_with("oversize.html"))
        })
        .await?;
        if self.editor.html.smoke_web().is_some() {
            return Err("Oversize HTML started a renderer".into());
        }
        self.editor.source.set_active(true);
        if self
            .window
            .borrow()
            .as_ref()
            .is_none_or(|w| w.text.len() > studio_markdown_native::text::WINDOW_BYTES + 2)
        {
            return Err("Oversize HTML Source is not bounded".into());
        }
        self.open(project, "passive.html");
        self.open(project, "other.rs");
        until("stale HTML open ignored", || {
            self.text
                .borrow()
                .as_ref()
                .is_some_and(|t| t.path.ends_with("other.rs"))
        })
        .await?;
        if self.editor.html.smoke_web().is_some() {
            return Err("Stale HTML renderer survived file replacement".into());
        }
        self.close.emit_clicked();
        Ok(
            serde_json::json!({"ok":true,"openingMs":opening_ms,"passiveMarkup":true,"isolatedWorld":true,"independentCspProbe":true,"nativeKeyboardCopy":true,"nativeFind":true,"nativeOutlineActivation":true,"guardedOutlineSave":true,"outlineSourceBytes":true,"rendererRecovery":true,"readOnly":true,"largeHeadingBounded":true,"sourceEditPreview":true,"dirtySnapshot":true,"exactOriginalSave":true,"liveLanguageTheme":true,"oversizeSource":true,"staleDocument":true,"commentQuoteWithoutInventedLines":true}),
        )
    }
}
