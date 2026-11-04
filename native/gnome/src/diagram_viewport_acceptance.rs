//! Private Wayland clipping/admission with synthetic replies, not engine timing.
use super::*;
use std::{path::Path, time::Duration};
impl Renderer {
    pub async fn smoke_viewport(
        &self,
        parent: &impl IsA<gtk::Window>,
        directory: &Path,
    ) -> Result<serde_json::Value, String> {
        async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
            let at = std::time::Instant::now();
            while !test() {
                if at.elapsed() > Duration::from_secs(8) {
                    return Err(format!("Diagram viewport timed out: {step}"));
                }
                glib::timeout_future(Duration::from_millis(20)).await;
            }
            Ok(())
        }
        fn drain(receiver: &mut mpsc::Receiver<Message>) -> Vec<Message> {
            let mut result = Vec::new();
            while let Ok(message) = receiver.try_recv() {
                result.push(message);
            }
            result
        }
        fn jobs(messages: &[Message]) -> Vec<&Message> {
            messages
                .iter()
                .filter(|m| matches!(m.request, Request::Diagram { .. }))
                .collect()
        }
        let (sender, mut receiver) = mpsc::channel(128);
        let owner = Self::new_viewport_bounded(sender, 996, 1024 * 1024);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let mut first = Vec::new();
        for n in 0..3 {
            let root = owner.widget(
                "mermaid",
                &format!("flowchart LR\nA{n}-->B{n}"),
                Styles::new(),
            );
            content.append(&root);
            first.push(root);
        }
        let spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
        spacer.set_height_request(1400);
        content.append(&spacer);
        let inside = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let top = gtk::Box::new(gtk::Orientation::Vertical, 0);
        top.set_height_request(900);
        inside.append(&top);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let left = gtk::Box::new(gtk::Orientation::Vertical, 0);
        left.set_width_request(1000);
        row.append(&left);
        let nested = owner.widget("d2", "a -> b: clipped", Styles::new());
        nested.set_width_request(320);
        row.append(&nested);
        inside.append(&row);
        let inner = gtk::ScrolledWindow::builder()
            .child(&inside)
            .height_request(280)
            .hscrollbar_policy(gtk::PolicyType::Always)
            .vscrollbar_policy(gtk::PolicyType::Always)
            .build();
        content.append(&inner);
        let outer = gtk::ScrolledWindow::builder().child(&content).build();
        let window = adw::Window::builder()
            .title("Private native diagram viewport")
            .transient_for(parent)
            .default_width(600)
            .default_height(440)
            .content(&outer)
            .build();
        // Ensure failures still close this private toplevel and cancel its lane.
        struct Cleanup(adw::Window, Rc<Renderer>);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                self.1.reset();
                self.0.close();
            }
        }
        let _cleanup = Cleanup(window.clone(), owner.clone());
        window.present();
        crate::window_capture::ready(&outer).await?;
        until("two initial admitted jobs", || {
            owner.active.borrow().len() == 2
        })
        .await?;
        let initial = drain(&mut receiver);
        let initial_jobs = jobs(&initial);
        if initial_jobs.len() != 2
            || initial_jobs
                .iter()
                .any(|m| matches!(&m.request,Request::Diagram{kind,..} if kind=="d2"))
        {
            return Err("Diagram admission exceeded two slots or rendered a clipped chart".into());
        }
        let old = initial_jobs[0];
        let get = |root: &gtk::Box| {
            owner
                .widgets
                .borrow()
                .values()
                .filter_map(Weak::upgrade)
                .find(|d| d.root.upgrade().as_ref() == Some(root))
                .unwrap()
        };
        let nested_item = get(&nested);
        let old_item = first
            .iter()
            .map(&get)
            .find(|d| d.serial.get() == old.serial)
            .unwrap();
        let reply = |message: &Message| Response {
            key: 996,
            generation: message.generation,
            serial: message.serial,
            kind: "diagram",
            result: Ok(Reply::Diagram {
                id: if let Request::Diagram { id, .. } = message.request {
                    id
                } else {
                    unreachable!()
                },
                image: crate::diagram::Image {
                    width: 320,
                    height: 160,
                    pixels: [30u8, 150, 110, 255].repeat(320 * 160).into(),
                    math: None,
                    vector: None,
                },
            }),
        };
        let adjustment = outer.vadjustment();
        adjustment.set_value(adjustment.upper() - adjustment.page_size());
        crate::window_capture::next_frame(&outer).await?;
        until("offscreen jobs cancelled", || {
            owner.active.borrow().is_empty() && owner.pending.borrow().is_empty()
        })
        .await?;
        let cancelled = drain(&mut receiver);
        if jobs(&cancelled).len() != 0
            || cancelled
                .iter()
                .filter(|m| matches!(m.request, Request::CancelDiagram { .. }))
                .count()
                != 2
        {
            return Err("Offscreen cancellation admitted a queued/clipped diagram".into());
        }
        owner.event(reply(old));
        if old_item.picture.paintable().is_some() || old_item.bytes.get() != 0 {
            return Err("Late offscreen diagram reply installed pixels".into());
        }
        let v = inner.vadjustment();
        v.set_value(v.upper() - v.page_size());
        crate::window_capture::next_frame(&inner).await?;
        glib::timeout_future(Duration::from_millis(70)).await;
        if !jobs(&drain(&mut receiver)).is_empty() || nested_item.serial.get() != 0 {
            return Err("Horizontal clip did not block diagram admission".into());
        }
        let h = inner.hadjustment();
        h.set_value(h.upper() - h.page_size());
        crate::window_capture::next_frame(&inner).await?;
        until("nested chart admitted", || {
            nested_item.serial.get() > 0 && owner.active.borrow().contains(&nested_item.id)
        })
        .await?;
        let messages = drain(&mut receiver);
        let visible = jobs(&messages);
        if visible.len() != 1
            || !matches!(&visible[0].request,Request::Diagram{kind,..} if kind=="d2")
        {
            return Err("Nested visible chart did not get its own request".into());
        }
        owner.event(reply(visible[0]));
        crate::window_capture::next_frame(&nested).await?;
        // An error must settle instead of being retried on every geometry
        // notification. Only the native Retry button should admit another job.
        owner.cancel(nested_item.id);
        nested_item.clear();
        owner.enqueue(nested_item.id);
        let messages = drain(&mut receiver);
        let retry_job = jobs(&messages);
        if retry_job.len() != 1 {
            return Err("Visible retry setup did not admit one diagram".into());
        }
        owner.event(Response {
            key: 996,
            generation: owner.generation.get(),
            serial: retry_job[0].serial,
            kind: "diagram",
            result: Err("Isolated viewport engine failure".into()),
        });
        crate::window_capture::next_frame(&nested).await?;
        glib::timeout_future(Duration::from_millis(80)).await;
        let passes = owner.visibility_passes.get();
        let sequence = owner.sequence.get();
        glib::timeout_future(Duration::from_millis(100)).await;
        if !nested_item.retry.is_visible()
            || !nested_item.failed.get()
            || owner.visibility_passes.get() != passes
            || owner.sequence.get() != sequence
            || !jobs(&drain(&mut receiver)).is_empty()
        {
            return Err("Failed visible diagram kept polling or retrying while idle".into());
        }
        nested_item.retry.emit_clicked();
        until("native Retry admitted", || {
            owner.active.borrow().contains(&nested_item.id)
        })
        .await?;
        let messages = drain(&mut receiver);
        let retry_job = jobs(&messages);
        if retry_job.len() != 1 {
            return Err("Native Retry did not admit exactly one diagram".into());
        }
        owner.event(reply(retry_job[0]));
        crate::window_capture::next_frame(&nested).await?;
        let height = nested.height();
        if nested_item.picture.paintable().is_none() {
            return Err("Visible diagram reply was not installed".into());
        }
        crate::window_capture::capture(&window, &directory.join("diagram-viewport.png")).await?;
        // Moving a rendered card outside both scopes releases backing data but
        // keeps GTK's measured aspect ratio, instead of collapsing the anchor.
        adjustment.set_value(0.0);
        crate::window_capture::next_frame(&outer).await?;
        if let Err(error) = until("offscreen pixels released", || {
            nested_item.bytes.get() == 0 && nested_item.picture.paintable().is_none()
        })
        .await
        {
            let details = serde_json::json!({
                "outer":[adjustment.value(),adjustment.upper(),adjustment.page_size()],
                "inner":[v.value(),v.upper(),v.page_size()],
                "bounds":nested.compute_bounds(&outer).map(|b|[b.x(),b.y(),b.width(),b.height()]),
                "focus":gtk::prelude::GtkWindowExt::focus(&window).map(|w|(w.type_().name().to_owned(),w.widget_name().to_string())),
                "inView":nested_item.in_view(),"bytes":nested_item.bytes.get(),"serial":nested_item.serial.get(),
                "passes":owner.visibility_passes.get(),"queued":owner.visibility.borrow().as_ref().is_some_and(|u|u.pending()),
            });
            crate::window_capture::capture(
                &window,
                &directory.join("diagram-viewport-failure.png"),
            )
            .await?;
            return Err(format!("{error}: {details}"));
        }
        crate::window_capture::next_frame(&outer).await?;
        if nested.height() != height || nested_item.picture.vector().is_some() {
            return Err("Offscreen diagram release changed anchor geometry or retained SVG".into());
        }
        owner.reset();
        drain(&mut receiver);
        // Persistent viewport adjustments outlive a fragment. Reset disconnects
        // their handlers and old remaps never resurrect a cancelled generation.
        h.set_value(0.0);
        v.set_value(0.0);
        adjustment.set_value(adjustment.upper() - adjustment.page_size());
        glib::timeout_future(Duration::from_millis(70)).await;
        if !jobs(&drain(&mut receiver)).is_empty() || !owner.widgets.borrow().is_empty() {
            return Err("Reset resurrected diagram admission".into());
        }
        Ok(
            serde_json::json!({"ok":true,"twoSlots":true,"nestedClipping":true,"horizontalClipping":true,
            "offscreenCancellation":true,"lateReplyRejected":true,"pixelsReleased":true,"geometryRetained":true,"resetCleanup":true,
            "syntheticReplies":true,"idlePolling":false,"failureSettles":true,"nativeRetry":true}),
        )
    }
}
