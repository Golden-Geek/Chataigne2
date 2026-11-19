use std::time::Duration;

use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    edit::Edit,
    node::{
        GRADIENT_DECL_ID, GRADIENT_NODE_TYPE, GRADIENT_STOP_COLOR_DECL_ID, GRADIENT_STOP_POSITION_DECL_ID, Node,
        NodeId, PARAMETER_ANIMATION_CONTROL_DECL_ID, PARAMETER_ANIMATION_FREQUENCY_DECL_ID,
        PARAMETER_ANIMATION_PHASE_DECL_ID, UserContextNode, gradient_from_snapshot,
    },
    parameter::{
        ParamValue, ParameterControlMode, ParameterControlSpec, ParameterControlState, ParameterEventBehaviour,
    },
};

use super::*;
use crate::{app::AppNode, media::channel};

fn intent(engine: &mut AppEngine, edit: UiEditIntent) -> UiAck {
    let ack = apply_project_intent(engine, edit);
    assert!(ack.success, "{ack:?}");
    ack
}

fn child(engine: &AppEngine, node: NodeId, decl: &str) -> NodeId {
    engine.process_tree_snapshot().find_child(node, decl).unwrap()
}

fn set(engine: &mut AppEngine, node: NodeId, value: ParamValue) {
    intent(
        engine,
        UiEditIntent::SetParam {
            node,
            value,
            behaviour: ParameterEventBehaviour::Append,
        },
    );
}

fn control(engine: &mut AppEngine, node: NodeId, mode: ParameterControlMode, spec: ParameterControlSpec) {
    intent(
        engine,
        UiEditIntent::SetParamControlState {
            node,
            state: ParameterControlState::new(mode, spec).into(),
        },
    );
}

fn color(engine: &mut AppEngine, reference: ChannelReference) -> [f32; 4] {
    engine.run_tick(Duration::from_millis(50)).unwrap();
    let ChannelSource::SolidColor { rgba } = channel(engine, reference).unwrap().definition.source else {
        panic!("expected a Solid Color channel");
    };
    rgba
}

#[test]
fn solid_color_gradient_updates_live_and_preserves_the_authored_color() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::SolidColor);
    let owner = engine.node_id_by_uuid(reference.channel.0).unwrap();
    let parameters = child(&engine, owner, "parameters");
    let parameter = child(&engine, parameters, "parameters/rgba");
    set(&mut engine, parameter, ParamValue::Color(0.0, 1.0, 0.0, 1.0));
    control(
        &mut engine,
        parameter,
        ParameterControlMode::Animation,
        ParameterControlSpec::Animation,
    );
    let animation = child(&engine, parameter, PARAMETER_ANIMATION_CONTROL_DECL_ID);
    let gradient = child(&engine, animation, GRADIENT_DECL_ID);
    let stops = engine.process_tree_snapshot().child_ids(gradient);
    assert_eq!(stops.len(), 2);
    let [first, last] = [stops[0], stops[1]];
    let first_color = child(&engine, first, GRADIENT_STOP_COLOR_DECL_ID);
    let last_color = child(&engine, last, GRADIENT_STOP_COLOR_DECL_ID);
    let last_position = child(&engine, last, GRADIENT_STOP_POSITION_DECL_ID);
    let frequency = child(&engine, animation, PARAMETER_ANIMATION_FREQUENCY_DECL_ID);
    let phase = child(&engine, animation, PARAMETER_ANIMATION_PHASE_DECL_ID);
    set(&mut engine, frequency, ParamValue::Float(0.0));
    set(&mut engine, phase, ParamValue::Float(0.25));
    set(&mut engine, first_color, ParamValue::Color(1.0, 0.0, 0.0, 1.0));
    set(&mut engine, last_color, ParamValue::Color(0.0, 0.0, 1.0, 0.5));
    assert_eq!(color(&mut engine, reference), [0.75, 0.0, 0.25, 0.875]);

    set(&mut engine, last_position, ParamValue::Float(0.5));
    assert_eq!(color(&mut engine, reference), [0.5, 0.0, 0.5, 0.75]);
    assert!(engine.undo().unwrap());
    assert_eq!(color(&mut engine, reference), [0.75, 0.0, 0.25, 0.875]);
    assert!(engine.redo().unwrap());
    assert_eq!(color(&mut engine, reference), [0.5, 0.0, 0.5, 0.75]);

    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(color(&mut loaded, reference), [0.5, 0.0, 0.5, 0.75]);

    set(&mut engine, last_color, ParamValue::Color(0.0, 0.0, 4.0, 0.5));
    assert_eq!(color(&mut engine, reference), [0.5, 0.0, 1.0, 0.75]);
    intent(&mut engine, UiEditIntent::RemoveNode { node: last });
    assert_eq!(color(&mut engine, reference), [1.0, 0.0, 0.0, 1.0]);
    intent(&mut engine, UiEditIntent::RemoveNode { node: first });
    assert_eq!(color(&mut engine, reference), [0.0, 1.0, 0.0, 1.0]);

    assert!(engine.undo().unwrap());
    assert_eq!(color(&mut engine, reference), [1.0, 0.0, 0.0, 1.0]);
    control(
        &mut engine,
        parameter,
        ParameterControlMode::Manual,
        ParameterControlSpec::Manual,
    );
    assert_eq!(color(&mut engine, reference), [0.0, 1.0, 0.0, 1.0]);
    assert!(engine.undo().unwrap());
    assert_eq!(color(&mut engine, reference), [1.0, 0.0, 0.0, 1.0]);
}

