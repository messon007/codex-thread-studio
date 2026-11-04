//! Memory-weighted wrapper around the maintained LRU implementation.
use std::{borrow::Borrow, hash::Hash, num::NonZeroUsize};
pub trait Weight {
    fn weight(&self) -> usize;
}
pub struct ByteCache<K, V> {
    entries: lru::LruCache<K, V>,
    bytes: usize,
    limit: usize,
}
impl<K: Hash + Eq, V: Weight> ByteCache<K, V> {
    pub fn new(entries: NonZeroUsize, bytes: usize) -> Self {
        Self {
            entries: lru::LruCache::new(entries),
            bytes: 0,
            limit: bytes,
        }
    }
    pub fn insert(&mut self, key: K, value: V) {
        let weight = value.weight();
        if weight > self.limit {
            self.remove(&key);
            return;
        }
        self.bytes = self.bytes.saturating_add(weight);
        if let Some((_, old)) = self.entries.push(key, value) {
            self.bytes = self.bytes.saturating_sub(old.weight());
        }
        self.trim();
    }
    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let value = self.entries.pop(key)?;
        self.bytes = self.bytes.saturating_sub(value.weight());
        Some(value)
    }
    pub fn peek<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.entries.peek(key)
    }
    pub fn update<Q, R>(&mut self, key: &Q, update: impl FnOnce(&mut V) -> R) -> Option<R>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let value = self.entries.get_mut(key)?;
        let before = value.weight();
        let result = update(value);
        let after = value.weight();
        self.bytes = self.bytes.saturating_sub(before).saturating_add(after);
        self.trim();
        Some(result)
    }
    pub fn for_each(&mut self, mut apply: impl FnMut(&K, &mut V)) {
        self.bytes = 0;
        for (key, value) in self.entries.iter_mut() {
            apply(key, value);
            self.bytes = self.bytes.saturating_add(value.weight());
        }
        self.trim();
    }
    fn trim(&mut self) {
        while self.bytes > self.limit {
            if let Some((_, value)) = self.entries.pop_lru() {
                self.bytes = self.bytes.saturating_sub(value.weight());
            } else {
                self.bytes = 0;
                break;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    impl Weight for Vec<u8> {
        fn weight(&self) -> usize {
            self.len()
        }
    }
    #[test]
    fn accounts_for_replacement_growth_eviction_and_moving_values() {
        let mut cache = ByteCache::new(NonZeroUsize::new(2).unwrap(), 8);
        cache.insert("a", vec![1; 4]);
        cache.insert("b", vec![2; 4]);
        cache.update("a", |value| value.push(1));
        assert!(cache.remove("b").is_none());
        assert_eq!(cache.bytes, 5);
        cache.insert("a", vec![1; 2]);
        assert_eq!(cache.bytes, 2);
        cache.insert("large", vec![0; 9]);
        assert_eq!(cache.bytes, 2);
        let value = cache.remove("a").unwrap();
        assert_eq!(value.len(), 2);
        assert_eq!(cache.bytes, 0);
    }
}
