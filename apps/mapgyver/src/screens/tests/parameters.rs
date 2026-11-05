use super::*;
use crate::app::AppNode;
use golden_core::{
    app::{ProjectNode, from_sparse_project_json, to_sparse_project_json_pretty, validate_engine_project_candidate},
    edit::Edit,
    engine::ProjectNodeRecord,
    node::{Node, NodeId, NodeReference, NodeUuid},
    parameter::{ParamValue, ParameterEventBehaviour, RangeConstraint},
    process_ctx::ExecutionPhase,
};
use std::collections::HashMap;

fn parameters(engine: &AppEngine, root: NodeId) -> HashMap<String, NodeId> {
    let mut child = engine.nodes.get(root).unwrap().node_data().first_child;
    while let Some(id) = child {
        let node = engine.nodes.get(id).unwrap();
        child = node.node_data().next_sibling;
        if !crate::parameters::is_parameter_folder(node) {
            continue;
        }
        let mut fields = HashMap::new();
        let mut parameter = node.node_data().first_child;
        while let Some(id) = parameter {
            let node = engine.nodes.get(id).unwrap();
            assert!(
                node.engine_param_snapshot().is_some(),
                "every property is a real Parameter"
            );
            fields.insert(node.node_data().meta.decl_id.0.clone(), id);
            parameter = node.node_data().next_sibling;
        }
        return fields;
    }
    panic!("missing Parameters folder")
}

fn set(engine: &mut AppEngine, node: NodeId, value: ParamValue) -> UiAck {
    apply_project_intent(
        engine,
        UiEditIntent::SetParam {
            node,
            value,
            behaviour: ParameterEventBehaviour::Append,
        },
    )
}

fn dispatch(engine: &mut AppEngine) {
    engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).unwrap();
}

