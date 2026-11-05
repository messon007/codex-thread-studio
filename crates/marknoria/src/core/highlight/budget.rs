//! Bounded active parsers and a cooperative event-iteration deadline.
//! No background threads: safe for a native C ABI client to release its library.
use std::{
    sync::{
        Condvar, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
const PARALLEL: usize = 8;
static ACTIVE: Mutex<usize> = Mutex::new(0);
static CHANGED: Condvar = Condvar::new();
pub(super) struct Lease {
    pub flag: AtomicUsize,
    started: Instant,
}
impl Lease {
    pub fn new() -> Self {
        let mut active = ACTIVE.lock().unwrap();
        while *active >= PARALLEL {
            active = CHANGED.wait(active).unwrap()
        }
        *active += 1;
        Self {
            flag: AtomicUsize::new(0),
            started: Instant::now(),
        }
    }
    pub fn expired(&self) -> bool {
        if self.started.elapsed() > Duration::from_millis(500) {
            self.flag.store(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        *ACTIVE.lock().unwrap() -= 1;
        CHANGED.notify_one()
    }
}
