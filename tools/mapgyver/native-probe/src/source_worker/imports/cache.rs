//! Bounded local values keyed by allocation identity without retaining reader ownership.

use std::collections::VecDeque;
use std::sync::{Arc, Weak};

struct Entry<K, V> {
    identity: Weak<K>,
    value: V,
}

pub(super) struct WeakCache<K, V> {
    entries: VecDeque<Entry<K, V>>,
    capacity: usize,
}

impl<K, V: Clone> WeakCache<K, V> {
    pub(super) fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "weak cache capacity must be positive");
        Self {
            entries: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub(super) fn get(&mut self, identity: &Arc<K>) -> Option<V> {
        let weak = Arc::downgrade(identity);
        let index = self
            .entries
            .iter()
            .position(|entry| Weak::ptr_eq(&entry.identity, &weak))?;
        let entry = self.entries.remove(index)?;
        let value = entry.value.clone();
        self.entries.push_back(entry);
        Some(value)
    }

    /// Returns ownership of the replaced or least-recently-used value to the caller.
    pub(super) fn insert(&mut self, identity: &Arc<K>, value: V) -> Option<V> {
        let weak = Arc::downgrade(identity);
        let index = self
            .entries
            .iter()
            .position(|entry| Weak::ptr_eq(&entry.identity, &weak));
        let previous = if let Some(index) = index {
            self.entries.remove(index)
        } else if self.entries.len() == self.capacity {
            self.entries.pop_front()
        } else {
            None
        };
        self.entries.push_back(Entry { identity: weak, value });
        previous.map(|entry| entry.value)
    }

    pub(super) fn prune(&mut self) -> usize {
        let previous = self.entries.len();
        // Do not upgrade weak identities: even a temporary strong reader can prevent a
        // producer from reusing its slot while this local cache is being maintained.
        self.entries.retain(|entry| entry.identity.strong_count() != 0);
        previous - self.entries.len()
    }

    /// Cloned active values can outlive the removed entries.
    pub(super) fn clear(&mut self) -> usize {
        let previous = self.entries.len();
        self.entries.clear();
        previous
    }

    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests;
