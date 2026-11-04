//! In-memory views only. Backend history remains authoritative; no drafts are
//! written to disk, and eviction never deletes a backend conversation.
use crate::{
    byte_cache::{ByteCache, Weight},
    model::{Item, Thread},
};
use std::{num::NonZeroUsize, sync::Arc};
pub struct View {
    pub backend: String,
    pub thread: Thread,
    pub generation: u64,
    pub history_generation: u64,
    pub history_changes: crate::history_state::Changes,
    pub history_verified: bool,
    pub items: Vec<Arc<Item>>,
    pub active_turn: Option<String>,
    pub loading: bool,
    pub sending: bool,
    pub valid: bool,
    pub session_model: String,
    pub session_tier: String,
}
impl Weight for View {
    fn weight(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.backend.capacity()
            + self.thread.id.capacity()
            + self.thread.parent.as_ref().map_or(0, String::capacity)
            + self.thread.title.capacity()
            + self.thread.cwd.capacity()
            + self.session_model.capacity()
            + self.session_tier.capacity()
            + self.history_changes.weight()
            + self.items.capacity() * std::mem::size_of::<Arc<Item>>()
            + self.items.iter().map(|item| item.weight()).sum::<usize>()
    }
}
pub struct Cache(pub ByteCache<String, View>);
impl Default for Cache {
    fn default() -> Self {
        Self(ByteCache::new(
            NonZeroUsize::new(16).unwrap(),
            64 * 1024 * 1024,
        ))
    }
}
impl Cache {
    pub fn invalidate(&mut self, backend: &str) {
        self.0.for_each(|_, view| {
            if view.backend == backend {
                view.valid = false;
                view.loading = false;
                view.sending = false;
                view.history_verified = false;
                view.history_changes.keep_active = false;
            }
        });
    }
}
