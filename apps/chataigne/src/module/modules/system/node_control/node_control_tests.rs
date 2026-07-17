use golden_core::{
    node::{DeclaredUserItemNode, Node},
    parameter::ParamValue,
};

use crate::app::NodeModule;
use crate::app::module_modules_system_node_control::{
    script_set_value, script_trigger,
};

#[test]
fn node_module_is_a_project_creatable_system_module() {
    assert_eq!(
        <NodeModule as DeclaredUserItemNode>::ITEM_NODE_TYPE,
        "node_module"
    );
    assert!(NodeModule::project_create("node_module").is_some());
}

#[test]
fn node_script_methods_require_stable_references() {
    assert!(script_set_value(None, &[ParamValue::Bool(true), ParamValue::Int(1)]).is_err());
    assert!(script_trigger(None, &[ParamValue::Bool(true)]).is_err());
}
