use super::*;
use golden_core::{
    edit::Edit,
    node::{Node, NodeId, NodeUuid},
    parameter::{ParamValue, ParameterEventBehaviour},
};
use std::collections::HashMap;

fn fields(engine: &AppEngine, instance: InstanceId) -> HashMap<String, NodeId> {
    let id = engine.node_id_by_uuid(instance.0).unwrap();
    let placement = super::super::children::placement(engine, id).unwrap();
    let folder = placement.node_data().first_child.unwrap();
    let mut next = engine.nodes.get(folder).unwrap().node_data().first_child;
    let mut fields = HashMap::new();
    while let Some(id) = next {
        let node = engine.nodes.get(id).unwrap();
        fields.insert(node.node_data().meta.decl_id.0.clone(), id);
        next = node.node_data().next_sibling;
    }
    fields
}

#[test]
fn source_drop_adds_at_canvas_position_with_backend_defaults_and_one_history_step() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let composition = create(&mut engine, MediaKind::Composition);
    add(&mut engine, composition, vec![source]);
    let before = inspect(&mut engine, composition);
    let history = engine.undo_len();
    let count = engine.nodes.len();
    let MediaEditResult::InstancesCreated { instances } = edit(
        &mut engine,
        composition,
        CompositionEdit::DropSource {
            copy: false,
            source: source.media.0,
            instance: None,
            position: Point { x: -120., y: 2230. },
        },
    ) else {
        panic!()
    };
    assert_eq!(instances.len(), 1);
    assert_eq!(engine.undo_len(), history + 1);
    assert_eq!(
        engine.nodes.len(),
        count + 16,
        "one complete detached instance subtree including Fit Mode"
    );
    let after = inspect(&mut engine, composition);
    assert_eq!(after.instances[0], before.instances[0]);
    assert_eq!(after.instances[1].reference, instances[0]);
    let expected = InstanceDefinition {
        source: Some(ChannelReference {
            media: MediaId(instances[0].instance.0),
            channel: source.channel,
        }),
        transform: Transform2D {
            position: [-120., 2230.],
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(after.instances[1].definition, expected);
    assert!(engine.undo().unwrap());
    assert_eq!(inspect(&mut engine, composition), before);
    assert!(engine.redo().unwrap());
    assert_eq!(inspect(&mut engine, composition), after);
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(inspect(&mut loaded, composition), after);
}

#[test]
fn source_replacement_preserves_exact_authored_parameters_labels_identity_and_peers() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::Picture);
    let replacement = create(&mut engine, MediaKind::SolidColor);
    let composition = create(&mut engine, MediaKind::Composition);
    let instances = add(&mut engine, composition, vec![source, source]);
    let target = instances[0].instance;
    let fields = fields(&engine, target);
    engine.edits.push(Edit::SetParamsAtomic {
        values: vec![
            (
                fields["parameters/position"],
                ParamValue::Vec2(100.123456789123, -20.987654321987),
            ),
            (
                fields["parameters/scale"],
                ParamValue::Vec2(-1.234567891234, 2.345678912345),
            ),
            (
                fields["parameters/pivot"],
                ParamValue::Vec2(0.123456789123, 0.987654321987),
            ),
            (
                fields["parameters/rotation_degrees"],
                ParamValue::Float(12.123456789123),
            ),
            (fields["parameters/opacity"], ParamValue::Float(0.987654321987)),
            (fields["parameters/blend"], ParamValue::Enum("Multiply".into())),
            (fields["parameters/render_enabled"], ParamValue::Bool(false)),
        ],
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    edit(
        &mut engine,
        composition,
        CompositionEdit::RenameInstance {
            instance: target,
            label: "Exact overlay".into(),
        },
    );
    let parameters = fields
        .iter()
        .filter(|(name, _)| !name.contains("/source_"))
        .map(|(_, id)| (*id, engine.nodes.get(*id).unwrap().engine_param_snapshot().unwrap()))
        .collect::<Vec<_>>();
    let before = inspect(&mut engine, composition);
    let document = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let MediaEditResult::InstanceChanged { snapshot } = edit(
        &mut engine,
        composition,
        CompositionEdit::DropSource {
            copy: false,
            source: replacement.channel.0,
            instance: Some(target),
            position: Point { x: 900., y: 800. },
        },
    ) else {
        panic!()
    };
    assert_eq!(engine.undo_len(), history + 1);
    let after = inspect(&mut engine, composition);
    let mut expected = before.instances[0].clone();
    expected.definition.source = Some(ChannelReference {
        media: MediaId(target.0),
        channel: replacement.channel,
    });
    assert_eq!(after.instances[0], expected);
    assert_eq!(snapshot, expected);
    assert_eq!(after.instances[1], before.instances[1]);
    for (id, snapshot) in parameters {
        assert_eq!(engine.nodes.get(id).unwrap().engine_param_snapshot().unwrap(), snapshot);
    }
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), document);
    assert!(engine.redo().unwrap());
    assert_eq!(inspect(&mut engine, composition), after);
}

#[test]
fn source_drop_rejects_cycles_foreign_or_stale_targets_and_invalid_sources_without_mutation() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let a = create(&mut engine, MediaKind::Composition);
    let b = create(&mut engine, MediaKind::Composition);
    let foreign = add(&mut engine, b, vec![a])[0].instance;
    let target = add(&mut engine, a, vec![source])[0].instance;
    let root = engine.nodes.get(engine.root).unwrap().node_data().meta.uuid;
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for (source, instance, position) in [
        (root, None, Point { x: 0., y: 0. }),
        (NodeUuid::nil(), None, Point { x: 0., y: 0. }),
        (a.media.0, None, Point { x: 0., y: 0. }),
        (b.channel.0, Some(target), Point { x: 0., y: 0. }),
        (source.media.0, Some(foreign), Point { x: 0., y: 0. }),
        (
            source.channel.0,
            Some(InstanceId(NodeUuid::nil())),
            Point { x: 0., y: 0. },
        ),
        (source.channel.0, None, Point { x: f32::NAN, y: 0. }),
    ] {
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::Composition {
                    reference: a,
                    edit: CompositionEdit::DropSource {
                        copy: false,
                        source,
                        instance,
                        position
                    }
                }
            )
            .success
        );
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
}