fn atomic(engine: &mut AppEngine, values: Vec<(NodeId, ParamValue)>) -> Result<(), String> {
    engine.edits.push(Edit::SetParamsAtomic {
        values,
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().map_err(|error| error.to_string())?;
    dispatch(engine);
    Ok(())
}

fn screen_definition(engine: &AppEngine, screen: ScreenId) -> ScreenDefinition {
    super::super::inspect(engine)
        .unwrap()
        .screens
        .into_iter()
        .find(|item| item.id == screen)
        .unwrap()
        .definition
}

#[test]
fn shared_inspector_can_rename_screens_and_surfaces_and_reload_their_permissions() {
    assert!(
        super::super::ScreenNode::new()
            .node_data()
            .meta
            .user_permissions
            .can_edit_name
    );
    assert!(
        super::super::SurfaceNode::new()
            .node_data()
            .meta
            .user_permissions
            .can_edit_name
    );
    let mut engine = fixture();
    let (screen, surface) = create(&mut engine);
    let history = engine.undo_len();
    for (uuid, label) in [(screen.0, "Main output"), (surface.surface.0, "Stage mapping")] {
        let node = engine.node_id_by_uuid(uuid).unwrap();
        assert!(
            engine
                .nodes
                .get(node)
                .unwrap()
                .node_data()
                .meta
                .user_permissions
                .can_edit_name
        );
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::PatchMeta {
                node,
                patch: golden_core::node::NodeMetaPatch {
                    label: Some(label.into()),
                    ..Default::default()
                }
                .into(),
            },
        );
        assert!(ack.success, "{ack:?}");
        assert_eq!(engine.nodes.get(node).unwrap().node_data().meta.label, label);
    }
    assert_eq!(engine.undo_len(), history + 2);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    validate_engine_project_candidate(&loaded).unwrap();
    for uuid in [screen.0, surface.surface.0] {
        let original = engine.nodes.get(engine.node_id_by_uuid(uuid).unwrap()).unwrap();
        let restored = loaded.nodes.get(loaded.node_id_by_uuid(uuid).unwrap()).unwrap();
        assert_eq!(restored.node_data().meta.label, original.node_data().meta.label);
        assert_eq!(
            restored.node_data().meta.user_permissions,
            original.node_data().meta.user_permissions
        );
        assert!(restored.node_data().meta.user_permissions.can_edit_name);
    }
}

#[test]
fn generic_screen_and_surface_parameters_own_projection_history_and_project_state() {
    let mut engine = fixture();
    let (screen, surface) = create(&mut engine);
    let screen_node = engine.node_id_by_uuid(screen.0).unwrap();
    let surface_node = engine.node_id_by_uuid(surface.surface.0).unwrap();
    let screen_fields = parameters(&engine, screen_node);
    let surface_fields = parameters(&engine, surface_node);
    assert_eq!(screen_fields.len(), 14);
    assert!(!screen_fields.contains_key("parameters/render_enabled"));
    assert_eq!(surface_fields.len(), 10);
    assert!(!surface_fields.contains_key("parameters/render_enabled"));
    assert!(
        engine
            .nodes
            .get(screen_node)
            .unwrap()
            .project_encode_data()
            .unwrap()
            .is_null()
    );
    assert!(
        engine
            .nodes
            .get(surface_node)
            .unwrap()
            .project_encode_data()
            .unwrap()
            .is_null()
    );

    let history = engine.undo_len();
    assert!(set(&mut engine, screen_fields["parameters/width"], ParamValue::Int(640)).success);
    assert!(screen_definition(&engine, screen).enabled);
    assert!(
        set(
            &mut engine,
            screen_fields["parameters/sender"],
            ParamValue::Str("Parameter output".into())
        )
        .success
    );
    for enabled in [false, true] {
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::PatchMeta {
                node: screen_node,
                patch: golden_core::node::NodeMetaPatch {
                    enabled: Some(enabled),
                    ..Default::default()
                }
                .into(),
            },
        );
        assert!(ack.success, "{ack:?}");
        assert_eq!(screen_definition(&engine, screen).enabled, enabled);
    }
    assert_eq!(engine.undo_len(), history + 4);
    assert!(
        matches!(screen_definition(&engine, screen), ScreenDefinition { target: Some(ScreenTarget::Spout { sender, .. }), enabled: true } if sender == "Parameter output")
    );

    assert!(
        set(
            &mut engine,
            surface_fields["parameters/opacity"],
            ParamValue::Float(0.25)
        )
        .success
    );
    assert_eq!(definition(&engine, surface).opacity, 0.25);
    assert!(engine.undo().unwrap());
    dispatch(&mut engine);
    assert_eq!(definition(&engine, surface).opacity, 1.0);
    assert!(engine.redo().unwrap());
    dispatch(&mut engine);
    assert_eq!(definition(&engine, surface).opacity, 0.25);

    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for (node, value) in [
        (screen_fields["parameters/width"], ParamValue::Int(8193)),
        (screen_fields["parameters/sender"], ParamValue::Str("x".repeat(240))),
        (surface_fields["parameters/opacity"], ParamValue::Float(2.0)),
        (surface_fields["parameters/top_left"], ParamValue::Vec2(f64::MAX, 0.0)),
        (
            surface_fields["parameters/source_media"],
            ParamValue::Reference(NodeReference::new(screen.0)),
        ),
    ] {
        assert!(!set(&mut engine, node, value).success);
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }

    let snapshot = super::super::inspect(&engine).unwrap();
    let loaded = from_sparse_project_json::<AppNode>(&before).unwrap();
    validate_engine_project_candidate(&loaded).unwrap();
    assert_eq!(super::super::inspect(&loaded).unwrap(), snapshot);
    let loaded_screen_fields = parameters(&loaded, loaded.node_id_by_uuid(screen.0).unwrap());
    let loaded_surface_fields = parameters(&loaded, loaded.node_id_by_uuid(surface.surface.0).unwrap());
    assert_eq!(loaded_screen_fields.len(), 14);
    assert_eq!(loaded_surface_fields.len(), 10);
    // Sparse projects omit unchanged, unreferenced declared leaves. Their schema
    // declarations restore the same parameter configuration with fresh identities.
    for (original_fields, loaded_fields) in [
        (&screen_fields, &loaded_screen_fields),
        (&surface_fields, &loaded_surface_fields),
    ] {
        for (name, id) in original_fields {
            assert_eq!(
                loaded.nodes.get(loaded_fields[name]).unwrap().engine_param_snapshot(),
                engine.nodes.get(*id).unwrap().engine_param_snapshot()
            );
        }
    }
    // Authored overrides are present in the sparse document and must retain their
    // exact UUIDs, rather than merely being recreated with equivalent values.
    for (original_id, loaded_id) in [
        (
            screen_fields["parameters/width"],
            loaded_screen_fields["parameters/width"],
        ),
        (
            screen_fields["parameters/sender"],
            loaded_screen_fields["parameters/sender"],
        ),
        (
            surface_fields["parameters/opacity"],
            loaded_surface_fields["parameters/opacity"],
        ),
    ] {
        let uuid = engine.nodes.get(original_id).unwrap().node_data().meta.uuid;
        assert_eq!(loaded.node_id_by_uuid(uuid), Some(loaded_id));
    }
}

