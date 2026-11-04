//! Renewable presentation scopes. Old transfers stay cancelled after remapping.
use gtk::{gio, prelude::*};
use std::cell::RefCell;
pub struct Scope {
    current: RefCell<gio::Cancellable>,
}
impl Scope {
    pub fn new() -> Self {
        Self {
            current: RefCell::new(gio::Cancellable::new()),
        }
    }
    pub fn lease(&self) -> Option<gio::Cancellable> {
        let current = self.current.borrow().clone();
        (!current.is_cancelled()).then_some(current)
    }
    pub fn resume(&self) {
        if self.current.borrow().is_cancelled() {
            self.current.replace(gio::Cancellable::new());
        }
    }
    pub fn cancel(&self) {
        let current = self.current.borrow().clone();
        current.cancel();
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        self.current.get_mut().cancel();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resumed_scope_keeps_old_transfers_cancelled() {
        let scope = Scope::new();
        let old = scope.lease().unwrap();
        scope.cancel();
        assert!(old.is_cancelled());
        assert!(scope.lease().is_none());
        scope.resume();
        let new = scope.lease().unwrap();
        assert_ne!(new, old);
        assert!(old.is_cancelled());
        assert!(!new.is_cancelled());
        drop(scope);
        assert!(new.is_cancelled());
    }
    #[test]
    fn duplicate_mapping_does_not_replace_an_active_scope() {
        let scope = Scope::new();
        let current = scope.lease().unwrap();
        scope.resume();
        assert_eq!(scope.lease().unwrap(), current);
        assert!(!current.is_cancelled());
    }
}
