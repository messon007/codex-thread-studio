//! Actual mapped error details and native acknowledgment, shared by write fixtures.
use super::*;
impl Queue {
    pub fn smoke_count(&self) -> usize {
        self.waiting.borrow().len() + usize::from(self.visible.borrow().is_some())
    }
    pub async fn smoke_wait(
        &self,
        source: &str,
        destination: &str,
    ) -> Result<adw::AlertDialog, String> {
        let at = std::time::Instant::now();
        loop {
            let alert = self.visible.borrow().clone();
            if let Some(alert) = alert.filter(|a| a.is_mapped()) {
                fn details(w: &gtk::Widget) -> Option<gtk::Label> {
                    if let Some(label) = w.downcast_ref::<gtk::Label>().filter(|l| {
                        l.widget_name() == "studio-write-failure-details"
                            && l.is_mapped()
                            && l.height() > 0
                    }) {
                        return Some(label.clone());
                    }
                    let mut child = w.first_child();
                    while let Some(w) = child {
                        child = w.next_sibling();
                        if let Some(l) = details(&w) {
                            return Some(l);
                        }
                    }
                    None
                }
                if let Some(details) = details(alert.upcast_ref())
                    .filter(|_| Self::smoke_close_button(alert.upcast_ref()).is_some())
                {
                    if alert.heading().as_deref() != Some(&crate::language::tr("Export failed"))
                        || alert.body()
                            != crate::language::format(
                                "Could not export “{source}”.",
                                &[("source", source)],
                            )
                        || !details.text().contains(destination)
                    {
                        return Err(format!("Export error lost its original context: heading={:?}, body={}, details={}",alert.heading(),alert.body(),details.text()));
                    }
                    if self
                        .parent
                        .upgrade()
                        .and_then(|p| p.visible_dialog())
                        .as_ref()
                        != Some(alert.upcast_ref())
                    {
                        return Err("Write error was hidden below another dialog".into());
                    }
                    // Observe the ordinary completed sheet transition. Do not
                    // emit frame-clock signals or allocate widgets manually.
                    glib::timeout_future(std::time::Duration::from_millis(350)).await;
                    return Ok(alert);
                }
            }
            if at.elapsed() > std::time::Duration::from_secs(15) {
                return Err(format!("No visible export failure for {destination}"));
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
    }
    pub fn smoke_ack(alert: &adw::AlertDialog) -> Result<(), String> {
        Self::smoke_close_button(alert.upcast_ref())
            .ok_or("Export failure Close button missing")?
            .emit_clicked();
        Ok(())
    }
    pub async fn smoke_capture(&self, path: &std::path::Path) -> Result<(), String> {
        let parent = self
            .parent
            .upgrade()
            .ok_or("Export error parent window missing")?;
        crate::window_capture::capture(&parent, path).await
    }
    fn smoke_close_button(w: &gtk::Widget) -> Option<gtk::Button> {
        if let Some(b) = w.downcast_ref::<gtk::Button>().filter(|b| {
            b.label().as_deref() == Some(&crate::language::tr("Close")) && b.is_mapped()
        }) {
            return Some(b.clone());
        }
        let mut child = w.first_child();
        while let Some(w) = child {
            child = w.next_sibling();
            if let Some(b) = Self::smoke_close_button(&w) {
                return Some(b);
            }
        }
        None
    }
    pub async fn smoke_clear(&self) -> Result<(), String> {
        let at = std::time::Instant::now();
        while self.blocks_close() {
            if at.elapsed() > std::time::Duration::from_secs(15) {
                return Err("Acknowledged export errors still block close".into());
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        Ok(())
    }
}
