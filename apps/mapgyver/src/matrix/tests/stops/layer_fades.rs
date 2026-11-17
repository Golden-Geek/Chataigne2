use super::super::layer_settings::{override_enabled, write};
use super::*;

fn duration(scene: &mut Scene, row: usize, millis: u32) {
    assert!(
        write(
            &mut scene.engine,
            scene.rows[row].0,
            "duration_ms",
            ParamValue::Float(millis.into())
        )
        .success
    );
}
fn stop(scene: &mut Scene, row: usize) -> TransitionDefinition {
    pulse(&mut scene.engine, scene.rows[row].0, "stop");
    current(&scene.engine, scene.media)
        .layers
        .iter()
        .find(|request| request.layer == scene.rows[row])
        .unwrap()
        .transition
}
fn weight(scene: &Scene, row: usize, now: TimeSnapshot) -> f32 {
    scene.weights(row, now).iter().map(|(_, weight)| weight).sum()
}

#[test]
fn default_layer_stop_fades_once_without_changing_peer_history_or_project() {
    let mut scene = Scene::new(2);
    scene.launch(0, LaunchMode::Cut);
    let now = time();
    assert_eq!(weight(&scene, 0, now), 1.0);
    let authored = to_sparse_project_json_pretty(&scene.engine).unwrap();
    let history = scene.engine.undo_len();
    assert_eq!(stop(&mut scene, 0), TransitionDefinition::default());
    let stopped = current(&scene.engine, scene.media);
    assert_eq!(weight(&scene, 0, now), 1.0);
    assert!((weight(&scene, 0, at(now, 125)) - 0.5).abs() < 0.00001);
    stop(&mut scene, 0);
    assert_eq!(
        current(&scene.engine, scene.media),
        stopped,
        "repeated Stop keeps the original deadline"
    );
    assert_eq!(weight(&scene, 0, at(now, 250)), 0.0);
    assert_eq!(weight(&scene, 1, at(now, 250)), 1.0);
    assert_eq!(to_sparse_project_json_pretty(&scene.engine).unwrap(), authored);
    assert_eq!(scene.engine.undo_len(), history);
}

#[test]
fn stop_resolves_current_cell_column_layer_metadata_and_dormant_values() {
    for (cell_enabled, column_enabled, expected) in [
        (
            true,
            true,
            TransitionDefinition {
                duration_ms: 1000,
                curve: TransitionCurve::Linear,
            },
        ),
        (
            false,
            true,
            TransitionDefinition {
                duration_ms: 800,
                curve: TransitionCurve::Smooth,
            },
        ),
        (
            false,
            false,
            TransitionDefinition {
                duration_ms: 600,
                curve: TransitionCurve::Smooth,
            },
        ),
    ] {
        let mut scene = Scene::new(1);
        duration(&mut scene, 0, 600);
        let cell = scene.cell(0, 0);
        for (owner, millis, enabled) in [
            (cell.0, 1000.0, cell_enabled),
            (scene.columns[0].0, 800.0, column_enabled),
        ] {
            assert!(write(&mut scene.engine, owner, "duration_ms", ParamValue::Float(millis)).success);
            assert!(override_enabled(&mut scene.engine, owner, enabled).success);
        }
        assert!(write(&mut scene.engine, cell.0, "curve", ParamValue::Enum("Linear".into())).success);
        scene.launch(0, LaunchMode::Cut);
        let now = time();
        scene.weights(0, now);
        assert_eq!(stop(&mut scene, 0), expected);
        scene.weights(0, now);
        let t = at(now, u64::from(expected.duration_ms / 4));
        let expected_weight = if expected.curve == TransitionCurve::Linear {
            0.75
        } else {
            0.84375
        };
        assert!((weight(&scene, 0, t) - expected_weight).abs() < 0.00001);
        assert_eq!(weight(&scene, 0, at(now, expected.duration_ms.into())), 0.0);
    }
    let mut scene = Scene::new(1);
    duration(&mut scene, 0, 0);
    scene.launch(0, LaunchMode::Cut);
    let now = time();
    scene.weights(0, now);
    assert_eq!(stop(&mut scene, 0).duration_ms, 0);
    assert_eq!(weight(&scene, 0, now), 0.0);
}

#[test]
fn independently_launched_destination_resolves_the_whole_interrupted_mix_and_reactivation_survives() {
    let mut scene = Scene::new(2);
    let cell = scene.cell(0, 2);
    assert!(write(&mut scene.engine, cell.0, "duration_ms", ParamValue::Float(400.0)).success);
    assert!(override_enabled(&mut scene.engine, cell.0, true).success);
    scene.launch(0, LaunchMode::Cut);
    let now = time();
    scene.weights(0, now);
    scene.launch(1, LaunchMode::Configured);
    scene.weights(0, now);
    pulse(&mut scene.engine, cell.0, "play");
    scene.weights(0, at(now, 100));
    assert_eq!(stop(&mut scene, 0).duration_ms, 400);
    let before = scene.weights(0, at(now, 100));
    let halfway = scene.weights(0, at(now, 300));
    for ((source, weight), (same, half)) in before.iter().zip(&halfway) {
        assert_eq!(source, same);
        assert!((half - weight * 0.5).abs() < 0.00001);
    }
    pulse(&mut scene.engine, cell.0, "cut");
    assert_eq!(scene.weights(0, at(now, 300)), [(2, 1.0)]);
    assert_eq!(
        scene.weights(0, at(now, 600)),
        [(2, 1.0)],
        "old completion cannot stop a newer launch"
    );
    assert_eq!(weight(&scene, 1, at(now, 600)), 1.0);
}

