//! GTK admission and stale-result guards; parsing happens on the Rust CPU pool.
use super::*;
#[derive(Default)]
pub struct Book {
    context: Option<crate::streaming::Context>,
    pub(super) serial: u64,
    pub(super) pending: Option<u64>,
    started: Option<std::time::Instant>,
    failed: Option<Arc<Item>>,
}
impl Ui {
    pub(super) fn streaming_context(&self) -> Option<crate::streaming::Context> {
        let state = self.state.borrow();
        if !state.ready || state.loading || !state.history_verified || self.router_selected() {
            return None;
        }
        Some(crate::streaming::Context {
            backend: state.backend.clone(),
            socket: state.socket_generation,
            history: state.history_generation,
            thread: state.selected.as_ref()?.id.clone(),
        })
    }
    pub(super) fn streaming_tick(&self) {
        let context = self.streaming_context();
        let mut book = self.streaming.borrow_mut();
        if book.context != context {
            book.context = context.clone();
            book.pending = None;
            book.started = None;
            book.failed = None;
            let _ = self.sender.try_send(Command::Streaming(None));
        }
        let Some(context) = context else {
            return;
        };
        if book.pending.is_some()
            || self.history_pages.active()
            || book
                .started
                .is_some_and(|at| at.elapsed() < Duration::from_millis(150))
        {
            return;
        }
        let candidate = {
            let state = self.state.borrow();
            crate::streaming::candidate(
                &state.items,
                state.active_turn.as_deref(),
                book.failed.as_ref(),
            )
        };
        let Some(source) = candidate else {
            return;
        };
        book.serial = book.serial.wrapping_add(1);
        let serial = book.serial;
        book.started = Some(std::time::Instant::now());
        if self
            .sender
            .try_send(Command::Streaming(Some(crate::streaming::Request {
                context,
                serial,
                source,
            })))
            .is_ok()
        {
            book.pending = Some(serial);
            book.failed = None;
        }
    }
    pub(super) fn streaming_response(&self, response: crate::streaming::Response) {
        let context = self.streaming_context();
        let mut book = self.streaming.borrow_mut();
        if book.pending != Some(response.request.serial)
            || context.as_ref() != Some(&response.request.context)
        {
            return;
        }
        book.pending = None;
        let mut state = self.state.borrow_mut();
        if let Some(current) = state.items.iter_mut().find(|i| {
            i.id == response.request.source.id && i.turn_id == response.request.source.turn_id
        }) {
            if response.result.is_err()
                && current.body.starts_with(&response.request.source.body)
                && current
                    .rich
                    .as_ref()
                    .is_none_or(|r| r.source_bytes < response.request.source.body.len())
            {
                Arc::make_mut(current).rich = None;
                book.failed = Some(current.clone());
                state.render_dirty = true;
                return;
            }
            if crate::streaming::apply(current, &response) {
                state.render_dirty = true;
            }
        }
    }
}
