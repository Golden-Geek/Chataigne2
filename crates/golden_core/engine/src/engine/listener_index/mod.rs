use std::collections::HashMap;

use crate::node::{EventSubscription, NodeId};

/// Two-way index mapping subscribers to their subscriptions and origins to their listeners.
///
/// - `by_subscriber[S]` → all subscriptions registered by subscriber S.
/// - `by_origin[O]`     → all (subscriber, max_depth) pairs that listen to origin O.
///
/// This lets `collect_subscription_recipients` look up only the ancestors of the current event
/// origin instead of scanning every subscriber, reducing the hot path from O(N_subscribers) to
/// O(tree_depth × avg_listeners_per_node).
#[derive(Default)]
pub(crate) struct ListenerIndex {
    by_subscriber: HashMap<NodeId, Vec<EventSubscription>>,
    by_origin: HashMap<NodeId, Vec<(NodeId, u32)>>,
}

impl ListenerIndex {
    pub(crate) fn is_empty(&self) -> bool {
        self.by_subscriber.is_empty()
    }

    pub(crate) fn clear(&mut self) {
        self.by_subscriber.clear();
        self.by_origin.clear();
    }

    pub(crate) fn add(&mut self, subscriber: NodeId, subscription: EventSubscription) {
        let subs = self.by_subscriber.entry(subscriber).or_default();
        if subs.contains(&subscription) {
            return;
        }
        subs.push(subscription);
        self.by_origin
            .entry(subscription.node)
            .or_default()
            .push((subscriber, subscription.max_depth));
    }

    pub(crate) fn remove(&mut self, subscriber: NodeId, subscription: EventSubscription) {
        if let Some(subs) = self.by_subscriber.get_mut(&subscriber) {
            if let Some(pos) = subs.iter().position(|s| *s == subscription) {
                subs.swap_remove(pos);
            }
            if subs.is_empty() {
                self.by_subscriber.remove(&subscriber);
            }
        }
        if let Some(origins) = self.by_origin.get_mut(&subscription.node) {
            if let Some(pos) = origins
                .iter()
                .position(|&(id, depth)| id == subscriber && depth == subscription.max_depth)
            {
                origins.swap_remove(pos);
            }
            if origins.is_empty() {
                self.by_origin.remove(&subscription.node);
            }
        }
    }

    /// Removes all entries where `node` appears as a subscriber or as an origin.
    pub(crate) fn purge_for_node(&mut self, node: NodeId) {
        // Remove node as a subscriber and clean up its back-references in by_origin.
        if let Some(subs) = self.by_subscriber.remove(&node) {
            for sub in &subs {
                if let Some(origins) = self.by_origin.get_mut(&sub.node) {
                    origins.retain(|&(id, _)| id != node);
                    if origins.is_empty() {
                        self.by_origin.remove(&sub.node);
                    }
                }
            }
        }
        // Remove node as an origin and clean up its subscribers' by_subscriber entries.
        if let Some(listeners) = self.by_origin.remove(&node) {
            for &(subscriber_id, _) in &listeners {
                if let Some(subs) = self.by_subscriber.get_mut(&subscriber_id) {
                    subs.retain(|s| s.node != node);
                    if subs.is_empty() {
                        self.by_subscriber.remove(&subscriber_id);
                    }
                }
            }
        }
    }

    /// Returns all `(subscriber_id, max_depth)` pairs that have a subscription rooted at `origin`.
    pub(crate) fn listeners_for_origin(&self, origin: NodeId) -> &[(NodeId, u32)] {
        self.by_origin.get(&origin).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Returns the subscriptions registered by `subscriber`, if any.
    #[cfg(test)]
    pub(crate) fn get_subscriptions(&self, subscriber: NodeId) -> Option<&Vec<EventSubscription>> {
        self.by_subscriber.get(&subscriber)
    }

    /// Iterates over all subscribers' subscription lists.
    #[cfg(test)]
    pub(crate) fn all_subscriber_subscriptions(&self) -> impl Iterator<Item = &Vec<EventSubscription>> {
        self.by_subscriber.values()
    }
}

#[cfg(test)]
mod tests;
