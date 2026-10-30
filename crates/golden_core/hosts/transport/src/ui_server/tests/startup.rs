use golden_engine::app::{ProjectLifecycle, create_new_project_engine};
use golden_engine::engine::Engine;
use golden_engine::node::{Folder, Node};

use super::super::startup::prepare_engine_for_host_startup;

golden_engine::define_node_enum!(
    enum StartupNode {}
);

impl ProjectLifecycle for StartupNode {
    fn initialize_new_project(engine: &mut Engine<Self>) -> Result<(), String> {
        engine.add_node(Folder::new("Seed").into(), None);
        Ok(())
    }

    fn prepare_project_candidate(engine: &mut Engine<Self>) -> Result<(), String> {
        if engine.nodes.get(engine.root).unwrap().node_data().first_child.is_none() {
            return Err("required seeded child is missing".into());
        }
        engine.add_node(Folder::new("Prepared").into(), None);
        Ok(())
    }
}

#[test]
fn startup_validates_attached_defaults_and_settles_candidate_hook_edits_before_activation() {
    let mut engine = create_new_project_engine::<StartupNode>().unwrap();
    prepare_engine_for_host_startup(&mut engine).unwrap();
    for label in ["Seed", "Prepared"] {
        let (_, node) = engine
            .nodes
            .iter()
            .find(|(_, node)| node.node_data().meta.label == label)
            .unwrap();
        assert_eq!(node.node_data().parent, Some(engine.root));
    }
    assert_eq!(engine.undo_len(), 0);
}

#[test]
fn startup_rejects_an_invalid_app_candidate() {
    let mut engine = Engine::<StartupNode>::new(Folder::new("Root").into());
    let error = prepare_engine_for_host_startup(&mut engine).unwrap_err();
    assert!(error.to_string().contains("required seeded child is missing"));
}
