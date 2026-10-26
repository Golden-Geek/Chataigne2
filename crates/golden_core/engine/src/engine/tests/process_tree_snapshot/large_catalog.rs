use super::*;
use crate::parameter::{ParameterConstraints, ParameterControlState};

#[test]
fn wide_parameter_catalog_fits_the_canonical_snapshot_lease_budget() {
    const ROWS: u64 = 32;
    const COLUMNS: u64 = 4096;
    const RECORDS: u64 = 1 + ROWS * (1 + COLUMNS);
    let root = NodeId(1);
    let mut nodes = HashMap::with_capacity(RECORDS as usize);
    let mut root_node = snapshot_node(root, NodeUuid(Uuid::from_u128(1)), None);
    root_node.first_child = Some(NodeId(2));
    root_node.child_count = ROWS as usize;
    nodes.insert(root, root_node);
    for row in 0..ROWS {
        let folder = NodeId(2 + row * (1 + COLUMNS));
        let mut folder_node = snapshot_node(folder, NodeUuid(Uuid::from_u128(folder.0.into())), None);
        folder_node.parent = Some(root);
        folder_node.first_child = Some(NodeId(folder.0 + 1));
        folder_node.child_count = COLUMNS as usize;
        folder_node.next_sibling = (row + 1 < ROWS).then_some(NodeId(folder.0 + 1 + COLUMNS));
        nodes.insert(folder, folder_node);
        for column in 0..COLUMNS {
            let id = NodeId(folder.0 + column + 1);
            let uuid = NodeUuid(Uuid::from_u128(id.0.into()));
            let mut parameter = snapshot_node(id, uuid, Some(ParamValue::Float(0.5)));
            parameter.parent = Some(folder);
            parameter.decl_id = format!("value_{}", uuid.0.simple());
            parameter.label = format!("Target {column:06}");
            parameter.short_name = format!("target_{column:06}");
            parameter.param_constraints = Some(ParameterConstraints::default());
            parameter.param_control = Some(ParameterControlState::default());
            parameter.next_sibling = (column + 1 < COLUMNS).then_some(NodeId(id.0 + 1));
            nodes.insert(id, parameter);
        }
    }
    let snapshot = Arc::new(ProcessTreeSnapshot::new(root, nodes));
    let bytes = snapshot.retained_bytes();
    eprintln!("{RECORDS} wide parameter records reserve {bytes} bytes");
    // Leave half of the existing 1 GiB pool available for concurrent source/retirement views.
    assert!(bytes < 512 * 1024 * 1024, "wide catalog reserves {bytes} bytes");
    assert!(bytes >= RECORDS as usize * std::mem::size_of::<ProcessTreeNodeSnapshot>());
    let lease = snapshot
        .try_lease()
        .expect("wide catalog should fit without a larger pool");
    assert_eq!(lease.node_count(), RECORDS as usize);
    let last_folder = NodeId(2 + (ROWS - 1) * (1 + COLUMNS));
    let last = NodeId(RECORDS);
    assert_eq!(lease.child_ids_slice(last_folder).len(), COLUMNS as usize);
    assert_eq!(
        lease.node_id_by_uuid(NodeUuid(Uuid::from_u128(RECORDS.into()))),
        Some(last)
    );
    assert_eq!(lease.node(last).unwrap().param_value, Some(ParamValue::Float(0.5)));
    assert_eq!(
        lease.find_child_by_decl_id(last_folder, &lease.node(last).unwrap().decl_id),
        Some(last)
    );
}
