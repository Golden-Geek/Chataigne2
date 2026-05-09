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
mod tests {
    use super::*;
    use crate::node::NodeId;

    fn id(n: u64) -> NodeId {
        NodeId(n)
    }

    fn sub(node: NodeId) -> EventSubscription {
        EventSubscription::node(node)
    }

    fn sub_depth(node: NodeId, max_depth: u32) -> EventSubscription {
        EventSubscription::subtree(node, max_depth)
    }

    #[test]
    fn add_populates_both_sides() {
        let mut idx = ListenerIndex::default();
        let (a, b) = (id(1), id(2));
        idx.add(a, sub(b));

        assert_eq!(idx.listeners_for_origin(b), &[(a, 0)]);
        assert!(idx.get_subscriptions(a).unwrap().contains(&sub(b)));
    }

    #[test]
    fn add_is_idempotent() {
        let mut idx = ListenerIndex::default();
        let (a, b) = (id(1), id(2));
        idx.add(a, sub(b));
        idx.add(a, sub(b));

        assert_eq!(idx.listeners_for_origin(b).len(), 1);
        assert_eq!(idx.get_subscriptions(a).unwrap().len(), 1);
    }

    #[test]
    fn remove_cleans_both_sides() {
        let mut idx = ListenerIndex::default();
        let (a, b) = (id(1), id(2));
        idx.add(a, sub(b));
        idx.remove(a, sub(b));

        assert!(idx.listeners_for_origin(b).is_empty());
        assert!(idx.get_subscriptions(a).is_none());
        assert!(idx.is_empty());
    }

    #[test]
    fn purge_as_subscriber_cleans_origin_back_refs() {
        let mut idx = ListenerIndex::default();
        let (a, b, c) = (id(1), id(2), id(3));
        idx.add(a, sub(b));
        idx.add(c, sub(b)); // c also listens to b

        idx.purge_for_node(a);

        assert!(idx.get_subscriptions(a).is_none());
        // b should only have c remaining
        assert_eq!(idx.listeners_for_origin(b), &[(c, 0)]);
    }

    #[test]
    fn purge_as_origin_cleans_subscriber_forward_refs() {
        let mut idx = ListenerIndex::default();
        let (a, b, c) = (id(1), id(2), id(3));
        idx.add(a, sub(b));
        idx.add(a, sub(c)); // a listens to both b and c

        idx.purge_for_node(b);

        assert!(idx.listeners_for_origin(b).is_empty());
        // a should still have its subscription to c
        let subs = idx.get_subscriptions(a).unwrap();
        assert!(!subs.contains(&sub(b)));
        assert!(subs.contains(&sub(c)));
    }

    #[test]
    fn purge_self_subscription_does_not_panic() {
        let mut idx = ListenerIndex::default();
        let a = id(1);
        idx.add(a, sub(a)); // a listens to itself

        idx.purge_for_node(a);

        assert!(idx.is_empty());
    }

    #[test]
    fn listeners_for_origin_with_max_depth() {
        let mut idx = ListenerIndex::default();
        let (a, b) = (id(1), id(2));
        idx.add(a, sub_depth(b, 3));

        assert_eq!(idx.listeners_for_origin(b), &[(a, 3)]);
    }

    #[test]
    fn multiple_subscribers_same_origin() {
        let mut idx = ListenerIndex::default();
        let (a, b, c, origin) = (id(1), id(2), id(3), id(99));
        idx.add(a, sub(origin));
        idx.add(b, sub(origin));
        idx.add(c, sub_depth(origin, 2));

        let listeners = idx.listeners_for_origin(origin);
        assert_eq!(listeners.len(), 3);
        assert!(listeners.contains(&(a, 0)));
        assert!(listeners.contains(&(b, 0)));
        assert!(listeners.contains(&(c, 2)));
    }
}
