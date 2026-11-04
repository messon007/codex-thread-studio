//! Preserve unchanged native model objects so paging keeps rows, focus and scroll anchors.
use gtk::{gio, glib, prelude::*};

pub fn reconcile<T: Clone + Eq + 'static>(store: &gio::ListStore, rows: &[&T]) {
    let old = store.n_items() as usize;
    let equal = |position: usize, row: &T| {
        store
            .item(position as u32)
            .and_downcast::<glib::BoxedAnyObject>()
            .is_some_and(|object| *object.borrow::<T>() == *row)
    };
    let mut first = 0;
    while first < old.min(rows.len()) && equal(first, rows[first]) {
        first += 1;
    }
    let mut suffix = 0;
    while suffix < old.min(rows.len()) - first
        && equal(old - suffix - 1, rows[rows.len() - suffix - 1])
    {
        suffix += 1;
    }
    if first == old && first == rows.len() {
        return;
    }
    let changed = rows[first..rows.len() - suffix]
        .iter()
        .map(|row| glib::BoxedAnyObject::new((*row).clone()))
        .collect::<Vec<_>>();
    store.splice(first as u32, (old - first - suffix) as u32, &changed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};
    #[test]
    fn append_and_filter_retain_unmodified_objects_and_equal_models_emit_no_change() {
        let store = gio::ListStore::new::<glib::BoxedAnyObject>();
        reconcile(&store, &[&"first".to_owned(), &"last".to_owned()]);
        let first = store.item(0).unwrap();
        let last = store.item(1).unwrap();
        let signals = Rc::new(Cell::new(0));
        let counter = signals.clone();
        store.connect_items_changed(move |_, _, _, _| counter.set(counter.get() + 1));
        reconcile(&store, &[&"first".to_owned(), &"last".to_owned()]);
        assert_eq!(signals.get(), 0);
        reconcile(
            &store,
            &[
                &"first".to_owned(),
                &"middle".to_owned(),
                &"last".to_owned(),
            ],
        );
        assert_eq!(store.item(0).unwrap(), first);
        assert_eq!(store.item(2).unwrap(), last);
        reconcile(&store, &[&"last".to_owned()]);
        assert_eq!(store.item(0).unwrap(), last);
        reconcile::<String>(&store, &[]);
        assert_eq!(store.n_items(), 0);
    }
}
