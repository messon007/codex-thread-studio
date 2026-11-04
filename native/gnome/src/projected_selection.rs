//! Primary selection and scoped drag export for projected child-anchor buffers.
//! GTK still owns selection mechanics. Temporarily omit its raw provider while
//! running the parent buffer handlers, then export an immutable lazy projection.
//! Drag offers share extraction/lifetime checks independently of primary
//! clipboard ownership; indexed-table text resolves on bounded workers.
use super::*;
use gtk::{gdk::subclass::prelude::*, subclass::prelude::*};
use std::{cell::OnceCell, ops::Range};

struct State {
    view: glib::WeakRef<gtk::TextView>,
    buffer: glib::WeakRef<gtk::TextBuffer>,
    presentation: std::rc::Weak<crate::syntax_ui::Presentation>,
    source: Arc<flow::Projection>,
    tables: Option<SelectionTables>,
    valid: Cell<bool>,
    suppress: Cell<bool>,
    offer: RefCell<Option<Provider>>,
    scope: selection_scope::Scope,
}
impl State {
    fn provider(self: &Rc<Self>, range: Range<usize>, primary: bool) -> Provider {
        let provider: Provider = glib::Object::new();
        provider
            .imp()
            .offer
            .set(Offer {
                state: Rc::downgrade(self),
                range,
                primary,
                scope: self.scope.lease().expect("Mounted selection scope"),
            })
            .ok()
            .unwrap();
        provider
    }
    fn view(&self) -> Option<gtk::TextView> {
        self.view.upgrade().filter(|v| {
            self.valid.get()
                && v.is_mapped()
                && self.scope.lease().is_some()
                && self.buffer.upgrade().is_some_and(|b| v.buffer() == b)
                && self.presentation.upgrade().is_some_and(|p| p.is_current())
        })
    }
    fn withdraw(&self) {
        let offer = self.offer.borrow_mut().take();
        if let Some((view, offer)) = self.view.upgrade().zip(offer) {
            let clipboard = view.primary_clipboard();
            if clipboard.content().as_ref() == Some(offer.upcast_ref()) {
                let _ = clipboard.set_content(None::<&gdk::ContentProvider>);
            }
        }
    }
    fn invalidate(&self) {
        self.valid.set(false);
        self.scope.cancel();
        if let Some(tables) = &self.tables {
            tables.cancel.cancel();
        }
        self.withdraw();
    }
    fn publish(self: &Rc<Self>) {
        if self.suppress.get() {
            return;
        }
        let Some(view) = self.view() else {
            self.withdraw();
            return;
        };
        let Some((start, end)) = view.buffer().selection_bounds() else {
            self.withdraw();
            return;
        };
        let range = start.offset() as usize..end.offset() as usize;
        // GTK emits two mark notifications for a single selection operation.
        if self
            .offer
            .borrow()
            .as_ref()
            .is_some_and(|p| p.imp().offer.get().unwrap().range == range)
            && view.primary_clipboard().content().as_ref()
                == self.offer.borrow().as_ref().map(|p| p.upcast_ref())
        {
            return;
        }
        let provider = self.provider(range, true);
        *self.offer.borrow_mut() = Some(provider.clone());
        if view
            .primary_clipboard()
            .set_content(Some(&provider))
            .is_err()
        {
            self.offer.borrow_mut().take();
        }
    }
}
struct Offer {
    state: std::rc::Weak<State>,
    range: Range<usize>,
    primary: bool,
    scope: gio::Cancellable,
}
mod provider {
    use super::*;
    #[derive(Default)]
    pub struct Provider {
        pub(super) offer: OnceCell<Offer>,
    }
    #[glib::object_subclass]
    impl ObjectSubclass for Provider {
        const NAME: &'static str = "StudioProjectedSelectionProvider";
        type Type = super::Provider;
        type ParentType = gdk::ContentProvider;
    }
    impl ObjectImpl for Provider {}
    impl ContentProviderImpl for Provider {
        fn formats(&self) -> gdk::ContentFormats {
            gdk::ContentFormats::builder()
                .add_mime_type("text/plain;charset=utf-8")
                .add_mime_type("text/plain")
                .build()
        }
        fn write_mime_type_future(
            &self,
            mime: &str,
            stream: &gio::OutputStream,
            priority: glib::Priority,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), glib::Error>> + 'static>>
        {
            let provider = self.obj().clone();
            let stream = stream.clone();
            let supported = matches!(mime, "text/plain;charset=utf-8" | "text/plain");
            Box::pin(async move {
                let error = |message: &str| glib::Error::new(gio::IOErrorEnum::Failed, message);
                if !supported {
                    return Err(error("Unsupported selection format"));
                }
                let offer = provider.imp().offer.get().unwrap();
                let valid = || {
                    let state = offer.state.upgrade()?;
                    let view = state.view()?;
                    let (start, end) = view.buffer().selection_bounds()?;
                    ((!offer.scope.is_cancelled()
                        && (!offer.primary
                            || view.primary_clipboard().content().as_ref()
                                == Some(provider.upcast_ref())))
                        && (start.offset() as usize..end.offset() as usize) == offer.range)
                        .then_some(state)
                };
                let state = valid().ok_or_else(|| error("Selection is no longer current"))?;
                let selected = scoped_text(
                    state.source.clone(),
                    offer.range.clone(),
                    flow::BUFFER_BYTES,
                    state.tables.clone(),
                    offer.scope.clone(),
                )
                .await
                .map_err(|e| error(&e))?;
                valid().ok_or_else(|| error("Selection changed during transfer"))?;
                let (_, _, failure) = gio::CancellableFuture::new(
                    stream.write_all_future(selected.text.into_bytes(), priority),
                    offer.scope.clone(),
                )
                .await
                .map_err(|_| error("Selection view is no longer visible"))?
                .map_err(|(_, e)| e)?;
                if let Some(error) = failure {
                    return Err(error);
                }
                Ok(())
            })
        }
    }
}
glib::wrapper! {
    pub struct Provider(ObjectSubclass<provider::Provider>) @extends gdk::ContentProvider;
}
mod buffer {
    use super::*;
    #[derive(Default)]
    pub struct Buffer {
        pub(super) state: OnceCell<Rc<State>>,
        depth: Cell<u32>,
    }
    #[glib::object_subclass]
    impl ObjectSubclass for Buffer {
        const NAME: &'static str = "StudioProjectedSelectionBuffer";
        type Type = super::Buffer;
        type ParentType = gtk::TextBuffer;
    }
    impl ObjectImpl for Buffer {}
    impl Buffer {
        fn without_raw_primary(&self, publish: bool, run: impl FnOnce()) {
            let outer = self.depth.get() == 0;
            self.depth.set(self.depth.get() + 1);
            // GtkTextView registers exactly once on realize, and balances that
            // registration on buffer replacement/unrealize. Keep it balanced
            // outside these synchronous parent handlers as well.
            let clipboard = outer
                .then(|| self.state.get())
                .flatten()
                .and_then(|s| s.view.upgrade())
                .filter(|v| v.is_realized() && v.buffer() == *self.obj())
                .map(|v| v.primary_clipboard());
            if let Some(clipboard) = &clipboard {
                self.obj().remove_selection_clipboard(clipboard);
            }
            run();
            if let Some(clipboard) = &clipboard {
                self.obj().add_selection_clipboard(clipboard);
            }
            self.depth.set(self.depth.get() - 1);
            if outer && publish {
                if let Some(state) = self.state.get() {
                    state.publish();
                }
            }
        }
        fn invalidate(&self) {
            if let Some(state) = self.state.get() {
                state.invalidate();
            }
        }
    }
    impl TextBufferImpl for Buffer {
        fn mark_set(&self, iter: &gtk::TextIter, mark: &gtk::TextMark) {
            if matches!(mark.name().as_deref(), Some("insert" | "selection_bound")) {
                self.without_raw_primary(true, || self.parent_mark_set(iter, mark));
            } else {
                self.parent_mark_set(iter, mark);
            }
        }
        fn insert_text(&self, iter: &mut gtk::TextIter, text: &str) {
            self.invalidate();
            self.without_raw_primary(false, || self.parent_insert_text(iter, text));
        }
        fn delete_range(&self, start: &mut gtk::TextIter, end: &mut gtk::TextIter) {
            self.invalidate();
            self.without_raw_primary(false, || self.parent_delete_range(start, end));
        }
    }
}
glib::wrapper! {
    pub struct Buffer(ObjectSubclass<buffer::Buffer>) @extends gtk::TextBuffer;
}
impl Buffer {
    pub fn scope(&self) -> Option<gio::Cancellable> {
        let state = self.imp().state.get()?;
        state.view()?;
        state.scope.lease()
    }
    /// Lazy drag data uses the same bounded Rust extraction as primary selection,
    /// with its own identity: another client may own the primary clipboard.
    pub fn drag_provider(&self) -> Option<gdk::ContentProvider> {
        let state = self.imp().state.get()?;
        state.view()?;
        let (a, b) = self.selection_bounds()?;
        Some(
            state
                .provider(a.offset() as usize..b.offset() as usize, false)
                .upcast(),
        )
    }
    pub fn projection(&self) -> Option<Arc<flow::Projection>> {
        let state = self.imp().state.get()?;
        state.view()?;
        Some(state.source.clone())
    }
    pub fn owns_primary(&self) -> bool {
        let Some(state) = self.imp().state.get() else {
            return false;
        };
        state.view.upgrade().is_some_and(|v| {
            v.primary_clipboard().content().as_ref()
                == state.offer.borrow().as_ref().map(|p| p.upcast_ref())
        }) && state.offer.borrow().is_some()
    }
    pub fn select_without_primary(&self, a: &gtk::TextIter, b: &gtk::TextIter) {
        if let Some(state) = self.imp().state.get() {
            state.suppress.set(true);
            self.select_range(a, b);
            state.suppress.set(false);
        }
    }
    pub fn quick_selection(&self, limit: usize) -> Option<String> {
        let state = self.imp().state.get()?;
        state.view()?;
        let (a, b) = self.selection_bounds()?;
        state
            .source
            .quick
            .as_ref()?
            .select(
                a.offset() as usize..b.offset() as usize,
                limit.min(flow::RICH_TEXT_BYTES),
            )
            .map(str::to_owned)
    }
    pub fn new() -> Self {
        glib::Object::new()
    }
    pub fn attach(
        &self,
        view: &gtk::TextView,
        presentation: &Rc<crate::syntax_ui::Presentation>,
        source: Arc<flow::Projection>,
        tables: Option<SelectionTables>,
    ) {
        let state = Rc::new(State {
            view: view.downgrade(),
            buffer: self.upcast_ref::<gtk::TextBuffer>().downgrade(),
            presentation: Rc::downgrade(presentation),
            source,
            tables,
            valid: Cell::new(true),
            suppress: Cell::new(false),
            offer: RefCell::new(None),
            scope: selection_scope::Scope::new(),
        });
        self.imp().state.set(state.clone()).ok().unwrap();
        let changed = state.clone();
        self.connect_changed(move |_| changed.invalidate());
        let mapped = state.clone();
        view.connect_map(move |_| {
            if mapped.valid.get() {
                mapped.scope.resume();
                mapped.publish();
            }
        });
        let unmapped = state.clone();
        view.connect_unmap(move |_| {
            unmapped.scope.cancel();
            unmapped.withdraw();
        });
        view.connect_buffer_notify(move |view| {
            if state.buffer.upgrade().is_none_or(|b| b != view.buffer()) {
                state.invalidate();
            }
        });
    }
}
