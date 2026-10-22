use super::*;
use crate::app::module_modules_audio_sound_card::{runtime, tests as sound_tests};

#[test]
fn queued_runtime_value_write_cannot_follow_a_target_moved_out_of_its_module() {
    rejects_moved_path(false);
}

#[test]
fn queued_runtime_value_write_cannot_follow_a_moved_ancestor() {
    rejects_moved_path(true);
}

fn rejects_moved_path(move_ancestor: bool) {
    let (mut engine, module) = sound_tests::create_sound_card();
    let bindings = runtime::collect_bindings_for_test(&engine.process_tree_snapshot(), module);
    let reads = ProjectionReads::prepare(&engine.process_tree_snapshot(), module, &bindings).unwrap();
    let node = bindings.active_voices.unwrap();
    let original = engine.nodes.get(node).unwrap().engine_param_snapshot().unwrap().value;
    let mut values = ValueDelivery::default();
    let mut cache = ValueCache::prepare(&bindings).unwrap();
    let mut snapshot = AudioObservationSnapshot::default();
    snapshot.playback.active_voices = 7;
    let observation = view(snapshot, 1);
    let mut queued = context(&engine);
    values.service(
        &mut queued,
        &bindings,
        &reads,
        ConfigGeneration::INITIAL,
        &observation,
        &mut cache,
    );
    assert!(!queued.edits.pending.is_empty());
    let moved = if move_ancestor {
        engine.process_tree_snapshot().node(node).unwrap().parent.unwrap()
    } else {
        node
    };
    engine.edits.push(Edit::MoveNode {
        node: moved,
        new_parent: engine.root,
        new_prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    engine.absorb_edits(&mut queued).unwrap();
    assert!(engine.apply_edits().is_err());
    assert_eq!(
        engine.nodes.get(node).unwrap().engine_param_snapshot().unwrap().value,
        original
    );
}

#[test]
fn queued_values_fail_after_generation_reset_or_publisher_retirement() {
    for reset in [true, false] {
        let mut engine = Engine::new(Parameter::new(
            "value",
            ParamValue::Float(0.0),
            ParameterChangeCheck::ValueChange,
        ));
        let mut bindings = SoundCardValueBindings {
            input_master_level: Some(engine.root),
            ..Default::default()
        };
        bindings.runtime_values.insert(engine.root);
        let reads = ProjectionReads::prepare(&engine.process_tree_snapshot(), engine.root, &bindings).unwrap();
        let mut cache = ValueCache::prepare(&bindings).unwrap();
        let mut delivery = ValueDelivery::default();
        let observation = view(
            AudioObservationSnapshot {
                input_global_max_rms: 0.75,
                ..Default::default()
            },
            1,
        );
        let mut ctx = context(&engine);
        delivery.service(
            &mut ctx,
            &bindings,
            &reads,
            ConfigGeneration::INITIAL,
            &observation,
            &mut cache,
        );
        assert!(!ctx.edits.pending.is_empty());
        if reset {
            delivery.reset();
        } else {
            drop(delivery);
        }
        engine.absorb_edits(&mut ctx).unwrap();
        assert!(engine.apply_edits().is_err());
        assert_eq!(engine.nodes.get(engine.root).unwrap().get(), &ParamValue::Float(0.0));
    }
}

#[test]
fn preparation_rejects_cross_module_and_overdeep_projection_paths() {
    let (mut engine, module) = sound_tests::create_sound_card();
    let mut bindings = SoundCardValueBindings::default();
    bindings.runtime_values.insert(engine.root);
    assert!(
        ProjectionReads::prepare(&engine.process_tree_snapshot(), module, &bindings)
            .unwrap_err()
            .contains("outside")
    );
    let mut parent = module;
    for _ in 0..32 {
        engine.add_node(Folder::new("nested").into(), Some(parent));
        engine.apply_edits().unwrap();
        parent = engine.nodes.get(parent).unwrap().node_data().last_child.unwrap();
    }
    bindings.runtime_values.clear();
    bindings.runtime_values.insert(parent);
    assert!(
        ProjectionReads::prepare(&engine.process_tree_snapshot(), module, &bindings)
            .unwrap_err()
            .contains("32 nodes")
    );
}