#[test]
fn crop_parameters_expose_unit_bounds_and_normalize_containment_before_history() {
    let mut engine = fixture();
    let (_, surface) = create(&mut engine);
    let root = engine.node_id_by_uuid(surface.surface.0).unwrap();
    let fields = parameters(&engine, root);
    let history = engine.undo_len();
    for field in ["parameters/crop_origin", "parameters/crop_size"] {
        let id = fields[field];
        let snapshot = engine.nodes.get(id).unwrap().engine_param_snapshot().unwrap();
        assert_eq!(
            snapshot.constraints.range,
            RangeConstraint::components(Some(vec![0.0, 0.0]), Some(vec![1.0, 1.0]))
        );
    }
    assert!(
        set(
            &mut engine,
            fields["parameters/crop_origin"],
            ParamValue::Vec2(0.9, 0.9)
        )
        .success
    );
    assert_eq!(definition(&engine, surface).crop, UnitRect::FULL);
    assert_eq!(engine.undo_len(), history, "a clamped translation no-op has no history");
    assert!(set(&mut engine, fields["parameters/crop_size"], ParamValue::Vec2(0.6, 0.5)).success);
    assert_eq!(
        engine
            .nodes
            .get(fields["parameters/crop_size"])
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .value,
        ParamValue::Vec2(0.6, 0.5),
        "valid authored values retain their precision"
    );
    let sized = definition(&engine, surface);
    assert!(
        set(
            &mut engine,
            fields["parameters/crop_origin"],
            ParamValue::Vec2(0.9, -0.2)
        )
        .success
    );
    let translated = definition(&engine, surface);
    assert!(translated.crop.is_valid());
    assert!((translated.crop.x - 0.4).abs() < 1e-6);
    assert_eq!(translated.crop.y, 0.0);
    assert_eq!(
        (translated.crop.width, translated.crop.height),
        (sized.crop.width, sized.crop.height)
    );
    assert!(set(&mut engine, fields["parameters/crop_size"], ParamValue::Vec2(1.5, 1.5)).success);
    let expanded = definition(&engine, surface);
    assert!(expanded.crop.is_valid());
    assert_eq!(
        (expanded.crop.x, expanded.crop.y),
        (translated.crop.x, translated.crop.y)
    );
    assert!((expanded.crop.x + expanded.crop.width - 1.0).abs() < 1e-6);
    assert_eq!(expanded.crop.height, 1.0);
    assert_eq!(engine.undo_len(), history + 3);
    for expected in [&translated, &sized] {
        assert!(engine.undo().unwrap());
        assert_eq!(&definition(&engine, surface), expected);
    }
    for expected in [&translated, &expanded] {
        assert!(engine.redo().unwrap());
        assert_eq!(&definition(&engine, surface), expected);
    }
    assert!(set(&mut engine, fields["parameters/crop_size"], ParamValue::Vec2(0.0, 0.0)).success);
    assert!(definition(&engine, surface).crop.is_valid());
}

