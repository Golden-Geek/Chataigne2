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
