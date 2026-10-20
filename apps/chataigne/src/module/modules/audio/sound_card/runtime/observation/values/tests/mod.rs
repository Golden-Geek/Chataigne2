use super::*;
use golden_audio::{AudioChannelId, ChannelObservation};
use golden_core::{
    engine::Engine,
    node::{Folder, Node},
    parameter::{Parameter, ParameterChangeCheck},
    process_ctx::ExecutionPhase,
};

fn view(snapshot: AudioObservationSnapshot, sequence: u64) -> ObservationView {
    super::super::capture::view_for_test(snapshot, sequence)
}

fn context<T: Node>(engine: &Engine<T>) -> ProcessCtx {
    ProcessCtx::new(ExecutionPhase::EngineTick, engine.time)
}

#[test]
fn failed_value_receipts_retry_unchanged_input_without_optimistic_cache_or_duplicate_pending_edits() {
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
    let snapshot = AudioObservationSnapshot {
        input_global_max_rms: 0.75,
        ..Default::default()
    };
    let view = view(snapshot, 1);
    let mut values = ValueDelivery::default();
    let mut first = context(&engine);
    values.service(&mut first, &bindings, ConfigGeneration::INITIAL, &view);
    let count = first.edits.pending.len();
    values.service(&mut first, &bindings, ConfigGeneration::INITIAL, &view);
    assert_eq!(first.edits.pending.len(), count);
    assert!(values.cache.is_empty());
    engine.edits.push(Edit::SetParam {
        node: NodeId(u64::MAX),
        value: ParamValue::Int(1),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.absorb_edits(&mut first).unwrap();
    assert!(engine.apply_edits().is_err());
    for _ in 0..8 {
        let mut ctx = context(&engine);
        values.service(&mut ctx, &bindings, ConfigGeneration::INITIAL, &view);
        engine.absorb_edits(&mut ctx).unwrap();
        engine.apply_edits().unwrap();
        if !values.has_pending() {
            break;
        }
    }
    assert!(!values.has_pending());
    assert_eq!(values.cache.get(&engine.root.0), Some(&ParamValue::Float(0.75)));
    assert_eq!(engine.nodes.get(engine.root).unwrap().get(), &ParamValue::Float(0.75));
}

#[test]
fn value_cursor_bounds_work_and_finishes_a_pinned_frame_under_continuous_new_frames() {
    let mut engine: Engine<crate::app::AppNode> = Engine::new(Folder::new("root").into());
    let mut bindings = SoundCardValueBindings::default();
    let mut snapshot = AudioObservationSnapshot::default();
    for index in 0..150u128 {
        engine.add_node(
            Parameter::new(
                &format!("value{index}"),
                ParamValue::Float(0.0),
                ParameterChangeCheck::ValueChange,
            )
            .into(),
            None,
        );
    }
    engine.apply_edits().unwrap();
    let tree = engine.process_tree_snapshot();
    for (index, node) in tree.child_ids(engine.root).into_iter().enumerate() {
        let id = AudioChannelId::from_uuid(uuid::Uuid::from_u128(index as u128 + 1));
        bindings.channel_levels.insert(id, node);
        bindings.runtime_values.insert(node);
        snapshot.inputs.push(ChannelObservation {
            channel: id,
            rms_linear: 0.25,
            rms_dbfs: -12.0,
            peak_dbfs: -6.0,
            clipped: false,
        });
    }
    let mut values = ValueDelivery::default();
    let first = view(snapshot.clone(), 1);
    let mut ctx = context(&engine);
    values.service(&mut ctx, &bindings, ConfigGeneration::INITIAL, &first);
    assert!(ctx.edits.pending.len() <= SERVICE_ITEMS);
    assert!(values.has_pending());
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    for sequence in 2..30 {
        snapshot.inputs.iter_mut().for_each(|channel| channel.rms_linear = 0.75);
        let newest = view(snapshot.clone(), sequence);
        let mut ctx = context(&engine);
        values.service(&mut ctx, &bindings, ConfigGeneration::INITIAL, &newest);
        assert!(ctx.edits.pending.len() <= SERVICE_ITEMS);
        engine.absorb_edits(&mut ctx).unwrap();
        engine.apply_edits().unwrap();
        if values.completed == Some(1) {
            break;
        }
    }
    assert_eq!(
        values.completed,
        Some(1),
        "continuous producers cannot restart an unfinished cursor"
    );
    assert!(values.cache.values().all(|value| *value == ParamValue::Float(0.25)));
    assert_eq!(values.cache.len(), 150);
}

#[test]
fn a_reconfigured_binding_never_uses_an_older_receipt_as_its_value_cache() {
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
    let view = view(
        AudioObservationSnapshot {
            input_global_max_rms: 0.5,
            ..Default::default()
        },
        1,
    );
    let mut values = ValueDelivery::default();
    let mut old = context(&engine);
    values.service(&mut old, &bindings, ConfigGeneration::INITIAL, &view);
    values.reset();
    engine.absorb_edits(&mut old).unwrap();
    engine.apply_edits().unwrap();
    let mut current = context(&engine);
    values.service(&mut current, &bindings, ConfigGeneration::INITIAL.next(), &view);
    assert!(
        values.cache.is_empty(),
        "an old receipt must not suppress current publication"
    );
    assert_eq!(
        current.edits.pending.len(),
        1,
        "the last valid native plan remains observable while a new configuration is pending"
    );
    engine.absorb_edits(&mut current).unwrap();
    engine.apply_edits().unwrap();
    let mut settled = context(&engine);
    values.service(&mut settled, &bindings, ConfigGeneration::INITIAL.next(), &view);
    assert_eq!(values.cache.get(&engine.root.0), Some(&ParamValue::Float(0.5)));
}

#[test]
fn one_missing_value_target_cannot_poison_its_valid_suffix_or_spin_actor_continuations() {
    let mut engine = Engine::new(Parameter::new(
        "value",
        ParamValue::Float(0.0),
        ParameterChangeCheck::ValueChange,
    ));
    let missing = NodeId(u64::MAX);
    let mut bindings = SoundCardValueBindings {
        input_master_level: Some(missing),
        output_master_level: Some(engine.root),
        ..Default::default()
    };
    bindings.runtime_values.extend([missing, engine.root]);
    let view = view(
        AudioObservationSnapshot {
            input_global_max_rms: 0.5,
            output_global_max_rms: 0.75,
            ..Default::default()
        },
        1,
    );
    let mut values = ValueDelivery::default();
    for _ in 0..12 {
        let mut ctx = context(&engine);
        values.service(&mut ctx, &bindings, ConfigGeneration::INITIAL, &view);
        assert!(ctx.edits.pending.len() <= SERVICE_ITEMS);
        engine.absorb_edits(&mut ctx).unwrap();
        let _ = engine.apply_edits();
        if !values.has_pending() {
            break;
        }
    }
    assert!(!values.has_pending());
    assert!(!values.cache.contains_key(&missing.0));
    assert_eq!(values.cache.get(&engine.root.0), Some(&ParamValue::Float(0.75)));
    for _ in 0..32 {
        let mut settled = context(&engine);
        values.service(&mut settled, &bindings, ConfigGeneration::INITIAL, &view);
        assert!(
            settled.edits.pending.is_empty(),
            "permanent failures wait for a new native capture"
        );
        assert!(!values.has_pending());
    }
}
