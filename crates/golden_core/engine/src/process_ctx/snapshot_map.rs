use std::{
    collections::{HashMap, HashSet, hash_map},
    hash::Hash,
    mem::size_of,
    sync::Arc,
};

use golden_io::{PayloadCapacityError, PayloadPool, RetainedPayload};

pub(crate) trait SnapshotValue: Clone + Send + Sync + 'static {
    fn heap_bytes(&self) -> usize;
}

const MAX_LAYERS: usize = 8;

type LayerLeaseResult<K, V> =
    Result<Vec<RetainedPayload<SnapshotLayer<K, V>>>, PayloadCapacityError<Arc<SnapshotLayer<K, V>>>>;

#[derive(Debug)]
enum Entries<K, V> {
    Base(HashMap<K, V>),
    Delta {
        previous: Arc<SnapshotLayer<K, V>>,
        changes: HashMap<K, Option<V>>,
    },
}

/// Each layer owns its map and records; parent links share independently leased storage.
#[derive(Debug)]
pub(crate) struct SnapshotLayer<K, V> {
    entries: Entries<K, V>,
    bytes: usize,
    depth: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct SnapshotMap<K, V> {
    root: Arc<SnapshotLayer<K, V>>,
    len: usize,
}

impl<K: Copy + Eq + Hash + Send + Sync + 'static, V: SnapshotValue> SnapshotMap<K, V> {
    pub(crate) fn new(values: HashMap<K, V>) -> Self {
        let bytes = size_of::<SnapshotLayer<K, V>>()
            .saturating_add(2 * size_of::<usize>())
            .saturating_add(super::memory::map(&values))
            .saturating_add(
                values
                    .values()
                    .map(SnapshotValue::heap_bytes)
                    .fold(0, usize::saturating_add),
            );
        Self {
            len: values.len(),
            root: Arc::new(SnapshotLayer {
                entries: Entries::Base(values),
                bytes,
                depth: 0,
            }),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    pub(crate) fn get(&self, key: &K) -> Option<&V> {
        let mut layer = self.root.as_ref();
        loop {
            match &layer.entries {
                Entries::Base(values) => return values.get(key),
                Entries::Delta { previous, changes } => {
                    if let Some(value) = changes.get(key) {
                        return value.as_ref();
                    }
                    layer = previous;
                }
            }
        }
    }

    fn layers(&self) -> impl Iterator<Item = &Arc<SnapshotLayer<K, V>>> {
        std::iter::successors(Some(&self.root), |layer| match &layer.entries {
            Entries::Base(_) => None,
            Entries::Delta { previous, .. } => Some(previous),
        })
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        let mut shadowed = HashSet::new();
        self.layers()
            .flat_map(|layer| match &layer.entries {
                Entries::Base(values) => EntryIter::Base(values.iter()),
                Entries::Delta { changes, .. } => EntryIter::Delta(changes.iter()),
            })
            .filter_map(move |(key, value, delta)| {
                if delta {
                    if !shadowed.insert(*key) {
                        return None;
                    }
                } else if shadowed.contains(key) {
                    return None;
                }
                value.map(|value| (key, value))
            })
    }

    pub(crate) fn keys(&self) -> impl Iterator<Item = &K> {
        self.iter().map(|(key, _)| key)
    }
    pub(crate) fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, value)| value)
    }

    pub(crate) fn changed(&self, changes: HashMap<K, Option<V>>) -> Self {
        if changes.is_empty() {
            return self.clone();
        }
        if self.needs_compaction(changes.len()) {
            let mut values: HashMap<_, _> = self
                .iter()
                .filter(|(key, _)| !changes.contains_key(key))
                .map(|(key, value)| (*key, value.clone()))
                .collect();
            for (key, value) in changes {
                if let Some(value) = value {
                    values.insert(key, value);
                } else {
                    values.remove(&key);
                }
            }
            return Self::new(values);
        }
        let mut len = self.len;
        for (key, value) in &changes {
            match (self.get(key).is_some(), value.is_some()) {
                (false, true) => len += 1,
                (true, false) => len -= 1,
                _ => {}
            }
        }
        let bytes = size_of::<SnapshotLayer<K, V>>()
            .saturating_add(2 * size_of::<usize>())
            .saturating_add(super::memory::map(&changes))
            .saturating_add(
                changes
                    .values()
                    .flatten()
                    .map(SnapshotValue::heap_bytes)
                    .fold(0, usize::saturating_add),
            );
        Self {
            len,
            root: Arc::new(SnapshotLayer {
                entries: Entries::Delta {
                    previous: self.root.clone(),
                    changes,
                },
                bytes,
                depth: self.root.depth + 1,
            }),
        }
    }

    pub(crate) fn needs_compaction(&self, changes: usize) -> bool {
        self.root.depth >= MAX_LAYERS || changes > (self.len / 4).max(32)
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        self.layers().map(|layer| layer.bytes).fold(0, usize::saturating_add)
    }

    pub(crate) fn lease_storage_bytes(&self) -> usize {
        self.layers().count() * size_of::<RetainedPayload<SnapshotLayer<K, V>>>()
    }

    pub(crate) fn try_lease(&self, pool: &PayloadPool) -> LayerLeaseResult<K, V> {
        let mut leases = Vec::with_capacity(self.layers().count());
        for layer in self.layers() {
            leases.push(pool.try_retain_arc(layer.clone(), layer.bytes)?);
        }
        Ok(leases)
    }
}

enum EntryIter<'a, K, V> {
    Base(hash_map::Iter<'a, K, V>),
    Delta(hash_map::Iter<'a, K, Option<V>>),
}

impl<'a, K, V> Iterator for EntryIter<'a, K, V> {
    type Item = (&'a K, Option<&'a V>, bool);
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Base(iter) => iter.next().map(|(key, value)| (key, Some(value), false)),
            Self::Delta(iter) => iter.next().map(|(key, value)| (key, value.as_ref(), true)),
        }
    }
}