#[test]
fn standalone_gradient_catalog_orders_stops_and_copies_their_values() {
    let mut engine = fixture();
    let context = UserContextNode::new("Colors");
    let context_uuid = context.node_data().meta.uuid;
    engine.edits.push(Edit::AddNode {
        parent: engine.root,
        node: Box::new(context),
        prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    let parent = engine.node_id_by_uuid(context_uuid).unwrap();
    assert!(
        engine
            .catalog_creatable_items(parent)
            .iter()
            .any(|item| item.node_type == GRADIENT_NODE_TYPE)
    );
    let created = intent(
        &mut engine,
        UiEditIntent::CreateUserItem {
            parent,
            node_type: GRADIENT_NODE_TYPE.into(),
            label: None,
            initial_params: vec![],
        },
    )
    .created_node
    .unwrap();
    let gradient = created.node_id;
    let first = engine.process_tree_snapshot().child_ids(gradient)[0];
    let last = engine.process_tree_snapshot().child_ids(gradient)[1];
    let first_position = child(&engine, first, GRADIENT_STOP_POSITION_DECL_ID);
    let last_position = child(&engine, last, GRADIENT_STOP_POSITION_DECL_ID);
    set(&mut engine, first_position, ParamValue::Float(0.75));
    set(&mut engine, last_position, ParamValue::Float(0.25));
    engine.run_tick(Duration::from_millis(50)).unwrap();
    engine.run_tick(Duration::from_millis(50)).unwrap();
    assert_eq!(engine.process_tree_snapshot().child_ids(gradient), vec![last, first]);

    intent(
        &mut engine,
        UiEditIntent::DuplicateNode {
            source: gradient,
            new_parent: parent,
            new_prev_sibling: Some(gradient),
            initial_params: vec![],
        },
    );
    let tree = engine.process_tree_snapshot();
    let copy = tree.child_ids(parent).into_iter().find(|id| *id != gradient).unwrap();
    let model = gradient_from_snapshot(&tree, gradient).unwrap();
    assert_eq!(gradient_from_snapshot(&tree, copy), Some(model.clone()));

    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    let restored = loaded.node_id_by_uuid(created.uuid).unwrap();
    assert_eq!(
        gradient_from_snapshot(&loaded.process_tree_snapshot(), restored),
        Some(model)
    );
}