#[test]
fn inspector_crop_samples_remain_contained_and_one_undo_restores_the_complete_surface() {
    let mut engine = fixture();
    let (_, surface) = create(&mut engine);
    let root = engine.node_id_by_uuid(surface.surface.0).unwrap();
    let fields = parameters(&engine, root);
    let original = definition(&engine, surface);
    let history = engine.undo_len();
    super::gestures::session(&mut engine, true);
    for index in 1..=50 {
        assert!(
            set(
                &mut engine,
                fields["parameters/crop_size"],
                ParamValue::Vec2(1.0 - f64::from(index) / 100.0, 0.8),
            )
            .success
        );
        assert!(
            set(
                &mut engine,
                fields["parameters/crop_origin"],
                ParamValue::Vec2(0.95, 0.95),
            )
            .success
        );
        assert!(definition(&engine, surface).crop.is_valid());
    }
    let accepted = definition(&engine, surface);
    assert_eq!(
        SurfaceDefinition {
            crop: original.crop,
            ..accepted.clone()
        },
        original
    );
    assert!(
        !set(
            &mut engine,
            fields["parameters/top_left"],
            ParamValue::Vec2(f64::INFINITY, 1.0)
        )
        .success
    );
    assert!(
        !set(
            &mut engine,
            fields["parameters/crop_origin"],
            ParamValue::Vec2(f64::NAN, 0.0)
        )
        .success
    );
    assert_eq!(definition(&engine, surface), accepted);
    super::gestures::session(&mut engine, false);
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert_eq!(definition(&engine, surface), original);
    assert!(engine.redo().unwrap());
    assert_eq!(definition(&engine, surface), accepted);
}

#[test]
fn atomic_mapping_uses_the_complete_quad_and_crop_and_undo_restores_the_complete_state() {
    let mut engine = fixture();
    let (_, surface) = create(&mut engine);
    let root = engine.node_id_by_uuid(surface.surface.0).unwrap();
    let fields = parameters(&engine, root);
    let original = definition(&engine, surface);
    let history = engine.undo_len();

    // Atomic edits retain their single history entry; individual invalid intermediate
    // shapes are covered separately by the free-warp gesture tests.
    assert!(
        set(
            &mut engine,
            fields["parameters/crop_origin"],
            ParamValue::Vec2(0.75, 0.75)
        )
        .success
    );
    assert_eq!(definition(&engine, surface), original);
    assert_eq!(engine.undo_len(), history);
    atomic(
        &mut engine,
        vec![
            (fields["parameters/top_left"], ParamValue::Vec2(1.0, 1.0)),
            (fields["parameters/top_right"], ParamValue::Vec2(0.0, 1.0)),
            (fields["parameters/bottom_right"], ParamValue::Vec2(0.0, 0.0)),
            (fields["parameters/bottom_left"], ParamValue::Vec2(1.0, 0.0)),
            (fields["parameters/crop_origin"], ParamValue::Vec2(0.75, 0.75)),
            (fields["parameters/crop_size"], ParamValue::Vec2(0.25, 0.25)),
        ],
    )
    .unwrap();
    let rotated = definition(&engine, surface);
    assert_eq!(
        rotated.quad.corners,
        [
            Point { x: 1.0, y: 1.0 },
            Point { x: 0.0, y: 1.0 },
            Point { x: 0.0, y: 0.0 },
            Point { x: 1.0, y: 0.0 }
        ]
    );
    assert_eq!(
        rotated.crop,
        UnitRect {
            x: 0.75,
            y: 0.75,
            width: 0.25,
            height: 0.25
        }
    );
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    dispatch(&mut engine);
    assert_eq!(definition(&engine, surface), original);
    assert!(engine.redo().unwrap());
    dispatch(&mut engine);
    assert_eq!(definition(&engine, surface), rotated);

    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    assert!(
        atomic(
            &mut engine,
            vec![
                (fields["parameters/opacity"], ParamValue::Float(0.5)),
                (fields["parameters/top_left"], ParamValue::Vec2(f64::INFINITY, 0.0)),
            ]
        )
        .is_err()
    );
    assert_eq!(definition(&engine, surface), rotated);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}

#[test]
fn inspector_corner_parameters_accept_invalid_intermediate_shapes_and_recover() {
    let mut engine = fixture();
    let (_, surface) = create(&mut engine);
    let root = engine.node_id_by_uuid(surface.surface.0).unwrap();
    let fields = parameters(&engine, root);
    let original = definition(&engine, surface);
    let history = engine.undo_len();
    super::gestures::session(&mut engine, true);
    for (index, (field, value)) in [
        ("parameters/top_left", (1.0, 1.0)),
        ("parameters/top_right", (0.0, 1.0)),
        ("parameters/bottom_right", (0.0, 0.0)),
        ("parameters/bottom_left", (1.0, 0.0)),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(set(&mut engine, fields[field], ParamValue::Vec2(value.0, value.1)).success);
        let ScreenEditResult::Surface { snapshot } =
            apply(&mut engine, ScreenEdit::InspectSurface { reference: surface })
        else {
            panic!()
        };
        assert_eq!(snapshot.mapping_valid, index == 3);
        assert_eq!(
            snapshot.definition.quad.corners[index],
            Point {
                x: value.0 as f32,
                y: value.1 as f32
            }
        );
    }
    super::gestures::session(&mut engine, false);
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    dispatch(&mut engine);
    assert_eq!(definition(&engine, surface), original);
}

