use crate::parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour};
use crate::ui_sync::UiEditIntent;

use super::*;

#[test]
fn compiler_parameter_capture_copies_only_the_mutated_shard() {
    let mut engine = Engine::new(Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None));
    let root = engine.root;
    let before = engine.parameter_values_cache.clone();

    let acknowledgement = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: root,
        value: ParamValue::Int(7),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(acknowledgement.success);
    let after = engine.parameter_values_cache.clone();

    assert_eq!(before.get(&root), Some(&ParamValue::Int(0)));
    assert_eq!(after.get(&root), Some(&ParamValue::Int(7)));
    assert_eq!(
        before.shared_shards_with(&after),
        ParameterValueStore::shard_count() - 1,
        "one parameter write should preserve every unaffected compiler-capture shard"
    );
}

#[test]
fn atomic_checkpoint_shares_parameter_values_and_restores_the_original_shards() {
    let mut engine = Engine::new(Parameter::new("root", ParamValue::Int(3), ParameterChangeCheck::None));
    let root = engine.root;
    let before = engine.parameter_values_cache.clone();
    let checkpoint = engine.edit_checkpoint();

    assert_eq!(
        before.shared_shards_with(&engine.parameter_values_cache),
        ParameterValueStore::shard_count()
    );
    engine.edits.push(Edit::SetParam {
        node: root,
        value: ParamValue::Int(9),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    assert_eq!(before.get(&root), Some(&ParamValue::Int(3)));
    assert_eq!(engine.parameter_values_cache.get(&root), Some(&ParamValue::Int(9)));
    assert_eq!(
        before.shared_shards_with(&engine.parameter_values_cache),
        ParameterValueStore::shard_count() - 1
    );

    engine.rollback_edit_checkpoint(checkpoint).unwrap();
    assert_eq!(
        engine.nodes.get(root).unwrap().engine_param_snapshot().unwrap().value,
        ParamValue::Int(3)
    );
    assert_eq!(engine.parameter_values_cache.get(&root), Some(&ParamValue::Int(3)));
    assert_eq!(
        before.shared_shards_with(&engine.parameter_values_cache),
        ParameterValueStore::shard_count(),
        "rollback restores the exact checkpoint roots"
    );
    assert_eq!(engine.undo_len(), 0);
}
