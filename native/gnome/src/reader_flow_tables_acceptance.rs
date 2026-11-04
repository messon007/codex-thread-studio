//! Actual indexed report tables mounted in the experimental continuous reader.
use super::*;
impl Reader {
    pub async fn smoke_flow_tables(
        self: &Rc<Self>,
        directory: &Path,
    ) -> Result<serde_json::Value, String> {
        async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
            let at = std::time::Instant::now();
            while !test() {
                if at.elapsed() > Duration::from_secs(20) {
                    return Err(format!("Indexed flow table timed out: {step}"));
                }
                glib::timeout_future(Duration::from_millis(20)).await;
            }
            Ok(())
        }
        fn checks(widget: &gtk::Widget) -> Vec<gtk::CheckButton> {
            let mut values = widget
                .downcast_ref::<gtk::CheckButton>()
                .cloned()
                .into_iter()
                .collect::<Vec<_>>();
            let mut child = widget.first_child();
            while let Some(widget) = child {
                child = widget.next_sibling();
                values.extend(checks(&widget));
            }
            values
        }
        async fn hash(path: PathBuf) -> Result<Vec<u8>, String> {
            gio::spawn_blocking(move || {
                use sha2::Digest;
                std::fs::read(path)
                    .map(|bytes| sha2::Sha256::digest(bytes).to_vec())
                    .map_err(|e| e.to_string())
            })
            .await
            .map_err(|_| "Flow hash worker stopped")?
        }
        let path = directory.join("project/large.md");
        let original = hash(path.clone()).await?;
        self.open_path(&path);
        until("real report index", || {
            self.is_ready() && self.state.borrow().path.as_ref() == Some(&path)
        })
        .await?;
        let (first_page, file_bytes, revision, context_pages, full_pages, full_rows) = {
            let state = self.state.borrow();
            let index = state.index.as_ref().unwrap();
            (
                index
                    .tables
                    .first()
                    .ok_or("Real report has no indexed table")?
                    .first_page,
                index.source_bytes,
                index.revision.clone(),
                (index.tables[0].first_page..index.tables[0].end_page)
                    .take(2)
                    .map(|p| (p, index.pages[p].clone()))
                    .collect::<Vec<_>>(),
                (index.tables[0].first_page..index.tables[0].end_page)
                    .map(|p| (p, index.pages[p].clone()))
                    .collect::<Vec<_>>(),
                index.tables[0].rows,
            )
        };
        self.navigate(Navigation::Page(first_page));
        until("real table fragment", || {
            !self.state.borrow().busy && self.blocks.n_items() > 0
        })
        .await?;
        let block = (0..self.blocks.n_items())
            .filter_map(|i| {
                let object = self
                    .blocks
                    .item(i)?
                    .downcast::<glib::BoxedAnyObject>()
                    .ok()?;
                let row = object.borrow::<DisplayBlock>();
                matches!(row.block.kind, BlockKind::Table { id: Some(0), .. })
                    .then(|| row.block.clone())
            })
            .next()
            .ok_or("Real indexed table fragment missing")?;
        let row_start = if let BlockKind::Table { row_start, .. } = &block.kind {
            *row_start
        } else {
            unreachable!()
        };
        if context_pages.len() != 2 || context_pages.iter().any(|(_, p)| p.table != Some(0)) {
            return Err("Real report needs two index pages of its first table".into());
        }
        let context_rows = context_pages.iter().map(|(_, p)| p.rows).sum::<usize>();
        until("backend source cells for both index pages", || {
            self.state
                .borrow()
                .tables
                .get(&0)
                .is_some_and(|t| t.rows.n_items() as usize >= context_rows)
        })
        .await?;
        let expected_cells = {
            let table = self
                .state
                .borrow()
                .tables
                .get(&0)
                .cloned()
                .ok_or("Backend table source missing")?;
            (0..context_rows)
                .map(|i| {
                    table
                        .rows
                        .item(i as u32)
                        .and_downcast::<glib::BoxedAnyObject>()
                        .ok_or("Backend source row missing")
                        .map(|row| row.borrow::<Vec<studio_markdown_native::Cell>>().clone())
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        let source_path = path.clone();
        let document_id = self
            .state
            .borrow()
            .reader_id
            .clone()
            .ok_or("Reader snapshot missing")?;
        let (prepared, joined_rows, whole) = gio::spawn_blocking(move || {
            let fragments = table_context(
                &source_path,
                &block,
                &document_id,
                &revision,
                &context_pages,
            )?;
            let prepared = crate::text_flow::prepare_lazy_context(
                &fragments,
                Arc::new(std::sync::atomic::AtomicBool::new(false)),
            )?;
            if prepared.document.objects.len() != 1
                || prepared.document.segments.len() != 1
                || prepared.document.segments[0].source.start != block.source.start
                || prepared.document.segments[0].source.line != block.source.line
                || prepared.document.segments[0].source.end != context_pages.last().unwrap().1.end
            {
                return Err("Joined real table changed its containing source range".to_owned());
            }
            let BlockKind::Table {
                header: original_header,
                ..
            } = &block.kind
            else {
                unreachable!()
            };
            let studio_markdown_native::flow::ObjectKind::Table { header, rows, .. } =
                &prepared.document.objects[0].kind
            else {
                unreachable!()
            };
            if original_header != header
                || rows.len() != context_rows
                || rows.iter().map(|r| &r.cells).ne(expected_cells.iter())
            {
                return Err(
                    "Joining real source slices changed cell content or source coordinates"
                        .to_owned(),
                );
            }
            let count = rows.len();
            let complete=studio_markdown_native::flow::project_context(&table_context(&source_path,&block,&document_id,&revision,&full_pages)?)?;
            if !matches!(&complete.objects[0].kind,studio_markdown_native::flow::ObjectKind::Table{rows,..} if rows.len()==full_rows) {
                return Err("Real table expected selection is incomplete".into());
            }
            let whole=complete.select(0..complete.characters,studio_markdown_native::flow::BUFFER_BYTES)?;
            Ok((prepared, count, whole))
        })
        .await
        .map_err(|_| "Indexed flow preparation stopped")??;
        let flow = self.flow_view(prepared)?;
        let table = self
            .flow_tables
            .borrow()
            .get(&0)
            .cloned()
            .ok_or("Indexed factory was not used")?;
        if flow.objects.first() != Some(table.widget.upcast_ref()) {
            return Err("Indexed anchor did not reuse the production table widget".into());
        }
        self.preview_scroll.set_child(Some(&flow.widget));
        until("allocated table and bounded source rows", || {
            table.widget.is_mapped()
                && table.widget.width() > 0
                && table.rows.n_items() > 0
                && table.start.get() == row_start
        })
        .await?;
        if table.rows.n_items() > 50 || table.total.get() <= 50 {
            return Err("Real flow table does not exercise bounded multi-page data".into());
        }
        let initial_rows = table.rows.n_items();
        let initial_columns = table.columns.borrow().len();
        let original_column = table
            .view
            .columns()
            .item(0)
            .and_downcast::<gtk::ColumnViewColumn>()
            .ok_or("First flow column missing")?;
        original_column.set_fixed_width(310);
        let popup = table.menu.popover().ok_or("Flow column picker missing")?;
        popup.popup();
        until("mapped native picker", || {
            checks(popup.upcast_ref())
                .iter()
                .filter(|c| c.is_mapped())
                .count()
                >= 2
        })
        .await?;
        let choices = checks(popup.upcast_ref());
        choices[0].set_active(false);
        until("projected columns", || {
            table.view.columns().n_items() as usize == initial_columns - 1
                && table
                    .rows
                    .item(0)
                    .and_downcast::<glib::BoxedAnyObject>()
                    .is_some_and(|r| {
                        r.borrow::<Vec<studio_markdown_native::Cell>>().len() == initial_columns - 1
                    })
        })
        .await?;
        let first = table
            .view
            .columns()
            .item(0)
            .and_downcast::<gtk::ColumnViewColumn>()
            .unwrap();
        if first.fixed_width() != 210 {
            return Err("Indexed flow width moved to another original column".into());
        }
        choices[0].set_active(true);
        until("original column and width restored", || {
            table.view.columns().n_items() as usize == initial_columns
                && table
                    .view
                    .columns()
                    .item(0)
                    .and_downcast::<gtk::ColumnViewColumn>()
                    .is_some_and(|c| c.fixed_width() == 310 && c.title() == original_column.title())
        })
        .await?;
        popup.popdown();
        crate::window_capture::next_frame(&self.widget).await?;
        let scroll = self.preview_scroll.vadjustment();
        let before_scroll = scroll.value();
        table.next.emit_clicked();
        until("next row page", || {
            table.start.get() == row_start + table.size.get() && table.rows.n_items() > 0
        })
        .await?;
        crate::window_capture::next_frame(&self.widget).await?;
        if (scroll.value() - before_scroll).abs() > 1.0 || table.rows.n_items() > 50 {
            return Err("Indexed flow paging moved outer scroll or exceeded row bound".into());
        }
        let cell = {
            let row = table
                .rows
                .item(0)
                .and_downcast::<glib::BoxedAnyObject>()
                .unwrap();
            let cells = row.borrow::<Vec<studio_markdown_native::Cell>>();
            cells[0].clone()
        };
        until("new-page native cell source", || {
            self.selection.borrow().iter().any(|entry| {
                entry.source.as_ref() == Some(&cell.source)
                    && entry
                        .widget
                        .upgrade()
                        .is_some_and(|w| w.is_mapped() && w.is::<gtk::Label>())
            })
        })
        .await?;
        let label = self
            .selection
            .borrow()
            .iter()
            .find_map(|entry| {
                (entry.source.as_ref() == Some(&cell.source))
                    .then(|| entry.widget.upgrade())
                    .flatten()
                    .filter(|w| w.is_mapped())
                    .and_then(|w| w.downcast::<gtk::Label>().ok())
            })
            .ok_or("Paged cell label missing")?;
        label.grab_focus();
        label.select_region(0, -1);
        self.smoke_inline_comment_range(
            &cell.content.plain,
            cell.source.line,
            cell.source.end_line,
        )?;
        label.emit_by_name::<()>("copy-clipboard", &[]);
        let copied = label
            .clipboard()
            .read_text_future()
            .await
            .map_err(|e| e.to_string())?;
        if copied.as_deref() != Some(cell.content.plain.as_str()) {
            return Err("Indexed cell Copy lost original text".into());
        }
        // The anchor represents the original full table, independently of the
        // visible page/columns. Its initial projection contains only 32 rows.
        let buffer = flow.widget.buffer();
        flow.widget.grab_focus();
        buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
        flow.widget.emit_copy_clipboard();
        until("complete indexed-table Copy", || {
            flow.copied.borrow().is_some() && !flow.copying.get()
        })
        .await?;
        let copied = flow
            .copied
            .borrow()
            .as_ref()
            .unwrap()
            .as_ref()
            .map_err(Clone::clone)?
            .clone();
        if copied.text != whole.text
            || copied.source != whole.source
            || copied.text == flow.document.objects[0].alternative
        {
            return Err(
                "Indexed table Copy did not expand all original rows and source bounds".into(),
            );
        }
        if flow
            .widget
            .clipboard()
            .read_text_future()
            .await
            .map_err(|e| e.to_string())?
            .as_deref()
            != Some(whole.text.as_str())
        {
            return Err("Complete indexed-table clipboard differs from original source".into());
        }
        if flow
            .widget
            .primary_clipboard()
            .read_text_future()
            .await
            .map_err(|e| e.to_string())?
            .as_deref()
            != Some(whole.text.as_str())
        {
            return Err("Complete indexed-table primary selection differs from Copy".into());
        }
        let captured = Rc::new(RefCell::new(None));
        let capture = captured.clone();
        let previous = self
            .comments
            .replace(Some(Rc::new(crate::annotations_ui::CaptureTarget {
                owner: Box::new(|| Some("indexed-selection-owner".into())),
                capture: Box::new(move |owner, selected| {
                    if owner != "indexed-selection-owner" {
                        return Err("Wrong indexed comment owner".into());
                    }
                    capture.replace(Some(selected));
                    Ok(())
                }),
            })));
        struct Restore<'a>(&'a Reader, Option<Rc<crate::annotations_ui::CaptureTarget>>);
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                self.0.comments.replace(self.1.take());
            }
        }
        let restore = Restore(self, previous);
        flow.widget
            .activate_action("reader-comment.capture", None)
            .map_err(|e| e.to_string())?;
        until("indexed comment capture or byte limit", || {
            !flow.commenting.get()
        })
        .await?;
        let comment_budget = whole.text.len() > studio_markdown_native::quote::MAX_QUOTE;
        if comment_budget {
            if captured.borrow().is_some() || !self.status.text().contains("16 KiB") {
                return Err("Oversized indexed-table comment was not rejected".into());
            }
        } else {
            let selected = captured.borrow();
            let selected = selected
                .as_ref()
                .ok_or("Complete indexed-table comment was not captured")?;
            let source = whole.source.as_ref().unwrap();
            if selected.excerpt != whole.text
                || selected.source["anchor"]["textKind"] != "flow"
                || selected.source["anchor"]["startLine"].as_u64() != Some(source.line as u64)
                || selected.source["anchor"]["endLine"].as_u64() != Some(source.end_line as u64)
            {
                return Err("Indexed-table comment lost full source identity".into());
            }
        }
        drop(restore);
        table.previous.emit_clicked();
        until("previous row page", || {
            table.start.get() == row_start && table.rows.n_items() == initial_rows
        })
        .await?;
        crate::window_capture::capture(&self.window, &directory.join("flow-indexed-table.png"))
            .await?;
        let source = flow.document.clone();
        let revision = self.state.borrow().index.as_ref().unwrap().revision.clone();
        let pending_prepared = gio::spawn_blocking(move || {
            crate::text_flow::prepare_projection(
                source,
                revision,
                Arc::new(std::sync::atomic::AtomicBool::new(false)),
                false,
            )
        })
        .await
        .map_err(|_| "Pending selection worker stopped")??;
        // Replace the previous adapter through its production lifecycle. It
        // owns indexed anchors that must be detached before its view unroots.
        self.clear_flow_preview();
        let mut options = self.flow_options().unwrap();
        options.indexed_tables = Some(Rc::new(|_, _| {
            Ok(gtk::Box::new(gtk::Orientation::Vertical, 0).upcast())
        }));
        options.comments = None;
        let pending = Rc::new(RefCell::new(Vec::<
            tokio::sync::oneshot::Sender<
                Result<Vec<studio_markdown_native::flow::TableText>, String>,
            >,
        >::new()));
        let pending_sink = pending.clone();
        options.selection_tables.as_mut().unwrap().resolve = Rc::new(move |_, _| {
            let (reply, receive) = tokio::sync::oneshot::channel();
            pending_sink.borrow_mut().push(reply);
            Box::pin(async move {
                receive
                    .await
                    .map_err(|_| "Held selection ended".to_owned())?
            })
        });
        let pending_view = crate::text_flow::view_with_options(pending_prepared, options)?;
        self.preview_scroll.set_child(Some(&pending_view.widget));
        until("pending selection view mapped", || {
            pending_view.widget.is_mapped()
        })
        .await?;
        pending_view
            .widget
            .clipboard()
            .set_text("clipboard-before-cancelled-indexed-selection");
        let buffer = pending_view.widget.buffer();
        buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
        pending_view.widget.emit_copy_clipboard();
        until("selection resolver is waiting", || {
            !pending.borrow().is_empty()
        })
        .await?;
        let stale_factory = self.flow_options().unwrap().indexed_tables.unwrap();
        table.load(table.size.get());
        let serial = table.serial.get();
        let generation = self.state.borrow().generation;
        self.set_mode(Mode::Source);
        until("source switch", || {
            self.state.borrow().mode == Mode::Source && !self.state.borrow().busy
        })
        .await?;
        until("unmap cancels pending selection", || {
            !pending_view.copying.get() && pending.borrow().iter().all(|s| s.is_closed())
        })
        .await?;
        if pending_view
            .widget
            .clipboard()
            .read_text_future()
            .await
            .map_err(|e| e.to_string())?
            .as_deref()
            != Some("clipboard-before-cancelled-indexed-selection")
        {
            return Err("A cancelled indexed selection replaced the clipboard".into());
        }
        if !self.flow_tables.borrow().is_empty()
            || table.active.get()
            || table.rows.n_items() != 0
            || table.current_reader().is_some()
        {
            return Err("Source switching retained indexed flow rows/ownership".into());
        }
        table.load(0);
        if table.serial.get() != serial {
            return Err("Expired flow table sent another request".into());
        }
        let status = self.status.text();
        self.event(Response {
            key: self.key,
            generation,
            serial,
            kind: "table",
            result: Err("STALE INDEXED TABLE ERROR".into()),
        });
        if self.status.text() != status {
            return Err("Stale table error changed the new view".into());
        }
        self.set_mode(Mode::Preview);
        until("ordinary preview restored", || {
            self.state.borrow().mode == Mode::Preview && !self.state.borrow().busy
        })
        .await?;
        if stale_factory(0, 0).is_ok() || table.rows.n_items() != 0 {
            return Err("Old indexed factory survived a reader mode epoch".into());
        }
        self.close();
        if hash(path).await? != original {
            return Err("Flow table test modified its report".into());
        }
        Ok(
            serde_json::json!({"ok":true,"experimentalAdapter":true,"sourceBytes":file_bytes,"productionTableWidget":true,"contextFragments":2,"realIndexPages":true,"coalescedTable":true,"joinedRows":joined_rows,"joinedSourceRange":true,
            "rowLimit":50,"initialRows":initial_rows,"columns":initial_columns,"columnFiltering":true,"columnWidths":true,
            "paging":true,"outerScrollStable":true,"cellCopy":true,"cellSourceComment":true,"wholeTableCopy":true,"wholeTablePrimary":true,"commentBudgetChecked":true,"commentLimitRejected":comment_budget,"selectionUnmapCancelled":true,"wholeTableBytes":whole.text.len(),
            "modeEpochRejected":true,"staleReplyRejected":true,"rowsReleased":true,"fileUnchanged":true}),
        )
    }
}

