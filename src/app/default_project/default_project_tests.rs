use golden_core::node::{Folder, Node};

use super::initialize_default_project;
use crate::app::{AppEngine, AppNode, StateMachineManager};

#[test]
fn default_project_contains_one_top_level_state_machine_manager() {
    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);

    initialize_default_project(&mut engine);
    engine.apply_edits().expect("default project nodes should attach");

    let managers = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.get_type() == StateMachineManager::NODE_TYPE)
        .collect::<Vec<_>>();

    assert_eq!(managers.len(), 1);
    assert_eq!(managers[0].1.node_data().parent, Some(engine.root));
}
