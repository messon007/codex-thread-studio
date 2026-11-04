//! Common admission before handing source to GtkSourceView's UI-thread regex
//! engine. Reading/formatting/indexing remains worker-owned; GTK keeps the text.
use sourceview::prelude::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
#[derive(Default)]
struct State {
    language: RefCell<Option<sourceview::Language>>,
    changing: Cell<bool>,
    queued: Cell<bool>,
}
impl State {
    fn detach(&self, source: &sourceview::Buffer) {
        self.changing.set(true);
        source.set_language(None);
        self.changing.set(false);
        source.set_highlight_syntax(false);
    }
    fn admit(self: &Rc<Self>, source: &sourceview::Buffer) {
        if !allowed(source.upcast_ref()) {
            // highlight-syntax only controls paint. A retained language still
            // runs its context engine (for brackets/context classes) on GTK.
            self.detach(source);
        } else if !self.queued.replace(true) {
            let weak = source.downgrade();
            let state = self.clone();
            gtk::glib::idle_add_local_once(move || {
                state.queued.set(false);
                let Some(source) = weak.upgrade() else {
                    return;
                };
                if !allowed(source.upcast_ref()) {
                    state.detach(&source);
                    return;
                }
                let language = state.language.borrow().clone();
                state.changing.set(true);
                source.set_language(language.as_ref());
                state.changing.set(false);
                source.set_highlight_syntax(true);
            });
        }
    }
}
fn allowed(buffer: &gtk::TextBuffer) -> bool {
    if buffer.char_count() as usize > studio_markdown_native::text::HIGHLIGHT_BYTES {
        false
    } else {
        let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
        studio_markdown_native::text::highlight_safe(&text)
    }
}
pub fn bind(buffer: &gtk::TextBuffer) {
    // GTK owns edits. Inspect only bounded live buffers before its idle regex
    // work starts; larger buffers need no text copy to disable highlighting.
    let state = Rc::new(State::default());
    if let Some(source) = buffer.downcast_ref::<sourceview::Buffer>() {
        state.language.replace(source.language());
        let state = state.clone();
        source.connect_language_notify(move |source| {
            if !state.changing.get() {
                state.language.replace(source.language());
                state.admit(source);
            }
        });
    }
    buffer.connect_changed(move |buffer| {
        let Some(source) = buffer.downcast_ref::<sourceview::Buffer>() else {
            return;
        };
        // set_text emits changed once for deletion and again for insertion.
        // Never re-enable on the temporary empty buffer between those events.
        state.admit(source);
    });
}
pub fn guard(buffer: &gtk::TextBuffer, text: &str) {
    if let Some(source) = buffer.downcast_ref::<sourceview::Buffer>() {
        source.set_highlight_syntax(studio_markdown_native::text::highlight_safe(text));
    }
}
pub fn set_text(buffer: &gtk::TextBuffer, text: &str) {
    if let Some(source) = buffer.downcast_ref::<sourceview::Buffer>() {
        source.set_highlight_syntax(false);
    }
    buffer.set_text(text);
    guard(buffer, text);
}
