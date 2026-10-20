use super::*;
use golden_core::parameter::{ParamValue, ParameterEventBehaviour};

fn admit(
    engine: &crate::app::AppEngine,
    module: NodeId,
    inventory: runtime::observation::DeviceInventory,
    wake: &RuntimeWakeSender,
) -> (ChoiceSync, ProcessCtx) {
    let mut choices = ChoiceSync {
        applied_warning: Some(None),
        ..Default::default()
    };
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    choices.service(
        &mut ctx,
        module,
        Some(wake),
        true,
        Some(&snapshot),
        Some(inventory.clone()),
    );
    await_ready(&choices);
    choices.service(&mut ctx, module, Some(wake), true, Some(&snapshot), Some(inventory));
    assert!(matches!(
        ctx.edits.pending.first().unwrap().edit,
        Edit::CheckPreconditions { .. }
    ));
    assert!(ctx.edits.pending.len() > 1);
    (choices, ctx)
}

#[test]
fn each_active_selector_change_after_admission_rejects_the_entire_choice_suffix() {
    for (key, next) in [
        ("audio_driver", "none"),
        ("device", "unavailable"),
        ("input_device", "unavailable"),
        ("output_device", "none"),
        ("sample_rate", "44100"),
        ("buffer_size", "128"),
    ] {
        let (mut engine, module, inventory, wake, _source) = choices_fixture();
        let Some(node) =
            super::super::super::find_path(&engine.process_tree_snapshot(), module, &format!("connection/{key}"))
        else {
            assert_eq!(key, "device");
            continue;
        };
        let mut constraints = engine
            .process_tree_snapshot()
            .node(node)
            .unwrap()
            .param_constraints
            .clone()
            .unwrap();
        constraints.enum_options = Default::default();
        engine.edits.push(Edit::SetParamConstraints { node, constraints });
        engine.apply_edits().unwrap();
        assert_ne!(
            engine.process_tree_snapshot().node(node).unwrap().param_value,
            Some(ParamValue::Enum(next.into()))
        );
        let (mut choices, mut ctx) = admit(&engine, module, inventory, &wake);
        let before = engine.process_tree_snapshot();
        let patched: Vec<_> = ctx
            .edits
            .pending
            .iter()
            .filter_map(|request| match &request.edit {
                Edit::SetParamConstraints { node, .. } => Some(*node),
                _ => None,
            })
            .collect();
        // The write arrives after native admission and ahead of its Core fence.
        engine.edits.push(Edit::SetParam {
            node,
            value: ParamValue::Enum(next.into()),
            behaviour: ParameterEventBehaviour::Append,
        });
        engine.edits.pending.extend(ctx.edits.drain());
        assert!(matches!(
            engine.apply_edits(),
            Err(golden_core::engine::EngineEditError::EditPreconditionFailed { edit_index: 1 })
        ));
        let after = engine.process_tree_snapshot();
        assert_eq!(
            after.node(node).unwrap().param_value,
            Some(ParamValue::Enum(next.into()))
        );
        for target in patched {
            assert_eq!(
                before.node(target).unwrap().param_constraints,
                after.node(target).unwrap().param_constraints,
                "{key}"
            );
        }
        choices.settle(Some(&wake));
        assert!(choices.applied_revision.is_none());
        assert!(choices.retry_at.is_some());
    }
}

#[test]
fn native_publication_or_retirement_after_admission_fails_the_choice_receipt() {
    for retire in [false, true] {
        let (mut engine, module, inventory, wake, source) = choices_fixture();
        let (mut choices, mut ctx) = admit(&engine, module, inventory, &wake);
        let before = engine.process_tree_snapshot();
        if retire {
            drop(source);
        } else {
            source.advance();
        }
        engine.edits.pending.extend(ctx.edits.drain());
        assert!(matches!(
            engine.apply_edits(),
            Err(golden_core::engine::EngineEditError::EditPreconditionFailed { edit_index: 0 })
        ));
        assert_eq!(
            before.subtree_revision(module),
            engine.process_tree_snapshot().subtree_revision(module)
        );
        choices.settle(Some(&wake));
        assert!(choices.applied_revision.is_none());
        assert!(choices.retry_at.is_some());
    }
}

#[test]
fn disjoint_gain_write_after_admission_does_not_reject_current_choices() {
    let (mut engine, module, inventory, wake, _source) = choices_fixture();
    let (mut choices, mut ctx) = admit(&engine, module, inventory, &wake);
    let gain = super::super::super::find_path(
        &engine.process_tree_snapshot(),
        module,
        "parameters/output/master_gain_db",
    )
    .unwrap();
    engine.edits.push(Edit::SetParam {
        node: gain,
        value: ParamValue::Float(-6.0),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.edits.pending.extend(ctx.edits.drain());
    engine.apply_edits().unwrap();
    choices.settle(Some(&wake));
    assert_eq!(choices.applied_revision, Some(7));
    assert_eq!(
        engine.process_tree_snapshot().node(gain).unwrap().param_value,
        Some(ParamValue::Float(-6.0))
    );
}
