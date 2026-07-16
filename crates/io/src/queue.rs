use std::collections::VecDeque;

/// A queue bounded by both item count and caller-defined item weight.
#[derive(Clone, Debug)]
pub struct BoundedQueue<T> {
    entries: VecDeque<Weighted<T>>,
    total_weight: usize,
    maximum_items: usize,
    maximum_weight: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Weighted<T> {
    value: T,
    weight: usize,
}

/// Returned without discarding ownership when a bounded queue rejects an item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueFull<T> {
    value: T,
}

impl<T> BoundedQueue<T> {
    pub fn new(maximum_items: usize, maximum_weight: usize) -> Self {
        assert!(maximum_items > 0, "bounded queue must accept at least one item");
        assert!(maximum_weight > 0, "bounded queue must accept positive weight");
        Self {
            entries: VecDeque::new(),
            total_weight: 0,
            maximum_items,
            maximum_weight,
        }
    }

    pub fn try_push(&mut self, value: T, weight: usize) -> Result<(), QueueFull<T>> {
        let next_weight = self.total_weight.checked_add(weight);
        if self.entries.len() >= self.maximum_items
            || next_weight.is_none_or(|next_weight| next_weight > self.maximum_weight)
        {
            return Err(QueueFull { value });
        }

        self.entries.push_back(Weighted { value, weight });
        self.total_weight = next_weight.expect("bounded queue weight was checked");
        Ok(())
    }

    pub fn pop_front(&mut self) -> Option<T> {
        let entry = self.entries.pop_front()?;
        self.total_weight = self.total_weight.saturating_sub(entry.weight);
        Some(entry.value)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn total_weight(&self) -> usize {
        self.total_weight
    }
}

impl<T> QueueFull<T> {
    pub fn into_inner(self) -> T {
        self.value
    }
}
