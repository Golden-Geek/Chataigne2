use golden_audio::PlaybackObservation;
use golden_core::{
    node::{Folder, Node},
    parameter::ParamValue,
};

use super::*;

#[test]
fn playback_observation_projects_active_and_loading_voice_counts() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(SoundCardModule::create().into(), None);
    for _ in 0..8 {
        engine
            .apply_edits()
            .expect("Sound Card declarations should materialize");
    }

    let module = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("Sound Card module");
    let snapshot = engine.process_tree_snapshot();
    let active_voices =
        find_path(&snapshot, module, "values/playback_status/active_voices").expect("active voice value");
    let loading_voices =
        find_path(&snapshot, module, "values/playback_status/loading_voices").expect("loading voice value");
    let bindings = runtime::collect_bindings_for_test(&snapshot, module);

    assert!(bindings.is_runtime_value(active_voices));
    assert!(bindings.is_runtime_value(loading_voices));
    assert_eq!(
        runtime::playback_status_updates_for_test(
            &bindings,
            PlaybackObservation {
                active_voices: 3,
                loading_voices: 2,
                ..PlaybackObservation::default()
            },
        ),
        [
            (Some(active_voices), ParamValue::Int(3)),
            (Some(loading_voices), ParamValue::Int(2)),
        ],
    );
}
