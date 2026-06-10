use golden_core::node::Node;

use super::{ConditionManager, ConsequencesManager, FilterChainManager, OutputsManager};

#[test]
fn managed_nodes_have_correct_type_ids() {
    assert_eq!(ConditionManager::NODE_TYPE, "sm_condition_manager");
    assert_eq!(ConsequencesManager::NODE_TYPE, "sm_consequences_manager");
    assert_eq!(FilterChainManager::NODE_TYPE, "sm_filter_chain_manager");
    assert_eq!(OutputsManager::NODE_TYPE, "sm_outputs_manager");
}

#[test]
fn managed_nodes_are_non_removable_by_default() {
    let cm = ConditionManager::new();
    let csq = ConsequencesManager::new();
    let fc = FilterChainManager::new();
    let out = OutputsManager::new();

    for (label, perm) in [
        ("ConditionManager", &cm.node_data().meta.user_permissions),
        ("ConsequencesManager", &csq.node_data().meta.user_permissions),
        ("FilterChainManager", &fc.node_data().meta.user_permissions),
        ("OutputsManager", &out.node_data().meta.user_permissions),
    ] {
        assert!(!perm.can_remove_and_duplicate, "{label} should not be removable");
        assert!(!perm.can_edit_name, "{label} should not have editable name");
    }
}