#[test]
fn headless_stop_holds_application_until_deadline_then_invalidates_the_logical_capture() {
    let mut scene = Scene::new(1);
    duration(&mut scene, 0, 40);
    let MediaEditResult::MatrixCellPrepared { cell, application } = edit(
        &mut scene.engine,
        scene.media,
        MatrixEdit::ConfigureCellApplication {
            layer: scene.rows[0],
            column: scene.columns[0],
            pre: 0,
            post: 0,
            definition: crate::applications::protocol::ApplicationDefinition {
                executable: Some("C:\\not-launched.exe".into()),
                ..Default::default()
            },
        },
    ) else {
        panic!()
    };
    pulse(&mut scene.engine, cell.0, "cut");
    stop(&mut scene, 0);
    let before = crate::applications::logical::capture_logical_applications(&scene.engine).unwrap();
    assert!(before.demand.iter().any(|entry| entry.application == application));
    std::thread::sleep(std::time::Duration::from_millis(60));
    let root = crate::matrix::playback::root(&scene.engine, scene.media).unwrap();
    let AppNode::MediaChannel(root) = scene.engine.nodes.get(root).unwrap() else {
        panic!()
    };
    assert!(root.matrix_playback().unwrap().service_videos().is_none());
    assert!(!before.is_current());
    assert!(
        crate::applications::logical::capture_logical_applications(&scene.engine)
            .unwrap()
            .demand
            .is_empty()
    );
}

#[test]
fn script_parameter_stop_uses_the_same_fade_and_does_not_author_history() {
    let mut scene = Scene::new(1);
    scene.launch(0, LaunchMode::Cut);
    let now = time();
    scene.weights(0, now);
    let saved = to_sparse_project_json_pretty(&scene.engine).unwrap();
    let history = scene.engine.undo_len();
    let node = parameter(&scene.engine, scene.rows[0].0, "stop");
    scene.engine.edits.push(Edit::SetNodeScriptProperty {
        node,
        write: golden_core::script::ScriptPropertyWrite::prepare("value", ParamValue::Trigger()).unwrap(),
    });
    for _ in 0..2 {
        scene.engine.apply_edits().unwrap();
        scene
            .engine
            .dispatch_inbox(golden_core::process_ctx::ExecutionPhase::EndOfTickStabilization)
            .unwrap();
    }
    scene.engine.apply_edits().unwrap();
    golden_core::application::service_script_app_operations(&mut scene.engine).unwrap();
    assert_eq!(weight(&scene, 0, now), 1.0);
    assert!((weight(&scene, 0, at(now, 125)) - 0.5).abs() < 0.00001);
    assert_eq!(weight(&scene, 0, at(now, 250)), 0.0);
    assert_eq!(scene.engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&scene.engine).unwrap(), saved);
}

#[test]
fn column_cut_during_a_layer_fade_releases_only_its_logical_outgoing_hold() {
    let mut scene = Scene::new(1);
    let mut cells = Vec::new();
    let mut applications = Vec::new();
    for column in &scene.columns[..2] {
        let MediaEditResult::MatrixCellPrepared { cell, application } = edit(
            &mut scene.engine,
            scene.media,
            MatrixEdit::ConfigureCellApplication {
                layer: scene.rows[0],
                column: *column,
                pre: 0,
                post: 0,
                definition: crate::applications::protocol::ApplicationDefinition {
                    executable: Some("C:\\not-launched.exe".into()),
                    ..Default::default()
                },
            },
        ) else {
            panic!()
        };
        cells.push(cell);
        applications.push(application);
    }
    let now = time();
    pulse(&mut scene.engine, cells[0].0, "cut");
    scene.weights(0, now);
    pulse(&mut scene.engine, cells[1].0, "play");
    scene.weights(0, at(now, 100));
    stop(&mut scene, 0);
    scene.weights(0, at(now, 100));
    let root = crate::matrix::playback::root(&scene.engine, scene.media).unwrap();
    let AppNode::MediaChannel(root) = scene.engine.nodes.get(root).unwrap() else {
        panic!()
    };
    let control = root.matrix_playback().unwrap().clone();
    assert!(
        applications
            .iter()
            .all(|id| control.layer_stop_applications().contains(id))
    );
    pulse(&mut scene.engine, scene.columns[0].0, "stop");
    current(&scene.engine, scene.media);
    assert!(!control.layer_stop_applications().contains(&applications[0]));
    assert!(control.layer_stop_applications().contains(&applications[1]));
    pulse(&mut scene.engine, cells[0].0, "cut");
    current(&scene.engine, scene.media);
    assert!(
        control.layer_stop_applications().is_empty(),
        "reactivation revokes old outgoing completion"
    );
    assert_eq!(scene.weights(0, at(now, 500)), [(0, 1.0)]);
}
