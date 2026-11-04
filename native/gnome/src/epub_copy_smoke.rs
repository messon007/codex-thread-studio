//! Debug-only clipboard acceptance on the fixture's owned Wayland compositor.
use super::*;
use std::{
    path::Path,
    time::{Duration, Instant},
};

async fn until(step: &str, check: impl Fn() -> bool) -> Result<(), String> {
    let start = Instant::now();
    while !check() {
        if start.elapsed() > Duration::from_secs(20) {
            return Err(format!("EPUB clipboard test timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}

use crate::smoke_input::request as marker;

impl Epub {
    async fn copy_inspect(&self, script: &str) -> Result<Value, String> {
        let web = self
            .surface
            .borrow()
            .clone()
            .ok_or("Missing EPUB surface")?;
        let result = web
            .evaluate_javascript_future(&format!("JSON.stringify({script})"), Some(WORLD), None)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::from_str(&result.to_str()).map_err(|e| e.to_string())
    }

    pub async fn smoke_clipboard(
        self: &Rc<Self>,
        root: &Path,
        capture: impl Fn(),
    ) -> Result<Value, String> {
        if std::env::var_os("STUDIO_OWNED_MUTTER_INPUT").is_none() {
            return Err("EPUB clipboard test requires its owned compositor".into());
        }
        until("initial book and focus", || {
            self.state.borrow().ready && self.parent.upgrade().is_some_and(|p| p.is_active())
        })
        .await?;
        let parent = self.parent.upgrade().ok_or("EPUB window disappeared")?;
        parent.fullscreen();
        until("fullscreen allocation", || {
            parent.is_fullscreen() && parent.width() == 1600 && parent.height() == 1000
        })
        .await?;
        if parent.scale_factor() != 1 {
            return Err("Fixture requires scale one".into());
        }
        glib::timeout_future(Duration::from_millis(400)).await;
        let geometry = self.copy_inspect("(()=>{const e=document.querySelector('#copy');e.scrollIntoView();const w=document.createTreeWalker(e,NodeFilter.SHOW_TEXT);let first=null,last=null;while(w.nextNode()){const n=w.currentNode;if(n.length){first ||= n;last=n}}const a=document.createRange(),b=document.createRange();a.setStart(first,0);a.setEnd(first,1);b.setStart(last,last.length-1);b.setEnd(last,last.length);const x=a.getBoundingClientRect(),y=b.getBoundingClientRect();return {text:e.textContent,x:x.left+.1,y:x.top+x.height/2,end:y.right-.1,endY:y.top+y.height/2}})()").await?;
        let web = self
            .surface
            .borrow()
            .clone()
            .ok_or("Missing EPUB surface")?;
        let start = web
            .compute_point(
                &parent,
                &gtk::graphene::Point::new(
                    geometry["x"].as_f64().unwrap() as f32,
                    geometry["y"].as_f64().unwrap() as f32,
                ),
            )
            .ok_or("EPUB surface unmapped")?;
        marker(root,json!({"action":"pointer","x":start.x(),"y":start.y(),"dx":geometry["end"].as_f64().unwrap()-geometry["x"].as_f64().unwrap(),"dy":geometry["endY"].as_f64().unwrap()-geometry["y"].as_f64().unwrap()})).await?;
        glib::timeout_future(Duration::from_millis(200)).await;
        let selected = self.copy_inspect("getSelection().toString()").await?;
        if selected != geometry["text"] {
            return Err(format!(
                "Real pointer selection differs: {selected}; expected {}",
                geometry["text"]
            ));
        }
        let copy = self
            .copy
            .compute_point(
                &parent,
                &gtk::graphene::Point::new(
                    self.copy.width() as f32 / 2.,
                    self.copy.height() as f32 / 2.,
                ),
            )
            .ok_or("Copy button unmapped")?;
        marker(root, json!({"action":"pointer","x":copy.x(),"y":copy.y()})).await?;
        until("copy completion", || {
            !self.copying.get() && self.status.text() == "Copied selected text"
        })
        .await?;
        let actual = marker(root, json!({"action":"read"})).await?;
        if actual["text"] != geometry["text"] {
            return Err(format!("Cross-process clipboard differs: {actual}"));
        }
        glib::timeout_future(Duration::from_millis(250)).await;
        let retained = marker(root, json!({"action":"read"})).await?;
        if retained["text"] != geometry["text"] {
            return Err("Clipboard ownership was lost after Copy".into());
        }
        capture();
        // WebKit's normal accelerator must coexist with GTK's toolbar owner.
        self.copy.clipboard().set_text("before-webkit-shortcut");
        web.grab_focus();
        marker(root, json!({"action":"key","key":"copy"})).await?;
        glib::timeout_future(Duration::from_millis(200)).await;
        if marker(root, json!({"action":"read"})).await?["text"] != geometry["text"] {
            return Err("WebKit Ctrl+C did not transfer the exact selection".into());
        }
        // Moving focus to the native toolbar may collapse live DOM selection;
        // retained ranges must also support keyboard activation.
        self.copy
            .clipboard()
            .set_text("before-native-keyboard-copy");
        if !self.copy.grab_focus() {
            return Err("Native Copy could not receive focus".into());
        }
        marker(root, json!({"action":"key","key":"space"})).await?;
        glib::timeout_future(Duration::from_millis(200)).await;
        until("keyboard copy", || !self.copying.get()).await?;
        if marker(root, json!({"action":"read"})).await?["text"] != geometry["text"] {
            return Err("Keyboard toolbar Copy lost its retained DOM selection".into());
        }
        // Deterministic late result: only the isolated test world delays DOM
        // work. Navigate within the same resource while GTK stays responsive.
        self.copy_inspect("(()=>{window.originalCopy=StudioReader.copySelection;window.copyCalls=0;StudioReader.copySelection=()=>{copyCalls++;const end=performance.now()+350;while(performance.now()<end){}return 'stale-copy-result'};return true})()").await?;
        self.copy_selection();
        if !self.copying.get() || self.copy.is_sensitive() {
            return Err("Copy remained enabled while pending".into());
        }
        self.copy_selection();
        glib::timeout_future(Duration::from_millis(40)).await;
        let stale_status = Rc::new(Cell::new(false));
        let observed = stale_status.clone();
        let handler = self.status.connect_label_notify(move |label| {
            if label.text() == "Copied selected text" || label.text().starts_with("Unable to copy:")
            {
                observed.set(true);
            }
        });
        let resource = self.state.borrow().resource.clone();
        self.navigate(&format!("{resource}#copy"), "", false);
        self.status.set_text("Navigated while Copy was pending");
        glib::timeout_future(Duration::from_millis(450)).await;
        self.status.disconnect(handler);
        if self.copying.get() || !self.copy.is_sensitive() || stale_status.get() {
            return Err(format!("Old Copy completion affected the new section view: copying={}, sensitive={}, status={:?}, ready={}, epoch={}",self.copying.get(),self.copy.is_sensitive(),self.status.text(),self.state.borrow().ready,self.state.borrow().document_epoch));
        }
        if self.copy_inspect("copyCalls").await? != 1 {
            return Err("Pending Copy admitted a duplicate operation".into());
        }
        if marker(root, json!({"action":"read"})).await?["text"] != geometry["text"] {
            return Err("A stale Copy overwrote the compositor clipboard".into());
        }
        self.copy_inspect("(()=>{StudioReader.copySelection=originalCopy;return true})()")
            .await?;
        // Source length is UTF-16; the native transport limit is UTF-8 bytes.
        // Exercise the actual DOM helper with >2 MiB UTF-8 but <2M characters.
        self.copy_inspect("(()=>{const e=document.querySelector('#copy');e.textContent='中'.repeat(700000);const r=document.createRange();r.selectNodeContents(e);getSelection().removeAllRanges();getSelection().addRange(r);return r.toString().length})()").await?;
        self.copy_selection();
        until("UTF-8 bound", || !self.copying.get()).await?;
        if self.status.text() != "Select a smaller passage to copy" || !self.copy.is_sensitive() {
            return Err("Oversized Unicode Copy was not rejected cleanly".into());
        }
        if marker(root, json!({"action":"read"})).await?["text"] != geometry["text"] {
            return Err("Rejected Copy overwrote the clipboard".into());
        }
        self.copy_inspect("(()=>{const e=document.querySelector('#copy');e.textContent='   ';const r=document.createRange();r.selectNodeContents(e);getSelection().removeAllRanges();getSelection().addRange(r);return true})()").await?;
        self.copy_selection();
        until("whitespace Copy", || !self.copying.get()).await?;
        if marker(root, json!({"action":"read"})).await?["text"] != "   " {
            return Err("Copy trimmed whitespace-only selection".into());
        }
        // Closing while an asynchronous result is pending must not resurrect it.
        self.copy_inspect("(()=>{StudioReader.copySelection=()=>{const end=performance.now()+250;while(performance.now()<end){}return 'closed-copy-result'};return true})()").await?;
        self.copy_selection();
        self.close();
        glib::timeout_future(Duration::from_millis(350)).await;
        if self.copying.get()
            || self.copy.is_sensitive()
            || marker(root, json!({"action":"read"})).await?["text"] != "   "
        {
            return Err("Closing the book retained an active Copy operation".into());
        }
        Ok(
            json!({"ok":true,"realPointerSelection":true,"toolbarCopy":true,"keyboardCopy":true,"webkitShortcut":true,"independentClipboardRead":true,"unicodeAndTrailingSpace":true,"whitespaceOnly":true,"utf8Bound":true,"sameResourceStaleResult":true,"pendingAdmission":true,"closeIsolation":true,"selection":geometry["text"]}),
        )
    }
}
