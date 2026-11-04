use super::*;
impl Panel {
    pub(crate) fn smoke_snapshot(&self) -> Option<Arc<Snapshot>> {
        self.snapshot.borrow().clone()
    }
    pub(crate) fn smoke_idle(&self) -> bool {
        !self.busy.get()
    }
    pub(crate) fn smoke_create(&self, goal: &str) -> Result<(), String> {
        self.create.emit_clicked();
        let dialog = self
            .dialog
            .borrow()
            .clone()
            .ok_or("Create Map dialog did not open")?;
        let body = dialog
            .child()
            .and_downcast::<adw::ToolbarView>()
            .and_then(|t| t.content())
            .and_downcast::<gtk::Box>()
            .ok_or("Missing native creation form")?;
        let entry = body
            .first_child()
            .and_downcast::<gtk::Entry>()
            .ok_or("Missing goal field")?;
        entry.set_text(goal);
        let mut child = body.first_child();
        let mut submit = None;
        while let Some(widget) = child {
            if let Ok(b) = widget.clone().downcast::<gtk::Button>() {
                submit = Some(b);
            }
            child = widget.next_sibling();
        }
        submit
            .ok_or("Missing Create Map submit button")?
            .emit_clicked();
        Ok(())
    }
    pub(crate) fn smoke_topic(&self, title: &str, done: bool) -> Result<(), String> {
        fn find(widget: &gtk::Widget, title: &str) -> Option<gtk::Button> {
            if let Some(b) = widget.downcast_ref::<gtk::Button>() {
                if b.child()
                    .and_downcast::<gtk::Label>()
                    .is_some_and(|l| l.text() == title)
                {
                    return Some(b.clone());
                }
            }
            let mut child = widget.first_child();
            while let Some(c) = child {
                if let Some(b) = find(&c, title) {
                    return Some(b);
                }
                child = c.next_sibling();
            }
            None
        }
        let button =
            find(self.widget.upcast_ref(), title).ok_or("Map topic button is not realized")?;
        let button = if done {
            button
                .next_sibling()
                .and_downcast::<gtk::Button>()
                .ok_or("Missing topic completion button")?
        } else {
            button
        };
        if !button.is_sensitive() {
            return Err("Map topic action is disabled".into());
        }
        button.emit_clicked();
        Ok(())
    }
    pub(crate) fn smoke_generate(&self) {
        self.generate.emit_clicked();
    }
    pub(crate) fn smoke_undo(&self) {
        self.undo.emit_clicked();
    }
    pub(crate) fn smoke_cancel(&self) {
        self.cancel.emit_clicked();
    }
    pub(crate) fn smoke_delete(&self) {
        self.delete.emit_clicked();
    }
    pub(crate) fn smoke_status(&self) -> String {
        self.status.text().into()
    }
    pub(crate) fn smoke_writable(&self) -> bool {
        self.generate.is_sensitive() && !self.busy.get() && !self.uncertain.get()
    }
}
