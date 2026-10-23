use super::engine::*;
use super::*;
use crate::app::{
    module_command::{
        ModuleCommandDeliveryPolicy, ModuleCommandExecuteEvent, ModuleCommandParamOverride,
        MODULE_COMMAND_EXECUTE_TOPIC,
    },
    MouseMoveCommand, MouseScrollCommand,
};
use golden_core::{edit::Edit, parameter::ParamValue};
fn add(
    engine: &mut crate::app::AppEngine,
    id: golden_core::node::NodeId,
    node: crate::app::AppNode,
) -> golden_core::node::NodeId {
    let tester = path(engine, id, "command_tester");
    engine.add_user_item(node, Some(tester));
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    let mut child = engine.nodes.get(tester).unwrap().node_data().first_child.unwrap();
    while let Some(next) = engine.nodes.get(child).unwrap().node_data().next_sibling {
        child = next;
    }
    child
}
#[test]
fn scripts_preserve_absolute_relative_normalized_moves_and_signed_two_axis_scroll() {
    let control = Control::default();
    let (mut engine, id, slots) = create(control.clone());
    for args in [
        vec![ParamValue::Float(10.0), ParamValue::Float(20.0)],
        vec![
            ParamValue::Float(-3.0),
            ParamValue::Float(5.0),
            ParamValue::Str("relative".into()),
        ],
        vec![
            ParamValue::Float(0.25),
            ParamValue::Float(0.75),
            ParamValue::Enum("absolute".into()),
            ParamValue::Enum("normalized".into()),
        ],
    ] {
        invoke(&mut engine, id, "moveMouse", args);
        quiet(&mut engine, id);
    }
    invoke(&mut engine, id, "scroll", vec![ParamValue::Int(-2), ParamValue::Int(3)]);
    quiet(&mut engine, id);
    assert_eq!(
        *control.calls.lock().unwrap(),
        [
            Request::Move {
                x: 10.0,
                y: 20.0,
                coordinate: MouseMoveCoordinate::Absolute,
                units: MouseMoveUnits::Pixels
            },
            Request::Move {
                x: -3.0,
                y: 5.0,
                coordinate: MouseMoveCoordinate::Relative,
                units: MouseMoveUnits::Pixels
            },
            Request::Move {
                x: 0.25,
                y: 0.75,
                coordinate: MouseMoveCoordinate::Absolute,
                units: MouseMoveUnits::Normalized
            },
            Request::Scroll {
                vertical: -2,
                horizontal: 3
            }
        ]
    );
    finish(&mut engine, id, &slots);
}
#[test]
fn invalid_move_and_scroll_never_reach_native_and_later_calls_still_work() {
    let control = Control::default();
    let (mut engine, id, slots) = create(control.clone());
    for args in [
        vec![ParamValue::Float(f64::NAN), ParamValue::Float(0.0)],
        vec![ParamValue::Float(f64::MAX), ParamValue::Float(0.0)],
        vec![
            ParamValue::Int(1),
            ParamValue::Int(2),
            ParamValue::Str("relative".into()),
            ParamValue::Str("normalized".into()),
        ],
    ] {
        invoke(&mut engine, id, "moveMouse", args);
        quiet(&mut engine, id);
    }
    invoke(
        &mut engine,
        id,
        "scroll",
        vec![ParamValue::Int(1), ParamValue::Str("bad".into())],
    );
    quiet(&mut engine, id);
    invoke(
        &mut engine,
        id,
        "click",
        vec![ParamValue::Str("left".into()), ParamValue::Str("extra".into())],
    );
    quiet(&mut engine, id);
    assert!(control.calls.lock().unwrap().is_empty());
    invoke(&mut engine, id, "click", vec![]);
    quiet(&mut engine, id);
    assert_eq!(*control.calls.lock().unwrap(), [request(MouseButtonKind::Left)]);
    finish(&mut engine, id, &slots);
}
#[test]
fn movement_command_freezes_values_and_last_overrides_without_authored_mutation() {
    let control = Control::default();
    let (mut engine, id, slots) = create(control.clone());
    let command = add(&mut engine, id, MouseMoveCommand::create().into());
    let x = path(&engine, command, "x");
    let y = path(&engine, command, "y");
    let event = ModuleCommandExecuteEvent {
        command_id: command,
        invocation_id: None,
        delivery_policy: ModuleCommandDeliveryPolicy::Standard,
        param_overrides: vec![
            ModuleCommandParamOverride {
                param_id: x,
                value: ParamValue::Float(1.0),
            },
            ModuleCommandParamOverride {
                param_id: x,
                value: ParamValue::Float(9.0),
            },
            ModuleCommandParamOverride {
                param_id: y,
                value: ParamValue::Float(-4.0),
            },
        ],
    };
    engine.edits.push(Edit::EmitCustomEvent {
        event: golden_core::events::CustomEvent::new(
            MODULE_COMMAND_EXECUTE_TOPIC,
            Some(command),
            serde_json::to_value(event).unwrap(),
        ),
    });
    engine.apply_edits().unwrap();
    invoke(&mut engine, id, "scroll", vec![ParamValue::Int(2)]);
    set(&mut engine, x, ParamValue::Float(99.0));
    quiet(&mut engine, id);
    assert_eq!(
        *control.calls.lock().unwrap(),
        [
            Request::Move {
                x: 9.0,
                y: -4.0,
                coordinate: MouseMoveCoordinate::Absolute,
                units: MouseMoveUnits::Pixels
            },
            Request::Scroll {
                vertical: 2,
                horizontal: 0
            }
        ]
    );
    assert_eq!(value(&engine, x), ParamValue::Float(99.0));
    finish(&mut engine, id, &slots);
}
#[test]
fn scroll_command_is_registered_and_keeps_both_signed_axes() {
    let control = Control::default();
    let (mut engine, id, slots) = create(control.clone());
    let command = add(&mut engine, id, MouseScrollCommand::create().into());
    let vertical = path(&engine, command, "vertical");
    let horizontal = path(&engine, command, "horizontal");
    set(&mut engine, vertical, ParamValue::Int(-7));
    set(&mut engine, horizontal, ParamValue::Int(11));
    let execute = path(&engine, command, "trigger");
    set(&mut engine, execute, ParamValue::Trigger());
    quiet(&mut engine, id);
    assert_eq!(
        *control.calls.lock().unwrap(),
        [Request::Scroll {
            vertical: -7,
            horizontal: 11
        }]
    );
    finish(&mut engine, id, &slots);
}

#[test]
fn authored_choice_ids_accept_the_existing_backend_values_without_triggering_output() {
    let control = Control::default();
    let (mut engine, id, slots) = create(control.clone());
    let button = command(&mut engine, id);
    let movement = add(&mut engine, id, MouseMoveCommand::create().into());
    for (node, field, values) in [
        (button, "button", &["left", "middle", "right"][..]),
        (button, "action", &["click", "press", "release"][..]),
        (movement, "coordinate", &["absolute", "relative"][..]),
        (movement, "units", &["pixels", "normalized"][..]),
    ] {
        let parameter = path(&engine, node, field);
        for value in values {
            set(&mut engine, parameter, ParamValue::Enum((*value).into()));
        }
    }
    quiet(&mut engine, id);
    assert!(control.calls.lock().unwrap().is_empty());
    finish(&mut engine, id, &slots);
}
