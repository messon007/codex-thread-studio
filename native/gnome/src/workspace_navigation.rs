//! Bounded native pane history. Content remains owned by each viewer, never copied here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    Document,
    Source,
    Image,
    Pdf,
    Epub,
    Table,
    Browser,
    Files,
    Git,
    Terminal,
    Compare,
    Map,
    Resources,
}
impl Pane {
    pub fn name(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::Source => "source",
            Self::Image => "image",
            Self::Pdf => "pdf",
            Self::Epub => "epub",
            Self::Table => "table",
            Self::Browser => "browser",
            Self::Files => "files",
            Self::Git => "git",
            Self::Terminal => "terminal",
            Self::Compare => "compare",
            Self::Map => "map",
            Self::Resources => "resources",
        }
    }
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "document" => Self::Document,
            "source" => Self::Source,
            "image" => Self::Image,
            "pdf" => Self::Pdf,
            "epub" => Self::Epub,
            "table" => Self::Table,
            "browser" => Self::Browser,
            "files" => Self::Files,
            "git" => Self::Git,
            "terminal" => Self::Terminal,
            "compare" => Self::Compare,
            "map" => Self::Map,
            "resources" => Self::Resources,
            _ => return None,
        })
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum Closed {
    Ignore,
    Hide,
    Show(Pane),
}
#[derive(Default)]
pub struct Navigation {
    path: Vec<Pane>,
}
impl Navigation {
    pub fn active(&self) -> Option<Pane> {
        self.path.last().copied()
    }
    pub fn reset(&mut self) {
        self.path.clear();
    }
    /// Explicit workspace selection starts a new base; repeated presentation does not.
    pub fn select(&mut self, pane: Pane) {
        self.path.clear();
        self.path.push(pane);
    }
    pub fn open(&mut self, pane: Pane, current: Option<Pane>) {
        match current {
            None => self.reset(),
            Some(current) if self.active() != Some(current) => {
                if let Some(position) = self.path.iter().position(|p| *p == current) {
                    self.path.truncate(position + 1);
                } else {
                    self.select(current);
                }
            }
            _ => {}
        }
        // Returning to an ancestor collapses the chain instead of creating a cycle.
        if let Some(position) = self.path.iter().position(|p| *p == pane) {
            self.path.truncate(position + 1);
        } else {
            self.path.push(pane);
        }
    }
    pub fn close(
        &mut self,
        pane: Pane,
        current: Option<Pane>,
        available: impl Fn(Pane) -> bool,
    ) -> Closed {
        if current != Some(pane) {
            return Closed::Ignore;
        }
        if self.active() != Some(pane) {
            self.select(pane);
        }
        self.path.pop();
        while self.active().is_some_and(|parent| !available(parent)) {
            self.path.pop();
        }
        match self.active() {
            Some(parent) => Closed::Show(parent),
            None => Closed::Hide,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::{Closed::*, Pane::*, *};
    #[test]
    fn nested_viewers_return_without_changing_their_parent_state() {
        let mut nav = Navigation::default();
        nav.select(Files);
        nav.open(Document, Some(Files));
        nav.open(Image, Some(Document));
        nav.open(Pdf, Some(Image));
        assert_eq!(nav.close(Pdf, Some(Pdf), |_| true), Show(Image));
        assert_eq!(nav.close(Image, Some(Image), |_| true), Show(Document));
        assert_eq!(nav.close(Document, Some(Document), |_| true), Show(Files));
        assert_eq!(nav.close(Files, Some(Files), |_| true), Hide);
    }
    #[test]
    fn stale_closes_cannot_hide_the_new_pane_or_erase_its_origin() {
        let mut nav = Navigation::default();
        nav.select(Git);
        nav.open(Table, Some(Git));
        assert_eq!(nav.close(Git, Some(Table), |_| true), Ignore);
        assert_eq!(nav.close(Table, Some(Table), |_| true), Show(Git));
        nav.open(Epub, Some(Git));
        nav.select(Terminal);
        assert_eq!(nav.close(Epub, Some(Terminal), |_| true), Ignore);
        assert_eq!(nav.close(Terminal, Some(Terminal), |_| true), Hide);
    }
    #[test]
    fn reopening_an_ancestor_or_the_same_viewer_never_forms_a_cycle() {
        let mut nav = Navigation::default();
        nav.select(Git);
        for _ in 0..100 {
            nav.open(Document, Some(Git));
            nav.open(Epub, Some(Document));
            nav.open(Browser, Some(Epub));
            nav.open(Document, Some(Browser));
            nav.open(Document, Some(Document));
            assert_eq!(nav.path, [Git, Document]);
            assert_eq!(nav.close(Document, Some(Document), |_| true), Show(Git));
        }
    }
    #[test]
    fn closed_parents_are_skipped_and_hidden_workspace_history_is_not_reused() {
        let mut nav = Navigation::default();
        nav.select(Files);
        nav.open(Document, Some(Files));
        nav.open(Image, Some(Document));
        assert_eq!(
            nav.close(Image, Some(Image), |pane| pane != Document),
            Show(Files)
        );
        nav.open(Pdf, None);
        assert_eq!(nav.close(Pdf, Some(Pdf), |_| true), Hide);
    }
    #[test]
    fn external_selection_is_synchronized_and_names_are_only_known_panes() {
        let mut nav = Navigation::default();
        nav.select(Git);
        nav.open(Pdf, Some(Compare));
        assert_eq!(nav.close(Pdf, Some(Pdf), |_| true), Show(Compare));
        assert_eq!(Pane::from_name("pdf").unwrap().name(), "pdf");
        assert!(Pane::from_name("file:///private").is_none());
    }
}
