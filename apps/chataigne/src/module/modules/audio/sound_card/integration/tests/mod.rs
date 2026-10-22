use super::super::tests::create_sound_card;
use super::*;
use golden_core::{edit::Edit, parameter::ParameterEventBehaviour, process_ctx::ExecutionPhase};

#[test]
fn oversized_startup_controls_fail_before_driver_copy_or_numeric_parsing() {
    let (mut engine, id) = create_sound_card();
    for (path, value) in [
        (
            "connection/audio_driver",
            "x".repeat(super::super::DRIVER_SELECTION_BYTES + 1),
        ),
        ("connection/sample_rate", "0".repeat(33)),
    ] {
        let parameter = find_path(&engine.process_tree_snapshot(), id, path).unwrap();
        let mut constraints = engine
            .process_tree_snapshot()
            .node(parameter)
            .unwrap()
            .param_constraints
            .clone()
            .unwrap();
        constraints.enum_options = Default::default();
        engine.edits.push(Edit::SetParamConstraints {
            node: parameter,
            constraints,
        });
        engine.edits.push(Edit::SetParam {
            node: parameter,
            value: ParamValue::Enum(value),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        engine.apply_edits().unwrap();
        assert!(
            matches!(engine.process_tree_snapshot().node(parameter).unwrap().param_value.as_ref(), Some(ParamValue::Enum(authored)) if authored.len() == if path.ends_with("audio_driver") { 129 } else { 33 })
        );
    }
    engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).unwrap();
    let crate::app::AppNode::SoundCardModule(module) = engine.nodes.get(id).unwrap() else {
        panic!("Sound Card")
    };
    assert_eq!(
        module.requested_driver().unwrap_err(),
        "Sound Card audio driver exceeds 128 bytes"
    );
    assert_eq!(
        module.requested_sample_rate().unwrap_err(),
        "Sound Card sample rate is invalid"
    );
}
