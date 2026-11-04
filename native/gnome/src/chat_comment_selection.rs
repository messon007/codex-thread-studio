//! Bind mounted native text widgets to the reply's current identity.
use super::{CommentAction, Item};
use std::{rc::Rc, sync::Arc};

pub(super) fn bind(widget: &gtk::Widget, item: &Arc<Item>, capture: &CommentAction) {
    let item = item.clone();
    let capture = capture.clone();
    crate::selection_menu::bind(
        widget,
        "chat-comment",
        Rc::new(move |selection| {
            capture(item.clone(), selection);
        }),
    );
}
