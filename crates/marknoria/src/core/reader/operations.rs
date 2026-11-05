//! Bounded, document-owned cancellation. No background thread or toolkit.
use std::{
    collections::HashMap,
    io,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
};

const MAX_RUNNING: usize = 8;
#[derive(Default)]
struct State {
    closed: bool,
    running: HashMap<uuid::Uuid, Weak<AtomicBool>>,
}
impl Drop for State {
    fn drop(&mut self) {
        for flag in self.running.values().filter_map(Weak::upgrade) {
            flag.store(true, Ordering::Relaxed);
        }
    }
}

/// Clone before dispatching work so a host can cancel without locking its reader.
#[derive(Clone, Default)]
pub struct Operations(Arc<Mutex<State>>);

pub struct Operation {
    id: uuid::Uuid,
    owner: Weak<Mutex<State>>,
    pub(crate) flag: Arc<AtomicBool>,
}

impl Operations {
    pub fn begin(&self) -> io::Result<Operation> {
        self.begin_with_id(&uuid::Uuid::new_v4().to_string())
    }

    /// Register before dispatch; the UUID correlates host work and cancellation.
    pub fn begin_with_id(&self, id: &str) -> io::Result<Operation> {
        let id = uuid::Uuid::parse_str(id).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "Operation ID must be a UUID")
        })?;
        let mut state = self.0.lock().unwrap();
        state.running.retain(|_, flag| flag.strong_count() != 0);
        if state.closed {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Reader is closed",
            ));
        }
        if state.running.contains_key(&id) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Operation ID is already active",
            ));
        }
        if state.running.len() >= MAX_RUNNING {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Reader operation budget exhausted",
            ));
        }
        let flag = Arc::new(AtomicBool::new(false));
        state.running.insert(id, Arc::downgrade(&flag));
        Ok(Operation {
            id,
            owner: Arc::downgrade(&self.0),
            flag,
        })
    }

    pub fn cancel(&self, id: &str) -> io::Result<bool> {
        let id = uuid::Uuid::parse_str(id)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid operation ID"))?;
        let state = self.0.lock().unwrap();
        let flag = state.running.get(&id).and_then(Weak::upgrade);
        if let Some(flag) = &flag {
            flag.store(true, Ordering::Relaxed);
        }
        Ok(flag.is_some())
    }

    pub fn close(&self) {
        let mut state = self.0.lock().unwrap();
        state.closed = true;
        for flag in state.running.values().filter_map(Weak::upgrade) {
            flag.store(true, Ordering::Relaxed);
        }
    }

    pub fn active(&self) -> usize {
        self.0
            .lock()
            .unwrap()
            .running
            .values()
            .filter(|flag| flag.strong_count() != 0)
            .count()
    }

    pub(crate) fn check(&self, operation: &Operation) -> io::Result<()> {
        if !operation.owner.ptr_eq(&Arc::downgrade(&self.0)) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Operation belongs to another document",
            ));
        }
        if self.0.lock().unwrap().closed || operation.cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Reader operation cancelled",
            ));
        }
        Ok(())
    }
}

impl Operation {
    pub fn id(&self) -> String {
        self.id.to_string()
    }
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Relaxed);
    }
    pub fn cancelled(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }
}

impl Drop for Operation {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.upgrade() {
            owner.lock().unwrap().running.remove(&self.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn operations_are_bounded_owned_cancelled_and_removed() {
        let owner = Operations::default();
        let other = Operations::default();
        let first = owner.begin().unwrap();
        assert!(other.check(&first).is_err());
        assert!(!other.cancel(&first.id()).unwrap());
        assert!(owner.begin_with_id(&first.id()).is_err());
        assert!(owner.cancel(&first.id()).unwrap());
        assert!(owner.check(&first).is_err());
        drop(first);
        let running: Vec<_> = (0..MAX_RUNNING).map(|_| owner.begin().unwrap()).collect();
        assert!(owner.begin().is_err());
        owner.close();
        assert!(running.iter().all(Operation::cancelled));
        assert!(owner.begin().is_err());
        drop(running);
        assert_eq!(owner.active(), 0);
        let orphan = {
            let owner = Operations::default();
            owner.begin().unwrap()
        };
        assert!(orphan.cancelled());
    }
}
