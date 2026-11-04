use super::*;
use std::path::Path;
impl Source {
    pub(crate) async fn smoke_search(
        &self,
        root: &Path,
        capture: impl Fn(&str),
    ) -> Result<serde_json::Value, String> {
        async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
            let started = std::time::Instant::now();
            while !test() {
                if started.elapsed() > Duration::from_secs(15) {
                    return Err(format!("Source search timed out: {step}"));
                }
                glib::timeout_future(Duration::from_millis(10)).await;
            }
            Ok(())
        }
        let project = root.join("project");
        let ticks = Rc::new(Cell::new(0));
        let max_gap = Rc::new(Cell::new(0u128));
        let clock = Rc::new(Cell::new(std::time::Instant::now()));
        let heartbeat = glib::timeout_add_local(Duration::from_millis(10), {
            let ticks = ticks.clone();
            let clock = clock.clone();
            let max_gap = max_gap.clone();
            move || {
                let now = std::time::Instant::now();
                max_gap.set(
                    max_gap
                        .get()
                        .max(now.duration_since(clock.replace(now)).as_millis()),
                );
                ticks.set(ticks.get() + 1);
                glib::ControlFlow::Continue
            }
        });
        let result=async {
            let started=std::time::Instant::now();
            self.open(project.to_str().unwrap(),"search.rs");
            let dispatch_ms = started.elapsed().as_millis();
            until("large snapshot",||self.text.borrow().is_some() && self.widget.is_mapped()).await?;
            let open_ms=started.elapsed().as_millis();
            let open_gap = max_gap.replace(0);
            clock.set(std::time::Instant::now());
            let bytes=self.text.borrow().as_ref().unwrap().document.bytes();
            self.search.toggle.emit_clicked();
            let quote="needle 中文😀  ";
            self.search.entry.set_text(quote);
            until("full-file matches",||self.search.index.get().is_some()).await?;
            if self.search.matches.borrow().hits.len()!=3 || self.line.get()!=205 || self.search_selection()!=quote { return Err("Full-file search lost exact spaces, Unicode or first line".into()); }
            self.search.next.emit_clicked();
            if self.line.get()!=801 || self.search_selection()!=quote { return Err("Next match did not cross source pages".into()); }
            self.search.previous.emit_clicked();
            if self.line.get()!=205 { return Err("Previous match did not return".into()); }
            self.search.previous.emit_clicked();
            if self.search_selection()!=quote || self.window.borrow().as_ref().unwrap().start<4*1024*1024 { return Err("Wrapped match in the long line used its first window".into()); }
            capture("source-search-long-line.png");
            let retained = self.window.borrow().as_ref().unwrap().start;
            self.widget.set_visible(false);
            self.widget.set_visible(true);
            if let Some(open) = self.on_open.borrow().as_ref() { open(); }
            until("Source return", || self.widget.is_mapped()).await?;
            if self.search.index.get()!=Some(2) || self.window.borrow().as_ref().unwrap().start!=retained || self.search_selection()!=quote {
                return Err("Returning to Source reset its selected occurrence or page".into());
            }
            self.search.entry.set_text("été");
            until("Unicode folding",||self.search.index.get().is_some()).await?;
            if self.search.matches.borrow().hits.len()!=3 { return Err("Unicode case folding lost occurrences".into()); }
            self.search.case.set_active(true);
            until("case sensitive",||self.search.index.get().is_some()).await?;
            if self.search.matches.borrow().hits.len()!=1 || self.search_selection()!="été" { return Err("Case toggle did not constrain search".into()); }
            self.search.case.set_active(false);
            self.search.entry.set_text("never present exact text");
            until("no matches",||self.search.count.text()==crate::language::tr("No matches found")).await?;
            if self.search.next.is_sensitive() || self.search.previous.is_sensitive() { return Err("Empty result left match navigation enabled".into()); }
            self.search.entry.set_text("repeat");
            until("bounded matches",||self.search.index.get().is_some()).await?;
            if self.search.matches.borrow().hits.len()!=crate::source_search::MAX_HITS || !self.search.matches.borrow().limited { return Err("Result cap was not explicit".into()); }
            self.search.entry.set_text(quote);
            until("literal restore",||self.search.index.get().is_some()).await?;
            crate::language::set("zh-CN","en-US")?;
            crate::i18n_ui::apply();
            if self.search.entry.text()!=quote || self.search.count.text()!="第 1 项，共 3 项匹配" || self.search.toggle.tooltip_text().as_deref()!=Some("在源码中查找") { return Err("Live language switch changed source query or lost search bindings".into()); }
            capture("source-search-chinese.png");
            crate::language::set("en-US","en-US")?;
            crate::i18n_ui::apply();
            self.search.entry.set_text("obsolete");
            self.start_search();
            let generation=self.generation.get();
            let serial=self.search.serial.get();
            self.search.entry.set_text(quote);
            self.event(Response {panel:3,generation,serial,kind:"source-search",result:Ok(Reply::SourceMatches(Matches {hits:vec![Hit {start:0,end:1,line:1}],limited:false}))});
            if self.search.index.get().is_some() { return Err("An obsolete query response overwrote current search".into()); }
            until("new query",||self.search.index.get().is_some()).await?;
            if self.search_selection()!=quote { return Err("Cancelled worker changed current selection".into()); }
            let old_generation=self.generation.get(); let old_serial=self.search.serial.get();
            self.open(project.to_str().unwrap(),"real-app.rs");
            self.event(Response {panel:3,generation:old_generation,serial:old_serial,kind:"source-search",result:Ok(Reply::SourceMatches(Matches {hits:vec![Hit {start:0,end:1,line:1}],limited:false}))});
            until("real source",||self.text.borrow().is_some()).await?;
            if self.search.bar.is_visible() || self.search.index.get().is_some() { return Err("File replacement retained stale search controls".into()); }
            self.search.toggle.emit_clicked();
            self.search.entry.set_text(".max_blocking_threads(2)");
            until("actual project file",||self.search.index.get().is_some()).await?;
            let real_line=self.search.matches.borrow().hits[0].line;
            if self.search_selection()!=".max_blocking_threads(2)" { return Err("Real project file search lost source selection".into()); }
            capture("source-search-real-file.png");
            self.search.close.emit_clicked();
            if self.search.bar.is_visible() || self.search.settings.search_text().is_some() { return Err("Closing search left page highlighting active".into()); }
            self.search.toggle.emit_clicked();
            until("reopened query",||self.search.index.get().is_some()).await?;
            self.search.entry.set_text("");
            if self.search.next.is_sensitive() || self.search.index.get().is_some() { return Err("Clearing query left stale results".into()); }
            let serial=self.search.serial.get(); let generation=self.generation.get();
            self.close.emit_clicked();
            self.event(Response {panel:3,generation,serial,kind:"source-search",result:Ok(Reply::SourceMatches(Matches {hits:vec![Hit {start:0,end:1,line:1}],limited:false}))});
            if self.search.index.get().is_some() || self.search.toggle.is_sensitive() { return Err("A stale response reactivated closed Source".into()); }
            if ticks.get()<5 { return Err("Source search did not leave the GTK heartbeat running".into()); }
            Ok(serde_json::json!({"ok":true,"bytes":bytes,"openMs":open_ms,"openDispatchMs":dispatch_ms,"openHeartbeatGapMs":open_gap,"mainLoopTicks":ticks.get(),"maxHeartbeatGapMs":max_gap.get(),"realFileLine":real_line,"literalWhitespace":true,"caseFolding":true,"crossPage":true,"longLine":true,"viewerReturn":true,"boundedResults":true,"liveLanguage":true,"queryGeneration":true,"fileGeneration":true,"closedGeneration":true}))
        }.await;
        heartbeat.remove();
        result
    }
    fn search_selection(&self) -> String {
        let buffer = self.view.buffer();
        buffer
            .selection_bounds()
            .map(|(a, b)| buffer.text(&a, &b, true).to_string())
            .unwrap_or_default()
    }
}