/// Read two actual index-page ranges from the private report copy, borrowing
/// their original header exactly as the gateway does. The resulting cells are
/// compared with cells independently parsed from the gateway response above.
fn table_context(
    path: &Path,
    block: &Block,
    document: &str,
    revision: &str,
    pages: &[(usize, crate::document::Page)],
) -> Result<Vec<studio_markdown_native::Fragment>, String> {
    use std::io::{Read, Seek, SeekFrom};
    let BlockKind::Table { id, rows, .. } = &block.kind else {
        return Err("Expected real table block".into());
    };
    let header_end = rows
        .first()
        .ok_or("Real table has no header/body boundary")?
        .source
        .start;
    let end = pages.last().ok_or("Real table context is empty")?.1.end;
    let bytes = end
        .checked_sub(block.source.start)
        .filter(|n| *n <= studio_markdown_native::flow::CONTEXT_BYTES)
        .ok_or("Real table source exceeds the private fixture budget")?;
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    file.seek(SeekFrom::Start(block.source.start as u64))
        .map_err(|e| e.to_string())?;
    let mut data = String::new();
    file.take(bytes as u64)
        .read_to_string(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() != bytes {
        return Err("Real table changed while reading source pages".into());
    }
    let window = |start: usize, end: usize, line: usize| -> Result<SourceWindow, String> {
        let a = start
            .checked_sub(block.source.start)
            .ok_or("Source page precedes its table")?;
        let b = end
            .checked_sub(block.source.start)
            .ok_or("Source page precedes its table")?;
        let text = data
            .get(a..b)
            .ok_or("Real table has invalid UTF-8 source coordinates")?;
        Ok(SourceWindow {
            start,
            end,
            line,
            end_line: line + text.bytes().filter(|b| *b == b'\n').count()
                - usize::from(text.ends_with('\n')),
            text: text.into(),
            projection: vec![],
            references: vec![],
        })
    };
    let header = window(block.source.start, header_end, block.source.line)?;
    pages
        .iter()
        .map(|(page, meta)| {
            Ok(studio_markdown_native::Fragment {
                page: *page,
                document_id: document.into(),
                revision: revision.into(),
                metadata: studio_markdown_native::FragmentMetadata {
                    start: meta.start,
                    end: meta.end,
                    line: meta.line,
                    rows: meta.rows,
                    row_start: meta.row_start,
                    table: *id,
                },
                source: window(meta.start, meta.end, meta.line)?,
                header: (meta.start >= header_end).then(|| header.clone()),
            })
        })
        .collect()
}
