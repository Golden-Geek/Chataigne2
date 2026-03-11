use crate::define_node_enum;
use crate::edit::Edit;
use crate::engine::Engine;
use crate::node::{Folder, Node, NodeId};
use crate::parameter::ParameterEventBehaviour;
use crate::process_ctx::ExecutionPhase;

use super::*;

define_node_enum!(
    enum AnimationCurveTestNode {}
);

fn first_child<T: Node>(engine: &Engine<T>, parent: NodeId) -> NodeId {
    engine
        .nodes
        .get(parent)
        .and_then(|node| node.node_data().first_child)
        .expect("parent should have one child")
}

fn direct_child_decl_ids<T: Node>(engine: &Engine<T>, parent: NodeId) -> Vec<String> {
    let mut decl_ids = Vec::new();
    let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    while let Some(child_id) = child {
        let child_node = engine.nodes.get(child_id).expect("child should exist");
        decl_ids.push(child_node.node_data().meta.decl_id.0.clone());
        child = child_node.node_data().next_sibling;
    }
    decl_ids
}

fn find_direct_child_by_decl<T: Node>(engine: &Engine<T>, parent: NodeId, decl_id: &str) -> Option<NodeId> {
    let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    while let Some(child_id) = child {
        let child_node = engine.nodes.get(child_id)?;
        if child_node.node_data().meta.decl_id.0 == decl_id {
            return Some(child_id);
        }
        child = child_node.node_data().next_sibling;
    }
    None
}

fn stabilize_dependency_updates<T: Node>(engine: &mut Engine<T>, reason: &str) {
    for _ in 0..3 {
        engine.apply_edits().expect(reason);
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dependency stabilization dispatch should succeed");
    }
}

#[test]
fn parse_helpers_map_variants() {
    assert_eq!(parse_step_mode("stepSize"), CurveStepMode::StepSize);
    assert_eq!(parse_shape("reverseSaw"), CurveShape::ReverseSaw);
    assert_eq!(parse_phase_mode("numPhases"), CurvePhaseMode::NumPhases);
}

#[test]
fn easing_node_dependencies_follow_kind_and_mode() {
    let root: AnimationCurveTestNode = Folder::new("Root").into();
    let mut engine = Engine::new(root);

    engine.add_node(
        AnimationCurveEasingNode::new_with_easing(
            "Ease",
            CurveEasing::Steps {
                step_mode: CurveStepMode::StepSize,
                step_size: 0.25,
                num_steps: 9,
            },
        )
        .into(),
        None,
    );
    stabilize_dependency_updates(&mut engine, "easing node creation should apply");

    let easing = first_child(&engine, engine.root);
    let direct_children_after_create = direct_child_decl_ids(&engine, easing);
    let step_mode = find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID)
        .unwrap_or_else(|| {
            panic!(
                "step mode should exist; children were {:?}",
                direct_children_after_create
            )
        });
    let kind =
        find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_KIND_DECL_ID).expect("kind should exist");

    assert!(
        find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID).is_none(),
        "non-bezier easings should hide the out handle position"
    );
    assert!(
        find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID).is_none(),
        "non-bezier easings should hide the out handle value"
    );
    assert!(
        find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID).is_none(),
        "non-bezier easings should hide the in handle position"
    );
    assert!(
        find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID).is_none(),
        "non-bezier easings should hide the in handle value"
    );

    assert_eq!(
        direct_child_decl_ids(&engine, easing),
        vec![
            PARAMETER_ANIMATION_EASING_KIND_DECL_ID.to_string(),
            PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID.to_string(),
            PARAMETER_ANIMATION_EASING_STEP_SIZE_DECL_ID.to_string(),
        ],
        "step-size easings should materialize only the active step parameter",
    );

    engine.edits.push(Edit::SetParam {
        node: step_mode,
        value: ParamValue::Enum("numSteps".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    stabilize_dependency_updates(&mut engine, "switching step mode should apply");

    assert_eq!(
        direct_child_decl_ids(&engine, easing),
        vec![
            PARAMETER_ANIMATION_EASING_KIND_DECL_ID.to_string(),
            PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID.to_string(),
            PARAMETER_ANIMATION_EASING_NUM_STEPS_DECL_ID.to_string(),
        ],
        "switching step mode should swap the dependent step parameter in place",
    );

    engine.edits.push(Edit::SetParam {
        node: kind,
        value: ParamValue::Enum("shape".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    stabilize_dependency_updates(&mut engine, "switching easing kind should apply");

    assert!(
        find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID).is_none(),
        "shape easings should hide the out handle position"
    );
    assert!(
        find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID).is_none(),
        "shape easings should hide the out handle value"
    );
    assert!(
        find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID).is_none(),
        "shape easings should hide the in handle position"
    );
    assert!(
        find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID).is_none(),
        "shape easings should hide the in handle value"
    );

    assert_eq!(
        direct_child_decl_ids(&engine, easing),
        vec![
            PARAMETER_ANIMATION_EASING_KIND_DECL_ID.to_string(),
            PARAMETER_ANIMATION_EASING_SHAPE_DECL_ID.to_string(),
            PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID.to_string(),
            PARAMETER_ANIMATION_EASING_PHASE_MODE_DECL_ID.to_string(),
            PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID.to_string(),
            PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID.to_string(),
            PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID.to_string(),
        ],
        "shape easings should expose only the active shape-specific parameters",
    );

    engine.edits.push(Edit::SetParam {
        node: kind,
        value: ParamValue::Enum("bezier".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    stabilize_dependency_updates(&mut engine, "switching easing kind to bezier should apply");

    assert_eq!(
        direct_child_decl_ids(&engine, easing),
        vec![
            PARAMETER_ANIMATION_EASING_KIND_DECL_ID.to_string(),
            PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID.to_string(),
            PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID.to_string(),
            PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID.to_string(),
            PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID.to_string(),
        ],
        "bezier easings should expose only the bezier handle parameters",
    );
}

#[test]
fn easing_node_preserves_script_source_default() {
    let root: AnimationCurveTestNode = Folder::new("Root").into();
    let mut engine = Engine::new(root);
    let expected_source = "return t * 0.5;".to_string();

    engine.add_node(
        AnimationCurveEasingNode::new_with_easing(
            "Ease",
            CurveEasing::Script {
                source: expected_source.clone(),
            },
        )
        .into(),
        None,
    );
    stabilize_dependency_updates(&mut engine, "script easing node creation should apply");

    let easing = first_child(&engine, engine.root);
    let direct_children_after_create = direct_child_decl_ids(&engine, easing);
    let script_source = find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_SCRIPT_SOURCE_DECL_ID)
        .unwrap_or_else(|| {
            panic!(
                "script easings should materialize script source; children were {:?}",
                direct_children_after_create
            )
        });
    let script_source_snapshot = engine
        .nodes
        .get(script_source)
        .and_then(Node::engine_param_snapshot)
        .expect("script source should expose a parameter snapshot");

    assert!(
        find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID).is_none(),
        "script easings should hide the out handle position"
    );
    assert!(
        find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID).is_none(),
        "script easings should hide the out handle value"
    );
    assert!(
        find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID).is_none(),
        "script easings should hide the in handle position"
    );
    assert!(
        find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID).is_none(),
        "script easings should hide the in handle value"
    );

    assert_eq!(script_source_snapshot.value, ParamValue::Str(expected_source));
}
