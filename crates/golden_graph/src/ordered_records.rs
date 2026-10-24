use std::{
    collections::{BTreeMap, HashMap},
    hash::Hash,
};

/// An authored position, retained across removal so rollback never shifts other records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RecordOrder(u128);

/// Identity lookup and authored iteration have independent storage. Removing one record
/// never renumbers the surviving entries.
#[derive(Clone, Debug)]
pub(crate) struct OrderedRecords<K, V> {
    records: HashMap<K, (RecordOrder, V)>,
    order: BTreeMap<RecordOrder, K>,
}

impl<K, V> Default for OrderedRecords<K, V> {
    fn default() -> Self {
        Self {
            records: HashMap::new(),
            order: BTreeMap::new(),
        }
    }
}

impl<K: Copy + Eq + Hash, V> OrderedRecords<K, V> {
    pub(crate) fn contains_key(&self, key: &K) -> bool {
        self.records.contains_key(key)
    }

    pub(crate) fn get(&self, key: &K) -> Option<&V> {
        self.records.get(key).map(|(_, value)| value)
    }

    pub(crate) fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.records.get_mut(key).map(|(_, value)| value)
    }

    pub(crate) fn values(&self) -> impl ExactSizeIterator<Item = &V> {
        self.order.values().map(|key| &self.records[key].1)
    }

    pub(crate) fn insert(&mut self, order: Option<RecordOrder>, key: K, value: V) {
        let order = order.unwrap_or_else(|| {
            // Deriving the next key from live order avoids a historical counter that
            // rollback or persistence would otherwise have to restore. Gaps are harmless.
            RecordOrder(self.order.last_key_value().map_or(0, |(last, _)| {
                last.0.checked_add(1).expect("authored order exhausted u128")
            }))
        });
        assert!(!self.records.contains_key(&key), "record identity must be unique");
        assert!(!self.order.contains_key(&order), "authored position must be vacant");
        self.records.insert(key, (order, value));
        self.order.insert(order, key);
    }

    pub(crate) fn remove(&mut self, key: &K) -> Option<(RecordOrder, V)> {
        let (order, value) = self.records.remove(key)?;
        assert!(
            self.order.remove(&order) == Some(*key),
            "authored order must be indexed"
        );
        Some((order, value))
    }
}

impl<K: Copy + Eq + Hash, V: PartialEq> PartialEq for OrderedRecords<K, V> {
    fn eq(&self, other: &Self) -> bool {
        // Ordinals are runtime indexes. Equality and persistence describe authored order,
        // including identity, rather than the gaps left by earlier committed removals.
        self.order.len() == other.order.len()
            && self
                .order
                .values()
                .zip(other.order.values())
                .all(|(left, right)| left == right && self.get(left) == other.get(right))
    }
}

#[cfg(test)]
mod tests;
