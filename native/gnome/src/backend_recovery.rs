//! Bounded recovery of backend transports, without retaining any request.
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
// The gateway accepts sixteen configured backends plus two built-ins.
const LIMIT: usize = 18;
const RETRIES: u8 = 8;
const HEALTHY: Duration = Duration::from_secs(30);
struct Connection {
    generation: u64,
    failures: u8,
    pending: bool,
    ready_at: Option<Instant>,
}
#[derive(Default)]
pub struct Book(HashMap<String, Connection>);
#[derive(Debug, PartialEq, Eq)]
pub enum Plan {
    Retry { delay: Duration, attempt: u8 },
    Exhausted,
    Ignore,
}
impl Book {
    /// An explicit connection starts a fresh budget, including before its first
    /// successful handshake. Automatic retries use `started` to retain theirs.
    pub fn connecting(&mut self, backend: &str, generation: u64) {
        if backend.is_empty()
            || backend.len() > 256
            || (!self.0.contains_key(backend) && self.0.len() >= LIMIT)
        {
            return;
        }
        self.0.insert(
            backend.into(),
            Connection {
                generation,
                failures: 0,
                pending: false,
                ready_at: None,
            },
        );
    }
    pub fn ready(&mut self, backend: &str, generation: u64, now: Instant) {
        if backend.is_empty()
            || backend.len() > 256
            || (!self.0.contains_key(backend) && self.0.len() >= LIMIT)
        {
            return;
        }
        let entry = self.0.entry(backend.into()).or_insert(Connection {
            generation,
            failures: 0,
            pending: false,
            ready_at: None,
        });
        if entry.generation != generation {
            *entry = Connection {
                generation,
                failures: 0,
                pending: false,
                ready_at: None,
            };
        }
        entry.pending = false;
        entry.ready_at.get_or_insert(now);
    }
    pub fn failed(&mut self, backend: &str, generation: u64, now: Instant) -> Plan {
        let Some(entry) = self
            .0
            .get_mut(backend)
            .filter(|e| e.generation == generation)
        else {
            return Plan::Ignore;
        };
        if entry.pending {
            return Plan::Ignore;
        }
        if entry
            .ready_at
            .take()
            .is_some_and(|at| now.saturating_duration_since(at) >= HEALTHY)
        {
            entry.failures = 0;
        }
        if entry.failures >= RETRIES {
            return Plan::Exhausted;
        }
        let delay = Duration::from_millis((500u64 << entry.failures).min(30_000));
        entry.failures += 1;
        entry.pending = true;
        Plan::Retry {
            delay,
            attempt: entry.failures,
        }
    }
    pub fn take(&mut self, backend: &str, generation: u64) -> bool {
        let Some(entry) = self
            .0
            .get_mut(backend)
            .filter(|e| e.generation == generation && e.pending)
        else {
            return false;
        };
        entry.pending = false;
        true
    }
    pub fn started(&mut self, backend: &str, old: u64, new: u64) {
        if let Some(entry) = self
            .0
            .get_mut(backend)
            .filter(|e| e.generation == old && !e.pending)
        {
            entry.generation = new;
            entry.ready_at = None;
        }
    }
    pub fn forget(&mut self, backend: &str) {
        self.0.remove(backend);
    }
    pub fn clear(&mut self) {
        self.0.clear();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsuccessful_startup_attempts_keep_their_budget_until_explicit_retry() {
        let mut b = Book::default();
        let t = Instant::now();
        b.connecting("startup", 1);
        for i in 1..=8 {
            assert!(matches!(
                b.failed("startup", i, t),
                Plan::Retry { attempt, .. } if u64::from(attempt) == i
            ));
            assert_eq!(b.failed("startup", i, t), Plan::Ignore);
            assert!(b.take("startup", i));
            b.started("startup", i, i + 1);
        }
        assert_eq!(b.failed("startup", 9, t), Plan::Exhausted);
        b.connecting("startup", 10);
        assert!(matches!(
            b.failed("startup", 10, t),
            Plan::Retry { attempt: 1, .. }
        ));
        assert_eq!(b.failed("startup", 9, t), Plan::Ignore);
        b.connecting("", 1);
        b.connecting(&"x".repeat(257), 1);
        for n in 0..100 {
            b.connecting(&format!("backend-{n}"), n);
        }
        assert_eq!(b.0.len(), LIMIT);
    }
    #[test]
    fn backoff_is_deduplicated_generation_owned_and_bounded_during_flapping() {
        let mut b = Book::default();
        let t = Instant::now();
        assert_eq!(b.failed("a", 1, t), Plan::Ignore);
        b.ready("a", 1, t);
        for (i, ms) in [500, 1000, 2000, 4000, 8000, 16000, 30000, 30000]
            .into_iter()
            .enumerate()
        {
            let generation = i as u64 + 1;
            assert_eq!(
                b.failed("a", generation, t),
                Plan::Retry {
                    delay: Duration::from_millis(ms),
                    attempt: i as u8 + 1
                }
            );
            assert_eq!(b.failed("a", generation, t), Plan::Ignore);
            assert!(!b.take("a", generation + 100));
            assert!(b.take("a", generation));
            assert!(!b.take("a", generation));
            b.started("a", generation, generation + 1);
            b.ready("a", generation + 1, t);
        }
        assert_eq!(b.failed("a", 9, t), Plan::Exhausted);
        b.ready("a", 10, t); // Explicit new connection resets exhausted ownership.
        assert!(matches!(
            b.failed("a", 10, t),
            Plan::Retry { attempt: 1, .. }
        ));
    }
    #[test]
    fn healthy_connections_reset_and_old_callbacks_cannot_reconnect_replacements() {
        let mut b = Book::default();
        let t = Instant::now();
        b.ready("a", 1, t);
        let _ = b.failed("a", 1, t);
        assert!(b.take("a", 1));
        b.started("a", 1, 2);
        b.ready("a", 2, t);
        b.ready("a", 2, t + HEALTHY); // Repeated ready notification does not postpone health.
        assert!(matches!(
            b.failed("a", 2, t + HEALTHY),
            Plan::Retry { attempt: 1, .. }
        ));
        b.ready("a", 3, t + HEALTHY);
        assert!(!b.take("a", 2));
        assert_eq!(b.failed("a", 2, t), Plan::Ignore);
        b.forget("a");
        assert_eq!(b.failed("a", 3, t), Plan::Ignore);
        for n in 0..100 {
            b.ready(&format!("{n}"), 1, t);
        }
        assert_eq!(b.0.len(), LIMIT);
        b.clear();
        assert!(b.0.is_empty());
    }
}