#[test]
fn atomic_screen_endpoint_swaps_validate_final_uniqueness_and_rectangle_bounds() {
    let mut engine = fixture();
    let (first, _) = create(&mut engine);
    let (second, _) = create(&mut engine);
    for (screen, sender) in [(first, "First output"), (second, "Second output")] {
        apply(
            &mut engine,
            ScreenEdit::ConfigureScreen {
                screen,
                definition: ScreenDefinition {
                    target: Some(ScreenTarget::Spout {
                        sender: sender.into(),
                        extent: golden_video::Extent2D::new(1920, 1080).unwrap(),
                    }),
                    enabled: true,
                },
            },
        );
    }
    let first_fields = parameters(&engine, engine.node_id_by_uuid(first.0).unwrap());
    let second_fields = parameters(&engine, engine.node_id_by_uuid(second.0).unwrap());
    let original = super::super::inspect(&engine).unwrap();
    let history = engine.undo_len();
    assert!(
        !set(
            &mut engine,
            first_fields["parameters/sender"],
            ParamValue::Str("Second output".into())
        )
        .success
    );
    assert_eq!(super::super::inspect(&engine).unwrap(), original);
    assert_eq!(engine.undo_len(), history);
    atomic(
        &mut engine,
        vec![
            (
                first_fields["parameters/sender"],
                ParamValue::Str("Second output".into()),
            ),
            (
                second_fields["parameters/sender"],
                ParamValue::Str("First output".into()),
            ),
        ],
    )
    .unwrap();
    let swapped = super::super::inspect(&engine).unwrap();
    assert_eq!(engine.undo_len(), history + 1);
    assert!(
        matches!(screen_definition(&engine, first).target, Some(ScreenTarget::Spout { sender, .. }) if sender == "Second output")
    );
    assert!(
        matches!(screen_definition(&engine, second).target, Some(ScreenTarget::Spout { sender, .. }) if sender == "First output")
    );
    assert!(engine.undo().unwrap());
    dispatch(&mut engine);
    assert_eq!(super::super::inspect(&engine).unwrap(), original);
    assert!(engine.redo().unwrap());
    dispatch(&mut engine);
    assert_eq!(super::super::inspect(&engine).unwrap(), swapped);
    let history = engine.undo_len();
    assert!(
        atomic(
            &mut engine,
            vec![
                (first_fields["parameters/sender"], ParamValue::Str("Collision".into())),
                (second_fields["parameters/sender"], ParamValue::Str("Collision".into())),
            ]
        )
        .is_err()
    );
    assert_eq!(super::super::inspect(&engine).unwrap(), swapped);
    assert_eq!(engine.undo_len(), history);

    assert!(
        set(
            &mut engine,
            first_fields["parameters/target_kind"],
            ParamValue::Enum("Rectangle".into())
        )
        .success
    );
    assert!(
        !set(
            &mut engine,
            first_fields["parameters/left"],
            ParamValue::Int(i32::MAX - 100)
        )
        .success
    );
    atomic(
        &mut engine,
        vec![
            (first_fields["parameters/left"], ParamValue::Int(i32::MAX - 100)),
            (first_fields["parameters/width"], ParamValue::Int(100)),
        ],
    )
    .unwrap();
    let definition = screen_definition(&engine, first);
    assert!(
        matches!(definition.target, Some(ScreenTarget::Rectangle { bounds }) if bounds.left == i32::MAX - 100 && bounds.extent.width() == 100)
    );
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    assert!(
        atomic(
            &mut engine,
            vec![
                (first_fields["parameters/left"], ParamValue::Int(i32::MAX - 50)),
                (first_fields["parameters/width"], ParamValue::Int(100)),
            ]
        )
        .is_err()
    );
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}

