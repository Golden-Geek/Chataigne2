use golden_core::node::Node;
use golden_core::parameter::ParamValue;

use super::{ConditionManager, ConsequencesManager, FilterChainManager, InputsManager, OutputsManager};

#[test]
fn managed_nodes_have_correct_type_ids() {
    assert_eq!(ConditionManager::NODE_TYPE, "sm_condition_manager");
    assert_eq!(ConsequencesManager::NODE_TYPE, "sm_consequences_manager");
    assert_eq!(InputsManager::NODE_TYPE, "sm_inputs_manager");
    assert_eq!(FilterChainManager::NODE_TYPE, "sm_filter_chain_manager");
    assert_eq!(OutputsManager::NODE_TYPE, "sm_outputs_manager");
}

#[test]
fn managed_nodes_are_non_removable_by_default() {
    let cm = ConditionManager::new();
    let csq = ConsequencesManager::new();
    let inp = InputsManager::new();
    let fc = FilterChainManager::new();
    let out = OutputsManager::new();

    for (label, perm) in [
        ("ConditionManager", &cm.node_data().meta.user_permissions),
        ("ConsequencesManager", &csq.node_data().meta.user_permissions),
        ("InputsManager", &inp.node_data().meta.user_permissions),
        ("FilterChainManager", &fc.node_data().meta.user_permissions),
        ("OutputsManager", &out.node_data().meta.user_permissions),
    ] {
        assert!(!perm.can_remove_and_duplicate, "{label} should not be removable");
        assert!(!perm.can_edit_name, "{label} should not have editable name");
    }
}

#[test]
fn condition_manager_operator_defaults_to_all() {
    let cm = ConditionManager::new();
    assert_eq!(cm.operator.get_ref().as_str(), "all");
}

#[test]
fn condition_manager_accepts_all_operators() {
    let mut cm = ConditionManager::new();
    for op in ["all", "any", "none", "at_least", "exactly"] {
        cm.operator.apply_runtime_value(&ParamValue::Str(op.to_string()));
        assert_eq!(cm.operator.get_ref().as_str(), op);
    }
}

#[test]
fn condition_manager_policy_defaults() {
    let cm = ConditionManager::new();
    assert_eq!(cm.empty_policy.get_ref().as_str(), "invalid");
    assert_eq!(cm.disabled_policy.get_ref().as_str(), "ignore");
    assert_eq!(cm.error_policy.get_ref().as_str(), "treat_as_invalid");
    assert_eq!(*cm.operator_count.get_ref(), 1.0);
}
