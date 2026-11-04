use crate::{engine::node_store::NodeStore, node::Folder};

#[test]
fn membership_revision_tracks_equal_size_replacement_and_detached_history_without_value_noise() {
    let mut nodes = NodeStore::<Folder>::new();
    let empty = nodes.membership_revision();
    let first = nodes.insert(Folder::new("First"));
    let inserted = nodes.membership_revision();
    assert_ne!(empty, inserted);
    nodes.get_mut(first).unwrap();
    assert_eq!(nodes.membership_revision(), inserted);
    let detached = nodes.detach(first).unwrap();
    let absent = nodes.membership_revision();
    assert_ne!(absent, inserted);
    nodes.reattach(first, detached);
    let restored = nodes.membership_revision();
    assert_ne!(restored, absent);
    assert_ne!(restored, inserted);
    nodes.remove(first).unwrap();
    let second = nodes.insert(Folder::new("Second"));
    assert_ne!(first, second);
    assert_ne!(restored, nodes.membership_revision());
    assert_eq!(nodes.len(), 1);
    assert_ne!(
        nodes.membership_revision().0,
        NodeStore::<Folder>::new().membership_revision().0
    );
}
