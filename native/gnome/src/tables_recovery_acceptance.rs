use super::*;
use crate::acceptance_wait::until;
impl Tables {
    pub(crate) async fn smoke_before_recovery(self: &Rc<Self>) -> Result<Value, String> {
        until("table open", || {
            self.state.borrow().id.is_some() && self.state.borrow().page.is_some()
        })
        .await?;
        self.sql_mode.set_active(true);
        if std::env::var("STUDIO_GNOME_SMOKE_TABLE_RECOVERY").as_deref() == Ok("xlsx") {
            if self.state.borrow().sheets.len() != 3 {
                return Err("Recovery workbook has no three worksheets".into());
            }
            self.set_sql("SELECT n, label, note FROM data ORDER BY CAST(n AS INTEGER) ASC");
            self.run_query();
            until("first worksheet query", || {
                self.state.borrow().query_id.is_some()
            })
            .await?;
            self.editor
                .buffer()
                .set_text("SELECT label FROM data -- unsent first-sheet draft 中文😀");
            self.sheet.set_selected(1);
            if self.state.borrow().query_id.is_some() || self.rows.n_items() != 0 {
                return Err("Changing worksheets reused the old result query".into());
            }
        }
        self.set_sql("SELECT n, label, note FROM data ORDER BY CAST(n AS INTEGER) DESC");
        self.run_query();
        until("table query", || self.state.borrow().query_id.is_some()).await?;
        self.page(50);
        until("table second page", || {
            self.state
                .borrow()
                .page
                .as_ref()
                .is_some_and(|p| p.offset == 50)
        })
        .await?;
        let draft = "SELECT label FROM data WHERE n > 50 -- unsent recovery draft 中文😀";
        self.editor.buffer().set_text(draft);
        self.wrap.set_active(true);
        let column = self
            .grid
            .columns()
            .item(1)
            .and_downcast::<gtk::ColumnViewColumn>()
            .ok_or("Table second column missing")?;
        column.set_fixed_width(306);
        let state = self.state.borrow();
        let page = state.page.as_ref().unwrap();
        if state.sheets.len() == 3
            && (state.sheet != 1
                || page.rows[0][0].text != "1099"
                || !page.rows[0][1].text.contains("Sheet 2 row 99"))
        {
            return Err("Recovery workbook did not select the second worksheet's own rows".into());
        }
        Ok(
            json!({"id":state.id,"generation":state.generation,"serial":state.view_ticket,"offset":page.offset,"first":page.rows[0][0].text,"draft":draft,"width":column.fixed_width(),"sheet":state.sheet,"query":state.query_id}),
        )
    }
    pub(crate) fn smoke_offline_worksheet(&self, before: &Value) -> Result<(), String> {
        if std::env::var("STUDIO_GNOME_SMOKE_TABLE_RECOVERY").as_deref() != Ok("xlsx") {
            return Ok(());
        }
        if self.sheet.is_sensitive() || self.sheet.selected() != 1 {
            return Err(
                "Offline worksheet selector retained live input or changed selection".into(),
            );
        }
        self.sheet.set_selected(0);
        let state = self.state.borrow();
        if state.sheet != 1
            || self.sheet.selected() != 1
            || self.sql_text() != before["draft"].as_str().unwrap()
            || state
                .page
                .as_ref()
                .is_none_or(|p| p.offset != 50 || p.sheet != 1)
            || self.rows.n_items() == 0
        {
            return Err(
                "An offline worksheet callback discarded the retained query/page or draft".into(),
            );
        }
        Ok(())
    }
    async fn smoke_worksheet_drafts(&self, before: &Value) -> Result<(), String> {
        let first = "SELECT label FROM data -- unsent first-sheet draft 中文😀";
        let asc = "SELECT n, label, note FROM data ORDER BY CAST(n AS INTEGER) ASC";
        let desc = "SELECT n, label, note FROM data ORDER BY CAST(n AS INTEGER) DESC";
        self.sheet.set_selected(0);
        if self.sql_text() != first
            || self.state.borrow().query_id.is_some()
            || self.rows.n_items() != 0
            || self.state.borrow().executed_sql.get(&0).map(String::as_str) != Some(asc)
        {
            return Err("First worksheet draft/query was lost or executed on selection".into());
        }
        self.sheet.set_selected(1);
        if self.sql_text() != before["draft"].as_str().unwrap()
            || self.state.borrow().query_id.is_some()
            || self.rows.n_items() != 0
            || self.state.borrow().executed_sql.get(&1).map(String::as_str) != Some(desc)
        {
            return Err("Second worksheet draft/query was lost or executed on selection".into());
        }
        // Explicit user Query, followed by restoring the separate editor draft.
        self.set_sql(desc);
        self.run_query();
        until("query after worksheet round trip", || {
            self.state.borrow().query_id.is_some()
                && self
                    .state
                    .borrow()
                    .page
                    .as_ref()
                    .is_some_and(|p| p.sheet == 1 && p.offset == 0)
        })
        .await?;
        self.page(50);
        until("worksheet result page after explicit query", || {
            self.state
                .borrow()
                .page
                .as_ref()
                .is_some_and(|p| p.sheet == 1 && p.offset == 50)
        })
        .await?;
        self.editor
            .buffer()
            .set_text(before["draft"].as_str().unwrap());
        Ok(())
    }
    pub(crate) async fn smoke_after_recovery(
        self: &Rc<Self>,
        before: &Value,
        conflict: bool,
    ) -> Result<Value, String> {
        until("table recovery", || {
            let state = self.state.borrow();
            !state.resume_pending
                && if conflict {
                    state.id.is_none() && state.recovering
                } else {
                    state.id.is_some() && !state.recovering
                }
        })
        .await?;
        if self.state.borrow().sheet != before["sheet"].as_u64().unwrap() as usize
            || self.sheet.selected() as usize != self.state.borrow().sheet
        {
            return Err("Table recovery changed the selected worksheet".into());
        }
        if self.sql_text() != before["draft"].as_str().unwrap() || !self.wrap.is_active() {
            return Err("Table recovery changed the SQL draft or Wrap".into());
        }
        if conflict {
            if self.query.is_sensitive() || !self.status.text().contains("changed") {
                return Err("Table conflict did not disable server operations".into());
            }
        } else {
            let state = self.state.borrow();
            let page = state.page.as_ref().ok_or("Recovered table page missing")?;
            if state.id.as_deref() == before["id"].as_str()
                || page.offset != 50
                || page.rows[0][0].text != before["first"].as_str().unwrap()
                || !state.sql_mode
                || state.query_id.is_none()
                || state.query_id.as_deref() == before["query"].as_str()
            {
                return Err(
                    "Table recovery changed the executed query/page or reused its old lease".into(),
                );
            }
        }
        let workbook = std::env::var("STUDIO_GNOME_SMOKE_TABLE_RECOVERY").as_deref() == Ok("xlsx");
        if workbook {
            if self.state.borrow().sheets.len() != 3
                || self.state.borrow().sheet != 1
                || before["first"].as_str() != Some("1099")
            {
                return Err("Workbook recovery has the wrong worksheet or rows".into());
            }
            if !conflict {
                self.smoke_worksheet_drafts(before).await?;
            }
        }
        let column = self
            .grid
            .columns()
            .item(1)
            .and_downcast::<gtk::ColumnViewColumn>()
            .unwrap();
        if column.fixed_width() != 306 {
            return Err("Table recovery reset column widths".into());
        }
        self.event(Response {
            generation: before["generation"].as_u64().unwrap(),
            serial: before["serial"].as_u64().unwrap(),
            kind: "page",
            result: Err("STALE TABLE REPLY".into()),
        });
        if self.status.text().contains("STALE") {
            return Err("Retired table reply reached the new view".into());
        }
        Ok(
            json!({"snapshotRevalidated":!conflict,"conflictProtected":conflict,"sqlDraftRetained":true,"executedQueryRetained":true,"pageRetained":true,"columnWidthRetained":true,"wrapRetained":true,"staleReplyRejected":true,"worksheetRetained":true,"workbook":workbook,"perSheetDraftsRetained":workbook&&!conflict,"worksheetCallbacksRetired":workbook}),
        )
    }
}
