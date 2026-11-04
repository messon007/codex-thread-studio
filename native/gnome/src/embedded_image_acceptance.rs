//! Real Markdown/chat image previews and native viewer/Save actions.
use super::*;

async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
    let at = std::time::Instant::now();
    while !test() {
        if at.elapsed() > Duration::from_secs(20) {
            return Err(format!("Embedded image timed out: {step}"));
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
fn previews(root: &gtk::Widget) -> Vec<gtk::Widget> {
    fn collect(root: &gtk::Widget, out: &mut Vec<gtk::Widget>) {
        if root.is_mapped() && root.widget_name() == "studio-embedded-image" {
            out.push(root.clone());
        }
        let mut child = root.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            collect(&widget, out);
        }
    }
    let mut found = Vec::new();
    collect(root, &mut found);
    found
}
fn open(root: &gtk::Widget) -> Option<gtk::Button> {
    if let Some(button) = root
        .downcast_ref::<gtk::Button>()
        .filter(|b| b.tooltip_text().as_deref() == Some("Open image viewer"))
    {
        return Some(button.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if let Some(button) = open(&widget) {
            return Some(button);
        }
    }
    None
}
pub async fn run(ui: &Rc<Ui>, root: &std::path::Path) -> Result<Value, String> {
    let started = std::time::Instant::now();
    let ticks = Rc::new(std::cell::Cell::new(0u64));
    let gap = Rc::new(std::cell::Cell::new(0u128));
    let last = Rc::new(std::cell::Cell::new(started));
    let (count, maximum, previous) = (ticks.clone(), gap.clone(), last.clone());
    let timer = glib::timeout_add_local(Duration::from_millis(16), move || {
        count.set(count.get() + 1);
        maximum.set(
            maximum.get().max(
                previous
                    .replace(std::time::Instant::now())
                    .elapsed()
                    .as_millis(),
            ),
        );
        glib::ControlFlow::Continue
    });
    struct Timer(Option<glib::SourceId>);
    impl Drop for Timer {
        fn drop(&mut self) {
            if let Some(timer) = self.0.take() {
                timer.remove();
            }
        }
    }
    let _timer = Timer(Some(timer));
    until("chat PNG and SVG previews", || {
        ui.state.borrow().history_verified
            && previews(ui.transcript_view.upcast_ref()).len() == 2
            && crate::images_ui::loaded_count(ui.transcript_view.upcast_ref(), true) >= 2
    })
    .await?;
    let chat_ms = started.elapsed().as_millis();
    ui.composer
        .buffer()
        .set_text("Unsent embedded-image draft 中文😀");
    smoke::capture(&ui.window, &root.join("embedded-chat.png"));
    let chat = previews(ui.transcript_view.upcast_ref());
    let stale = open(&chat[0]).ok_or("Chat embedded image action missing")?;
    stale.emit_clicked();
    until("chat native viewer", || ui.image_viewer.smoke_ready()).await?;
    ui.image_viewer
        .smoke_embedded_save(root, "saved-chat.png", (128, 128))
        .await?;
    smoke::capture(&ui.window, &root.join("embedded-viewer.png"));
    ui.image_viewer.close();
    let reader_at = std::time::Instant::now();
    ui.reader.open_path(&root.join("project/embedded.md"));
    ui.show_tool("document");
    if let Err(error) = until("reader PNG and SVG previews", || {
        ui.reader.is_ready()
            && previews(ui.reader.widget.upcast_ref()).len() == 2
            && crate::images_ui::loaded_count(ui.reader.widget.upcast_ref(), true) >= 2
    })
    .await
    {
        smoke::capture(&ui.window, &root.join("embedded-reader-failed.png"));
        return Err(format!(
            "{error}; {}; previews={} mapped={} retained={}",
            ui.reader.smoke_comment_debug(),
            previews(ui.reader.widget.upcast_ref()).len(),
            crate::images_ui::loaded_count(ui.reader.widget.upcast_ref(), true),
            crate::images_ui::loaded_count(ui.reader.widget.upcast_ref(), false)
        ));
    }
    let reader_ms = reader_at.elapsed().as_millis();
    let references = gio::spawn_blocking({
        let root = root.to_owned();
        move || root.join("reference-source").is_file()
    })
    .await
    .map_err(|_| "Reference fixture probe stopped")?;
    if references {
        fn guide(root: &gtk::Widget) -> Option<gtk::Label> {
            if let Some(label) = root
                .downcast_ref::<gtk::Label>()
                .filter(|l| l.is_mapped() && l.label().contains("guide.md:5"))
            {
                return Some(label.clone());
            }
            let mut child = root.first_child();
            while let Some(w) = child {
                child = w.next_sibling();
                if let Some(label) = guide(&w) {
                    return Some(label);
                }
            }
            None
        }
        let link = guide(ui.reader.widget.upcast_ref()).ok_or("Off-page reference link missing")?;
        let _: bool = link.emit_by_name("activate-link", &[&"guide.md:5"]);
        ui.reader.smoke_link(&root.join("project/guide.md")).await?;
        ui.reader.open_path(&root.join("project/embedded.md"));
        until("reference reader return", || {
            ui.reader.is_ready()
                && previews(ui.reader.widget.upcast_ref()).len() == 2
                && crate::images_ui::loaded_count(ui.reader.widget.upcast_ref(), true) >= 2
        })
        .await?;
    }
    fn caption(root: &gtk::Widget) -> Option<gtk::Label> {
        if let Some(label) = root
            .downcast_ref::<gtk::Label>()
            .filter(|l| l.is_mapped() && l.is_selectable() && l.text() == "Embedded PNG 中文😀")
        {
            return Some(label.clone());
        }
        let mut child = root.first_child();
        while let Some(w) = child {
            child = w.next_sibling();
            if let Some(label) = caption(&w) {
                return Some(label);
            }
        }
        None
    }
    let selected =
        caption(ui.reader.widget.upcast_ref()).ok_or("Image caption selection missing")?;
    selected.select_region(0, -1);
    ui.reader.smoke_inline_comment("Embedded PNG 中文😀", 3)?;

    smoke::capture(&ui.window, &root.join("embedded-reader.png"));
    let images = previews(ui.reader.widget.upcast_ref());
    open(&images[1])
        .ok_or("Reader SVG action missing")?
        .emit_clicked();
    until("reader SVG native viewer", || ui.image_viewer.smoke_ready()).await?;
    ui.image_viewer
        .smoke_embedded_save(root, "saved-reader.svg", (2048, 1366))
        .await?;
    ui.image_viewer.close();
    ui.show_tool("document");
    let large = gio::spawn_blocking({
        let root = root.to_owned();
        move || root.join("large-source").is_file()
    })
    .await
    .map_err(|_| "Fixture probe stopped")?;
    if large {
        until("reader large image remapped", || {
            open(&images[0]).is_some_and(|b| b.is_mapped())
        })
        .await?;
        open(&images[0]).unwrap().emit_clicked();
        until("large image viewer source lease", || {
            ui.image_viewer.smoke_ready()
        })
        .await?;
        ui.reader.smoke_close();
        ui.image_viewer
            .smoke_embedded_save(root, "saved-reader.png", (128, 128))
            .await?;
        ui.image_viewer.close();
        ui.show_tool("document");
    }
    ui.reader.open_path(&root.join("project/replacement.md"));
    until("replacement document", || {
        ui.reader.is_ready() && previews(ui.reader.widget.upcast_ref()).is_empty()
    })
    .await?;
    if crate::images_ui::loaded_count(ui.reader.widget.upcast_ref(), false) != 0 {
        return Err("Old reader image textures survived replacement".into());
    }
    let retained = open(&images[0]).ok_or("Retained reader action missing")?;
    retained.emit_clicked();
    if ui.image_viewer.has_document() {
        return Err("Unmapped image action opened stale viewer".into());
    }
    if !ui
        .composer_text()
        .contains("Unsent embedded-image draft 中文😀")
    {
        return Err("Image viewing changed the draft".into());
    }
    Ok(
        json!({"ok":true,"chatAndReader":true,"pngAndPercentSvg":true,"largeDataUrl":true,"nativeViewer":true,"originalPngSvgSave":true,"replacementCleanup":true,"staleActionRejected":true,"draftRetained":true,"captionSourceLines":true,"offPageReferenceLink":references,"largeDocument":large,"originalSurvivesReaderClose":large,"chatPreviewsMs":chat_ms,"readerPreviewsMs":reader_ms,"mainLoopTicks":ticks.get(),"maxHeartbeatGapMs":gap.get(),"maximumImageCallbackMs":ui.images.maximum_callback_ms.get()}),
    )
}
