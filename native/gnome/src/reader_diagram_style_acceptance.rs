use super::*;
impl Reader {
    pub fn smoke_diagram_signature(&self) -> Option<serde_json::Value> {
        self.state
            .borrow()
            .diagrams
            .values()
            .find(|d| d.kind == "mermaid")
            .and_then(|d| d.picture.smoke_signature())
    }
    pub async fn smoke_diagram_dirty_begin(self: &Rc<Self>) -> Result<String, String> {
        self.set_mode(Mode::Edit);
        let at = std::time::Instant::now();
        while self.state.borrow().busy || self.state.borrow().editor.is_none() {
            if at.elapsed() > Duration::from_secs(15) {
                return Err("Diagram document Edit did not load".into());
            }
            glib::timeout_future(Duration::from_millis(20)).await;
        }
        let buffer = self.edit_view.buffer();
        buffer.insert(&mut buffer.end_iter(), "\nUnsaved diagram note 中文😀\n");
        if !self.state.borrow().dirty {
            return Err("Diagram document did not retain its unsaved edit".into());
        }
        Ok(buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string())
    }
    pub fn smoke_diagram_dirty_check(&self, expected: &str) -> Result<(), String> {
        let buffer = self.edit_view.buffer();
        if !self.state.borrow().dirty
            || buffer.text(&buffer.start_iter(), &buffer.end_iter(), false) != expected
        {
            return Err("Diagram settings changed the unsaved editor buffer".into());
        }
        Ok(())
    }
    pub fn smoke_diagram_preview(self: &Rc<Self>) {
        self.set_mode(Mode::Preview);
    }
    pub fn smoke_diagram_discard(&self) {
        self.finish_close();
    }
}
