//! Ownership for an asynchronous native chooser. Invalidate before cancellation:
//! a late response must neither open a file nor clear a newer pending chooser.
use gtk::{gio, prelude::*};
use std::cell::RefCell;

#[derive(Default)]
pub struct Pending(RefCell<Option<gio::Cancellable>>);

impl Pending {
    pub fn is_pending(&self) -> bool {
        self.0.borrow().is_some()
    }
    pub fn begin(&self) -> Option<gio::Cancellable> {
        if self.is_pending() {
            return None;
        }
        let request = gio::Cancellable::new();
        self.0.replace(Some(request.clone()));
        Some(request)
    }
    pub fn complete(&self, request: &gio::Cancellable) -> bool {
        if self.0.borrow().as_ref() != Some(request) {
            return false;
        }
        self.0.borrow_mut().take();
        true
    }
    pub fn cancel(&self) {
        let pending = self.0.borrow_mut().take();
        if let Some(request) = pending {
            request.cancel();
        }
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        if let Some(request) = self.0.get_mut().take() {
            request.cancel();
        }
    }
}
