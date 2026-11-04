//! Real page file inputs and downloads through the fixture's private GTK portal.
use super::*;
use serde_json::json;
use std::{
    path::Path,
    time::{Duration, Instant},
};

async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let at = Instant::now();
    while !test() {
        if at.elapsed() > Duration::from_secs(20) {
            return Err(format!("Browser files timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
async fn inspect(web: &webkit6::WebView, script: &str) -> Result<Value, String> {
    let value = web
        .evaluate_javascript_future(&format!("JSON.stringify({script})"), Some(WORLD), None)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&value.to_str()).map_err(|e| e.to_string())
}
async fn loaded(web: &webkit6::WebView) -> Result<(), String> {
    until("page loaded", || {
        !web.is_loading() && web.title().as_deref() == Some("Browser file portal fixture")
    })
    .await
}
async fn click(
    browser: &Browser,
    root: &Path,
    web: &webkit6::WebView,
    selector: &str,
) -> Result<(), String> {
    let parent = browser.parent.upgrade().ok_or("Missing browser parent")?;
    if !parent.is_active() {
        crate::smoke_input::request(root, json!({"action":"focus"})).await?;
    }
    until("active page allocation", || {
        parent.is_active()
            && browser.active().as_ref() == Some(web)
            && web.is_mapped()
            && web.width() > 0
            && web.height() > 0
    })
    .await?;
    if !parent.is_fullscreen() || parent.width() != 1600 || parent.height() != 1000 {
        return Err("Browser file input requires its owned fullscreen monitor".into());
    }
    web.grab_focus();
    glib::timeout_future(Duration::from_millis(400)).await;
    let selector = serde_json::to_string(selector).unwrap();
    let at = Instant::now();
    let rect = loop {
        let rect=inspect(web,&format!("(() => {{const el=document.querySelector({selector});el.scrollIntoView();const r=el.getBoundingClientRect();return {{x:r.x+Math.min(r.width/2,50),y:r.y+r.height/2,w:r.width,h:r.height}};}})()")).await?;
        if rect["w"].as_f64().unwrap_or(0.0) > 0.0 && rect["h"].as_f64().unwrap_or(0.0) > 0.0 {
            break rect;
        }
        if at.elapsed() > Duration::from_secs(15) {
            let state=inspect(web,"({ready:document.readyState,hidden:document.hidden,width:innerWidth,height:innerHeight,bodyDisplay:getComputedStyle(document.body).display,rootDisplay:getComputedStyle(document.documentElement).display,inputDisplay:getComputedStyle(document.querySelector('#single')).display,bodyRect:document.body.getBoundingClientRect().toJSON(),viewport:visualViewport?{w:visualViewport.width,h:visualViewport.height}:null})").await?;
            return Err(format!("Page input has no rendered bounds: {rect}; state={state}; GTK={}x{} mapped={} visible={}",web.width(),web.height(),web.is_mapped(),web.is_visible()));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    };
    inspect(web,&format!("(() => {{const el=document.querySelector({selector});if(el.type==='file'){{el.value='';document.body.dataset.uploads='null';}}return true;}})()")).await?;
    let bounds = web
        .compute_bounds(&parent)
        .ok_or("Browser page has no allocation")?;
    crate::smoke_input::request(root, json!({"action":"pointer","x":bounds.x() as f64+rect["x"].as_f64().unwrap(),"y":bounds.y() as f64+rect["y"].as_f64().unwrap()})).await?;
    let state = inspect(
        web,
        "({active:document.activeElement.id,click:document.body.dataset.click||null,pointer:document.body.dataset.pointer||null,scale:devicePixelRatio})",
    )
    .await?;
    let data = json!({"selector":selector,"css":rect,"bounds":[bounds.x(),bounds.y(),bounds.width(),bounds.height()],"state":state,"activeWindow":parent.is_active(),"pick":parent.pick(bounds.x() as f64+rect["x"].as_f64().unwrap(),bounds.y() as f64+rect["y"].as_f64().unwrap(),gtk::PickFlags::DEFAULT).map(|w|w.type_().name().to_string()),"sensitive":web.is_sensitive(),"target":web.can_target()});
    let path = root.join("browser-click.json");
    gio::spawn_blocking(move || std::fs::write(path, serde_json::to_vec_pretty(&data).unwrap()))
        .await
        .map_err(|_| "Click trace stopped")?
        .map_err(|e| e.to_string())?;
    Ok(())
}
async fn uploads(web: &webkit6::WebView, count: usize) -> Result<Value, String> {
    let at = Instant::now();
    loop {
        let value = inspect(web, "JSON.parse(document.body.dataset.uploads||'null')").await?;
        if value.as_array().is_some_and(|v| v.len() == count) {
            return Ok(value);
        }
        if at.elapsed() > Duration::from_secs(20) {
            return Err(format!("Browser upload did not arrive: {value}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
}
impl Browser {
    pub(crate) async fn smoke_files(
        self: &Rc<Self>,
        root: &Path,
        url: &str,
        capture: impl Fn(&str),
    ) -> Result<Value, String> {
        let parent = self.parent.upgrade().ok_or("Missing parent")?;
        parent.fullscreen();
        until("fullscreen", || {
            parent.is_fullscreen() && parent.width() == 1600 && parent.height() == 1000
        })
        .await?;
        self.open(Some(url));
        until("engine", || self.active().is_some()).await?;
        let web = self.active().unwrap();
        loaded(&web).await?;
        self.add_tab(Some(url.replace("/one", "/two")));
        let preflight = self.active().ok_or("No preflight tab")?;
        loaded(&preflight).await?;
        glib::timeout_future(Duration::from_millis(700)).await;
        let viewport = inspect(&preflight, "({w:innerWidth,h:innerHeight})").await?;
        if viewport["w"].as_u64().unwrap_or(0) == 0 {
            return Err(format!("Second tab has no rendered viewport: {viewport}"));
        }
        self.tabs
            .close_page(&self.page_for(&preflight).ok_or("Missing preflight tab")?);
        click(self, root, &web, "#single").await?;
        capture("browser-first-input.png");
        crate::smoke_portal::wait(root, "calls", 1).await?;
        until("upload Cancel", || self.file_dialogs.borrow().is_empty()).await?;
        if inspect(&web, "document.querySelector('#single').files.length").await? != 0 {
            return Err("Canceled upload selected a file".into());
        }
        click(self, root, &web, "#single").await?;
        crate::smoke_portal::wait(root, "calls", 2).await?;
        let single = uploads(&web, 1).await?;
        if single != json!([{"name":"upload 中文 #.txt","text":"Explicit upload 中文😀\r\n"}])
        {
            return Err(format!("Selected upload bytes differ: {single}"));
        }
        click(self, root, &web, "#multiple").await?;
        crate::smoke_portal::wait(root, "calls", 3).await?;
        let multiple = uploads(&web, 2).await?;
        if multiple
            != json!([{"name":"upload 中文 #.txt","text":"Explicit upload 中文😀\r\n"},{"name":"second.txt","text":"Second explicit upload\n"}])
        {
            return Err(format!("Multiple upload bytes differ: {multiple}"));
        }
        for call in 4..=5 {
            click(self, root, &web, "#single").await?;
            crate::smoke_portal::wait(root, "calls", call).await?;
            until("invalid upload rejected", || {
                self.file_dialogs.borrow().is_empty()
            })
            .await?;
            if inspect(&web, "document.querySelector('#single').files.length").await? != 0 {
                return Err("Invalid portal result selected a file".into());
            }
        }
        // Keep another tab alive: closing an upload owner must not rely on Exit.
        self.add_tab(Some(url.replace("/one", "/two")));
        let other = self.active().ok_or("Missing second tab")?;
        loaded(&other).await?;
        // A completed load can precede the first native frame. Let this new
        // foreground tab receive its viewport before moving it to the background.
        crate::smoke_input::request(root, json!({"action":"focus"})).await?;
        until("second tab allocation", || {
            other.is_mapped() && parent.is_active()
        })
        .await?;
        let at = Instant::now();
        loop {
            let viewport = inspect(&other, "({w:innerWidth,h:innerHeight})").await?;
            if viewport["w"].as_u64().unwrap_or(0) > 0 && viewport["h"].as_u64().unwrap_or(0) > 0 {
                break;
            }
            if at.elapsed() > Duration::from_secs(15) {
                return Err(format!("Second tab never received a viewport: {viewport}"));
            }
            glib::timeout_future(Duration::from_millis(20)).await;
        }
        self.tabs
            .set_selected_page(&self.page_for(&web).ok_or("Missing original tab")?);
        click(self, root, &web, "#single").await?;
        crate::smoke_portal::wait(root, "calls", 6).await?;
        let epoch = self.epochs.borrow()[&(web.as_ptr() as usize)].get();
        web.reload(); // same URI, different document generation
        crate::smoke_portal::wait(root, "closed", 1).await?;
        until("reload epoch", || {
            self.epochs.borrow()[&(web.as_ptr() as usize)].get() > epoch && !web.is_loading()
        })
        .await?;
        loaded(&web).await?;
        click(self, root, &web, "#single").await?;
        crate::smoke_portal::wait(root, "calls", 7).await?;
        glib::timeout_future(Duration::from_millis(160)).await;
        if self.file_dialogs.borrow().len() != 1 {
            return Err("Old upload result cleared the reopened chooser".into());
        }
        self.tabs
            .close_page(&self.page_for(&web).ok_or("Missing upload tab")?);
        crate::smoke_portal::wait(root, "closed", 2).await?;
        if self.engine.borrow().is_none() || self.tabs.n_pages() != 1 {
            return Err("Closing one upload tab stopped other tabs".into());
        }
        click(self, root, &other, "#single").await?;
        capture("browser-after-tab-close.png");
        crate::smoke_portal::wait(root, "calls", 8).await?;
        uploads(&other, 1).await?;
        capture("browser-upload.png");
        for call in 9..=13 {
            inspect(&other,&format!("(() => {{document.querySelector('#download').href='/blob?case={call}';return true;}})()")).await?;
            click(self, root, &other, "#download").await?;
            crate::smoke_portal::wait(root, "calls", call).await?;
            if call == 12 {
                let row = self
                    .downloads
                    .borrow()
                    .last()
                    .ok_or("No download row")?
                    .row
                    .clone();
                fn cancel(widget: &gtk::Widget) -> Option<gtk::Button> {
                    if let Some(b) = widget.downcast_ref::<gtk::Button>() {
                        if b.tooltip_text().as_deref() == Some("Cancel download") {
                            return Some(b.clone());
                        }
                    }
                    let mut child = widget.first_child();
                    while let Some(w) = child {
                        child = w.next_sibling();
                        if let Some(b) = cancel(&w) {
                            return Some(b);
                        }
                    }
                    None
                }
                let button = cancel(row.upcast_ref()).ok_or("No download Cancel button")?;
                button.emit_clicked();
                button.emit_clicked(); // same GTK turn, before engine failure arrives

                crate::smoke_portal::wait(root, "closed", 3).await?;
            } else if call == 13 {
                self.exit();
                crate::smoke_portal::wait(root, "closed", 4).await?;
            }
            until("download settled", || {
                self.file_dialogs.borrow().is_empty()
                    && self.downloads.borrow().iter().all(|t| t.complete.get())
            })
            .await?;
        }
        self.open(Some(url));
        until("reopened engine", || self.active().is_some()).await?;
        let reopened = self.active().unwrap();
        loaded(&reopened).await?;
        click(self, root, &reopened, "#single").await?;
        crate::smoke_portal::wait(root, "calls", 14).await?;
        glib::timeout_future(Duration::from_millis(160)).await;
        if self.file_dialogs.borrow().len() != 1 {
            return Err("Old download callback cleared a new session's picker".into());
        }
        capture("browser-reopened-picker.png");
        self.exit();
        crate::smoke_portal::wait(root, "closed", 5).await?;
        glib::timeout_future(Duration::from_millis(160)).await;
        if !self.file_dialogs.borrow().is_empty() || self.engine.borrow().is_some() {
            return Err("Exit retained file requests".into());
        }
        self.open(Some(url));
        until("final engine", || self.active().is_some()).await?;
        let final_page = self.active().unwrap();
        loaded(&final_page).await?;
        for call in 15..=16 {
            inspect(&final_page,&format!("(() => {{document.querySelector('#download').href='/blob?case={call}';return true;}})()")).await?;
            click(self, root, &final_page, "#download").await?;
            crate::smoke_portal::wait(root, "calls", call).await?;
            if call == 16 {
                let mut values = crate::browser_policy::NavigationPreferences::from_browser(
                    self.preferences
                        .borrow()
                        .as_ref()
                        .ok_or("No final policy")?,
                );
                values.preview_javascript = false;
                self.apply_navigation(values, false);
                crate::smoke_portal::wait(root, "closed", 6).await?;
            }
            until("final download settled", || {
                self.file_dialogs.borrow().is_empty()
                    && self.downloads.borrow().iter().all(|t| t.complete.get())
            })
            .await?;
            if call == 15 {
                self.show_downloads();
                let dialog = self
                    .dialogs
                    .borrow()
                    .last()
                    .cloned()
                    .ok_or("Downloads dialog did not open")?;
                until("Downloads mapped", || dialog.is_mapped()).await?;
                glib::timeout_future(Duration::from_millis(100)).await;
                capture("browser-downloads.png");
                dialog.close();
                until("Downloads closed", || !dialog.is_mapped()).await?;
            }
        }
        self.exit();
        glib::timeout_future(Duration::from_millis(160)).await;
        until("download file workers drained", || self.file_io.idle()).await?;
        // Hold an actual owned stage across the outer fixture's Window::close.
        // The application must stay mapped until worker cleanup is complete.
        let directory = root.to_owned();
        self.file_io.run(
            move || {
                let target = crate::download_file::Target::prepare(
                    directory.join("close-never-published.bin"),
                    &std::sync::atomic::AtomicBool::new(false),
                )?;
                std::fs::write(directory.join("browser-close-prepared"), b"prepared")
                    .map_err(|e| e.to_string())?;
                let at = Instant::now();
                while !directory.join("browser-close-release").exists() {
                    if at.elapsed() > Duration::from_secs(5) {
                        return Err("Close guard fixture did not release its worker".into());
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                drop(target);
                std::fs::write(directory.join("browser-close-cleaned"), b"cleaned")
                    .map_err(|e| e.to_string())?;
                Ok(())
            },
            |result| {
                if let Err(error) = result {
                    eprintln!("Browser close worker: {error}");
                }
            },
        );
        let directory = root.to_owned();
        let jobs = self.file_io.clone();
        let parent = parent.downgrade();
        glib::spawn_future_local(async move {
            glib::timeout_future(Duration::from_millis(100)).await;
            let state = json!({"mapped":parent.upgrade().is_some_and(|p|p.is_mapped()),"waiting":!jobs.idle()});
            let _ = gio::spawn_blocking(move || {
                let temporary = directory.join("browser-close-guard.tmp");
                std::fs::write(&temporary, serde_json::to_vec(&state).unwrap()).and_then(|_| {
                    std::fs::rename(temporary, directory.join("browser-close-guard.json"))
                })
            })
            .await;
        });
        Ok(
            json!({"ok":true,"realPageInputs":true,"singleAndMultipleUpload":true,"exactUnicodeCrlf":true,"nonlocalAndEmptyRejected":true,"sameUrlReloadCancellation":true,"tabCloseCancellation":true,"newPickerRetained":true,"realNetworkDownloads":true,"downloadCancelClosesPicker":true,"repeatedCancel":true,"exitCancellation":true,"sessionReopen":true,"noOverwrite":true,"pendingPolicyCancellation":true,"closeWaitsForFileWorkers":true}),
        )
    }
}