#[test]
fn copying_a_surface_preserves_parameters_controls_and_shared_sources_independently() {
    use crate::media::protocol::{MEDIA_NAMESPACE, MEDIA_OPERATION, MediaEdit, MediaEditResult, MediaKind};
    use golden_core::parameter::{ParameterControlMode, ParameterControlSpec, ParameterControlState};

    let mut engine = fixture();
    let (screen, original) = create(&mut engine);
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(MediaEdit::CreateMedia {
                media_kind: MediaKind::SolidColor,
                label: None,
            })
            .unwrap(),
        },
    );
    assert!(ack.success, "{ack:?}");
    let MediaEditResult::Created { reference: source } = serde_json::from_value(ack.result.unwrap().value).unwrap()
    else {
        panic!("expected a media channel")
    };
    let root = engine.node_id_by_uuid(original.surface.0).unwrap();
    let fields = parameters(&engine, root);
    atomic(
        &mut engine,
        vec![
            (
                fields["parameters/source_media"],
                ParamValue::Reference(NodeReference::new(source.media.0)),
            ),
            (
                fields["parameters/source_channel"],
                ParamValue::Reference(NodeReference::new(source.channel.0)),
            ),
        ],
    )
    .unwrap();
    let opacity = fields["parameters/opacity"];
    // User-editable constraints are authored; declaration-owned constraints are
    // reconstructed by the sparse codec instead of saved as overrides.
    engine
        .nodes
        .get_mut(opacity)
        .unwrap()
        .node_data_mut()
        .meta
        .user_permissions
        .can_edit_constraints = true;
    let mut constraints = engine
        .nodes
        .get(opacity)
        .unwrap()
        .engine_param_snapshot()
        .unwrap()
        .constraints;
    constraints.step = Some(0.25);
    engine.edits.push(Edit::SetParam {
        node: opacity,
        value: ParamValue::Float(0.25),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.edits.push(Edit::SetParamConstraints {
        node: opacity,
        constraints,
    });
    engine.edits.push(Edit::SetParamControlState {
        node: opacity,
        state: ParameterControlState::new(ParameterControlMode::Expression, ParameterControlSpec::Expression),
    });
    engine.edits.push(Edit::PatchMeta {
        node: root,
        patch: golden_core::node::NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    dispatch(&mut engine);
    let original_definition = definition(&engine, original);
    assert!(!original_definition.enabled);
    assert_eq!(original_definition.source, Some(source));
    let parameter = engine.nodes.get(opacity).unwrap().engine_param_snapshot().unwrap();
    assert_ne!(parameter.value, parameter.default_value);
    let control = engine.nodes.get(opacity).unwrap().node_data().first_child.unwrap();
    let before = super::super::inspect(&engine).unwrap();
    let history = engine.undo_len();
    let screen_root = engine.node_id_by_uuid(screen.0).unwrap();
    let copied_root = engine
        .duplicate_subtree_with(
            root,
            screen_root,
            Some(root),
            None,
            Node::project_encode_data,
            AppNode::project_decode_node,
        )
        .unwrap();
    dispatch(&mut engine);
    let copied = SurfaceReference {
        screen,
        surface: SurfaceId(engine.nodes.get(copied_root).unwrap().node_data().meta.uuid),
    };
    assert_eq!(engine.undo_len(), history + 1);
    assert_eq!(definition(&engine, copied), original_definition);
    let copied_fields = parameters(&engine, copied_root);
    assert_eq!(copied_fields.len(), 10);
    for (name, original_id) in &fields {
        let copied_id = copied_fields[name];
        let original_node = engine.nodes.get(*original_id).unwrap();
        let copied_node = engine.nodes.get(copied_id).unwrap();
        assert_ne!(*original_id, copied_id);
        assert_ne!(original_node.node_data().meta.uuid, copied_node.node_data().meta.uuid);
        assert_eq!(
            original_node.engine_param_snapshot(),
            copied_node.engine_param_snapshot()
        );
    }
    let copied_opacity = copied_fields["parameters/opacity"];
    let copied_control = engine
        .nodes
        .get(copied_opacity)
        .unwrap()
        .node_data()
        .first_child
        .unwrap();
    assert_ne!(control, copied_control);
    assert_ne!(
        engine.nodes.get(control).unwrap().node_data().meta.uuid,
        engine.nodes.get(copied_control).unwrap().node_data().meta.uuid
    );
    assert_eq!(
        engine.nodes.get(control).unwrap().get_type(),
        engine.nodes.get(copied_control).unwrap().get_type()
    );
    let after = super::super::inspect(&engine).unwrap();
    assert!(engine.undo().unwrap());
    dispatch(&mut engine);
    assert_eq!(super::super::inspect(&engine).unwrap(), before);
    assert!(engine.redo().unwrap());
    dispatch(&mut engine);
    assert_eq!(super::super::inspect(&engine).unwrap(), after);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    validate_engine_project_candidate(&loaded).unwrap();
    assert_eq!(super::super::inspect(&loaded).unwrap(), after);
    let mut declared = fixture();
    let (_, declared_surface) = create(&mut declared);
    let declared_root = declared.node_id_by_uuid(declared_surface.surface.0).unwrap();
    let declared_fields = parameters(&declared, declared_root);
    let mut loaded_parameters = HashMap::new();
    for (original_fields, reference) in [(&fields, original), (&copied_fields, copied)] {
        let loaded_root = loaded.node_id_by_uuid(reference.surface.0).unwrap();
        let loaded_fields = parameters(&loaded, loaded_root);
        for (name, id) in original_fields {
            loaded_parameters.insert(*id, loaded_fields[name]);
        }
    }
    for id in fields
        .values()
        .chain(copied_fields.values())
        .copied()
        .chain([control, copied_control])
    {
        let node = engine.nodes.get(id).unwrap();
        // Sparse reconstructs unchanged declared defaults (see the generic Parameter
        // roundtrip fixture above). Authored values, constraints and controls retain UUIDs.
        let default_leaf = declared_fields.get(&node.node_data().meta.decl_id.0).is_some_and(|id| {
            let baseline = declared.nodes.get(*id).unwrap();
            let mut metadata = baseline.node_data().meta.clone();
            metadata.uuid = node.node_data().meta.uuid;
            baseline.engine_param_snapshot() == node.engine_param_snapshot() && metadata == node.node_data().meta
        });
        let loaded_id = loaded
            .node_id_by_uuid(node.node_data().meta.uuid)
            .or_else(|| default_leaf.then(|| loaded_parameters[&id]))
            .unwrap_or_else(|| panic!("authored sparse UUID missing for {}", node.node_data().meta.decl_id.0));
        assert_eq!(
            loaded.nodes.get(loaded_id).unwrap().engine_param_snapshot(),
            node.engine_param_snapshot()
        );
        assert_eq!(
            loaded.nodes.get(loaded_id).unwrap().node_data().meta.user_permissions,
            node.node_data().meta.user_permissions
        );
    }
}

#[test]
fn malformed_screen_and_surface_parameter_records_fail_public_candidate_validation() {
    for (surface, field, value) in [
        (false, "parameters/width", ParamValue::Int(8193)),
        (false, "parameters/sender", ParamValue::Str("invalid\0sender".into())),
        (true, "parameters/calibration", ParamValue::Enum("unknown".into())),
        (true, "parameters/crop_size", ParamValue::Vec2(0.0, 1.0)),
    ] {
        let mut engine = fixture();
        let (screen, reference) = create(&mut engine);
        let root = engine
            .node_id_by_uuid(if surface { reference.surface.0 } else { screen.0 })
            .unwrap();
        let id = parameters(&engine, root)[field];
        let uuid = engine.nodes.get(id).unwrap().node_data().meta.uuid;
        let mut project = engine.to_project_file_with(Node::project_encode_data).unwrap();
        assert!(corrupt_record(
            &mut project.root,
            uuid,
            &serde_json::to_value(value).unwrap()
        ));
        let result = AppEngine::from_project_file_with(project, AppNode::project_decode_node)
            .map_err(|error| error.to_string())
            .and_then(|loaded| validate_engine_project_candidate(&loaded));
        assert!(result.is_err(), "malformed {field} must fail candidate validation");
    }
}

fn corrupt_record(record: &mut ProjectNodeRecord, uuid: NodeUuid, value: &serde_json::Value) -> bool {
    if record.uuid == uuid {
        record.data = Some(serde_json::json!({ "value": value }));
        return true;
    }
    record
        .children
        .iter_mut()
        .any(|child| corrupt_record(child, uuid, value))
}
