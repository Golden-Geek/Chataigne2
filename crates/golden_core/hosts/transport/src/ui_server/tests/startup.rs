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

#[test]
fn host_service_startup_observes_the_prepared_authoritative_engine_and_propagates_failure() {
    let engine = create_new_project_engine::<StartupNode>().unwrap();
    let shutdown = crate::UiServerShutdown::default();
    let error = crate::run_with_ui_server_config_and_ready(
        engine,
        crate::UiServerConfig {
            bind_addr: "127.0.0.1:0".into(),
            shutdown: Some(shutdown.clone()),
            ..Default::default()
        },
        |runtime| {
            let prepared = runtime
                .capture_app(|engine| {
                    engine
                        .nodes
                        .iter()
                        .any(|(_, node)| node.node_data().meta.label == "Prepared")
                })
                .unwrap();
            assert!(prepared);
            Err::<(), _>(std::io::Error::other("app service rejected startup"))
        },
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "app service rejected startup");
    assert!(shutdown.is_requested());
}

#[test]
fn bind_failure_never_starts_app_services() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let engine = create_new_project_engine::<StartupNode>().unwrap();
    let invoked = std::sync::atomic::AtomicBool::new(false);
    let result = crate::run_with_ui_server_config_and_ready(
        engine,
        crate::UiServerConfig {
            bind_addr: listener.local_addr().unwrap().to_string(),
            ..Default::default()
        },
        |_| {
            invoked.store(true, std::sync::atomic::Ordering::Release);
            Ok(())
        },
    );
    assert!(result.is_err());
    assert!(!invoked.load(std::sync::atomic::Ordering::Acquire));
}
