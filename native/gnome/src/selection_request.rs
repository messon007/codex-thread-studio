//! Selection-owned asynchronous work. A retired request must neither keep a
//! reader busy nor clear a newer request when its future finally unwinds.
use adw::prelude::*;
use gtk::gio;
use std::{cell::RefCell, ops::Range, rc::Rc};

#[derive(Default)]
pub struct SelectionRequest {
    pending: RefCell<Option<(Range<usize>, gio::Cancellable)>>,
}
impl SelectionRequest {
    pub fn attach(view: &gtk::TextView) -> Rc<Self> {
        let state = Rc::new(Self::default());
        let weak = Rc::downgrade(&state);
        view.buffer().connect_mark_set(move |buffer, _, mark| {
            if *mark == buffer.get_insert() || *mark == buffer.selection_bound() {
                if let Some(state) = weak.upgrade() {
                    state.selection_changed(
                        buffer
                            .selection_bounds()
                            .map(|(a, b)| a.offset() as usize..b.offset() as usize),
                    );
                }
            }
        });
        let weak = Rc::downgrade(&state);
        view.buffer().connect_changed(move |_| {
            if let Some(state) = weak.upgrade() {
                state.cancel();
            }
        });
        let weak = Rc::downgrade(&state);
        view.connect_buffer_notify(move |_| {
            if let Some(state) = weak.upgrade() {
                state.cancel();
            }
        });
        let weak = Rc::downgrade(&state);
        view.connect_unmap(move |_| {
            if let Some(state) = weak.upgrade() {
                state.cancel();
            }
        });
        state
    }
    pub fn begin(&self, range: Range<usize>) -> Option<gio::Cancellable> {
        let mut pending = self.pending.borrow_mut();
        if pending.is_some() {
            return None;
        }
        let cancel = gio::Cancellable::new();
        *pending = Some((range, cancel.clone()));
        Some(cancel)
    }
    fn selection_changed(&self, range: Option<Range<usize>>) {
        if self
            .pending
            .borrow()
            .as_ref()
            .is_some_and(|(old, _)| Some(old) != range.as_ref())
        {
            self.cancel();
        }
    }
    fn cancel(&self) {
        let pending = self.pending.borrow_mut().take();
        if let Some((_, cancel)) = pending {
            cancel.cancel();
        }
    }
    pub fn finish(&self, cancel: &gio::Cancellable) {
        let mut pending = self.pending.borrow_mut();
        if pending.as_ref().is_some_and(|(_, active)| active == cancel) {
            pending.take();
        }
    }
}
impl Drop for SelectionRequest {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changed_selection_cancels_and_old_completion_preserves_new_request() {
        let state = SelectionRequest::default();
        let old = state.begin(1..20).unwrap();
        state.selection_changed(Some(1..20));
        assert!(!old.is_cancelled());
        assert!(state.begin(1..20).is_none());
        state.selection_changed(Some(2..20));
        assert!(old.is_cancelled());
        let new = state.begin(2..20).unwrap();
        state.finish(&old);
        assert!(state.begin(2..20).is_none());
        assert!(!new.is_cancelled());
        state.finish(&new);
        let next = state.begin(5..10).unwrap();
        drop(state);
        assert!(next.is_cancelled());
    }
    #[test]
    fn clearing_selection_releases_pending_work() {
        let state = SelectionRequest::default();
        let old = state.begin(0..8).unwrap();
        state.selection_changed(None);
        assert!(old.is_cancelled());
        assert!(state.begin(3..8).is_some());
    }
}
