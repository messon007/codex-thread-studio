//! Explicit foreground-session handoff; never resumes or interrupts a turn.
use std::collections::HashMap;

pub const QUEUE_PAUSE: &str = "Session released. Reload before resuming queued messages.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Pending,
    Released,
    Unconfirmed,
}
struct Entry {
    backend: String,
    generation: u64,
    ticket: u64,
    phase: Phase,
}
#[derive(Default)]
pub struct Book {
    entries: HashMap<String, Entry>,
    next: u64,
}
impl Book {
    pub fn phase(&self, key: &str) -> Option<Phase> {
        self.entries.get(key).map(|entry| entry.phase)
    }
    pub fn blocked(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }
    pub fn begin(&mut self, backend: &str, thread: &str, generation: u64) -> Result<u64, String> {
        let key = format!("{backend}:{thread}");
        if self.blocked(&key) {
            return Err("Reload this conversation before releasing it again".into());
        }
        if self.entries.len() >= 512 {
            return Err("Too many released sessions; reload one before releasing another".into());
        }
        self.next = self.next.wrapping_add(1);
        self.entries.insert(
            key,
            Entry {
                backend: backend.into(),
                generation,
                ticket: self.next,
                phase: Phase::Pending,
            },
        );
        Ok(self.next)
    }
    pub fn finish(&mut self, key: &str, generation: u64, ticket: u64, confirmed: bool) -> bool {
        let Some(entry) = self.entries.get_mut(key).filter(|entry| {
            entry.generation == generation
                && entry.ticket == ticket
                && entry.phase == Phase::Pending
        }) else {
            return false;
        };
        entry.phase = if confirmed {
            Phase::Released
        } else {
            Phase::Unconfirmed
        };
        true
    }
    pub fn lost(&mut self, backend: Option<&str>) {
        for entry in self.entries.values_mut() {
            if backend.is_none_or(|backend| backend == entry.backend)
                && entry.phase == Phase::Pending
            {
                entry.phase = Phase::Unconfirmed;
            }
        }
    }
    /// Only explicit Reload permits preparing this source again. A pending
    /// handoff must finish or become unconfirmed before that can happen.
    pub fn reload(&mut self, key: &str) -> bool {
        if self.phase(key) == Some(Phase::Pending) {
            return false;
        }
        self.entries.remove(key).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handoff_is_scoped_and_requires_explicit_reload() {
        let mut book = Book::default();
        let ticket = book.begin("codex", "one", 4).unwrap();
        assert!(book.blocked("codex:one"));
        assert!(!book.blocked("other:one"));
        assert!(!book.reload("codex:one"));
        assert!(!book.finish("other:one", 4, ticket, true));
        assert!(!book.finish("codex:one", 5, ticket, true));
        assert!(book.finish("codex:one", 4, ticket, true));
        assert_eq!(book.phase("codex:one"), Some(Phase::Released));
        book.lost(None);
        assert!(book.blocked("codex:one"));
        assert!(book.reload("codex:one"));
        let next = book.begin("codex", "one", 5).unwrap();
        assert!(!book.finish("codex:one", 4, ticket, true));
        assert!(book.finish("codex:one", 5, next, true));
    }
    #[test]
    fn uncertain_handoff_cannot_autoresume_or_accept_a_late_ack() {
        let mut book = Book::default();
        let a = book.begin("a", "one", 1).unwrap();
        let b = book.begin("b", "one", 1).unwrap();
        book.lost(Some("a"));
        assert_eq!(book.phase("a:one"), Some(Phase::Unconfirmed));
        assert_eq!(book.phase("b:one"), Some(Phase::Pending));
        assert!(!book.finish("a:one", 1, a, true));
        assert!(book.finish("b:one", 1, b, false));
        assert!(book.blocked("a:one") && book.blocked("b:one"));
        assert!(book.reload("a:one"));
        assert!(!book.finish("a:one", 1, a, true));
    }
}
