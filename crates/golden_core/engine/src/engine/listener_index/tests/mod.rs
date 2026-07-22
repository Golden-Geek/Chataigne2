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
