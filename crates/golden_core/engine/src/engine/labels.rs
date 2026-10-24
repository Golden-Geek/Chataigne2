use std::collections::{HashMap, HashSet};

use super::Engine;
use crate::node::{Node, NodeId};

impl<T: Node> Engine<T> {
    pub(crate) fn reserve_child_label(
        &self,
        parent: NodeId,
        preferred: &str,
        reservations: &mut HashMap<NodeId, UniqueLabels>,
    ) -> String {
        reservations
            .entry(parent)
            .or_insert_with(|| {
                let mut child = self.nodes.get(parent).and_then(|node| node.node_data().first_child);
                let mut labels = Vec::new();
                while let Some(id) = child {
                    let Some(node) = self.nodes.get(id) else { break };
                    labels.push(node.node_data().meta.label.trim().to_owned());
                    child = node.node_data().next_sibling;
                }
                UniqueLabels::new(labels)
            })
            .reserve(preferred)
    }
}

/// Reservations share occupied-suffix successor paths across a detached batch.
pub(crate) struct UniqueLabels {
    used: HashSet<String>,
    successors: HashMap<String, HashMap<u64, u64>>,
}

impl UniqueLabels {
    pub(crate) fn new(labels: impl IntoIterator<Item = String>) -> Self {
        Self {
            used: labels.into_iter().collect(),
            successors: HashMap::new(),
        }
    }

    pub(crate) fn reserve(&mut self, preferred: &str) -> String {
        let preferred = preferred.trim();
        let preferred = if preferred.is_empty() { "Item" } else { preferred };
        if self.used.insert(preferred.to_owned()) {
            return preferred.to_owned();
        }
        let (base, start) = preferred
            .rsplit_once(' ')
            .and_then(|(base, number)| {
                let number = number.parse::<u64>().ok().filter(|number| *number >= 2)?;
                (!base.trim().is_empty()).then_some((base, number.checked_add(1)))
            })
            .unwrap_or((preferred, Some(2)));
        let Some(mut suffix) = start else {
            return self.reserve(&format!("{preferred} Copy"));
        };
        let successors = self.successors.entry(base.to_owned()).or_default();
        let mut visited = Vec::new();
        loop {
            let candidate = format!("{base} {suffix}");
            let free = self.used.insert(candidate.clone());
            let Some(next) = suffix.checked_add(1) else {
                if free {
                    return candidate;
                }
                return self.reserve(&format!("{preferred} Copy"));
            };
            if free {
                for visited in visited {
                    successors.insert(visited, next);
                }
                successors.insert(suffix, next);
                return candidate;
            }
            visited.push(suffix);
            suffix = successors.get(&suffix).copied().unwrap_or(next);
        }
    }
}

#[cfg(test)]
mod tests;
