//! Bounded native filesystem jobs with window-close lifetime accounting.
use adw::prelude::*;
use gtk::{gio, glib};
use std::{
    cell::Cell,
    rc::Rc,
    sync::{Arc, OnceLock},
};
pub struct Jobs {
    pending: Cell<usize>,
    closing: Cell<bool>,
    parent: glib::WeakRef<adw::ApplicationWindow>,
}
impl Jobs {
    pub fn new(parent: &adw::ApplicationWindow) -> Rc<Self> {
        Rc::new(Self {
            pending: Cell::new(0),
            closing: Cell::new(false),
            parent: parent.downgrade(),
        })
    }
    pub fn can_start(&self) -> bool {
        self.pending.get() < 16 && !self.closing.get()
    }
    pub fn run<T: Send + 'static>(
        self: &Rc<Self>,
        work: impl FnOnce() -> Result<T, String> + Send + 'static,
        done: impl FnOnce(Result<T, String>) + 'static,
    ) {
        static LIMIT: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
        let limit = LIMIT
            .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(4)))
            .clone();
        self.pending.set(self.pending.get() + 1);
        let jobs = self.clone();
        glib::spawn_future_local(async move {
            let permit = limit
                .acquire_owned()
                .await
                .expect("Native file lane remains open");
            let result = gio::spawn_blocking(move || {
                let _permit = permit;
                work()
            })
            .await
            .unwrap_or_else(|_| Err("Native file worker stopped".into()));
            done(result); // Completion may enqueue cleanup; account for that first.
            jobs.pending.set(jobs.pending.get() - 1);
            if jobs.pending.get() == 0 && jobs.closing.get() {
                let jobs = jobs.clone();
                glib::idle_add_local_once(move || {
                    if jobs.pending.get() == 0 && jobs.closing.replace(false) {
                        if let Some(parent) = jobs.parent.upgrade() {
                            parent.close();
                        }
                    }
                });
            }
        });
    }
    pub fn discard<T: Send + 'static>(self: &Rc<Self>, value: T) {
        self.run(
            move || {
                drop(value);
                Ok(())
            },
            |_| {},
        );
    }
    pub fn allow_close(&self) -> bool {
        if self.pending.get() == 0 {
            true
        } else {
            self.closing.set(true);
            false
        }
    }
    pub fn cancel_close(&self) {
        self.closing.set(false);
    }
    #[cfg(debug_assertions)]
    pub fn idle(&self) -> bool {
        self.pending.get() == 0
    }
}
