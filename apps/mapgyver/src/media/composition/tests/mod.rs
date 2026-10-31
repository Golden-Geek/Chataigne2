use super::protocol::*;
use crate::{
    app::{AppEngine, AppNode},
    media::{
        protocol::*,
        tests::{apply, create, fixture, invoke},
    },
};
use golden_core::app::{from_sparse_project_json, to_sparse_project_json_pretty};

fn edit(engine: &mut AppEngine, reference: ChannelReference, edit: CompositionEdit) -> MediaEditResult {
    apply(engine, MediaEdit::Composition { reference, edit })
}
fn inspect(engine: &mut AppEngine, reference: ChannelReference) -> CompositionSnapshot {
    let MediaEditResult::Composition { snapshot } = edit(engine, reference, CompositionEdit::Inspect) else {
        panic!()
    };
    snapshot
}
fn add(engine: &mut AppEngine, reference: ChannelReference, sources: Vec<ChannelReference>) -> Vec<InstanceReference> {
    let MediaEditResult::InstancesCreated { instances } =
        edit(engine, reference, CompositionEdit::AddInstances { sources })
    else {
        panic!()
    };
    instances
}

#[test]
fn independent_instances_copy_order_remove_and_roundtrip_with_atomic_history() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let composition = create(&mut engine, MediaKind::Composition);
    let history = engine.undo_len();
    let instances = add(&mut engine, composition, vec![source, source]);
    assert_ne!(instances[0], instances[1]);
    assert_eq!(engine.undo_len(), history + 1);
    let initial = inspect(&mut engine, composition);
    assert_eq!(
        initial.instances.iter().map(|node| node.reference).collect::<Vec<_>>(),
        instances
    );
    assert_eq!(initial.instances[0].definition.transform.position, [960.0, 540.0]);
    let source_before = crate::media::channel(&engine, source).unwrap();
    let mut definition = initial.instances[0].definition.clone();
    definition.transform = Transform2D {
        position: [40.0, 60.0],
        scale: [-1.5, 2.0],
        rotation: 0.25,
        pivot: [0.2, 0.8],
    };
    definition.opacity = 0.4;
    definition.blend = BlendMode::Multiply;
    edit(
        &mut engine,
        composition,
        CompositionEdit::ConfigureInstance {
            instance: instances[0].instance,
            definition: definition.clone(),
        },
    );
    let changed = inspect(&mut engine, composition);
    assert_eq!(changed.instances[0].definition, definition);
    assert_eq!(changed.instances[1], initial.instances[1]);
    assert_eq!(crate::media::channel(&engine, source).unwrap(), source_before);
    let MediaEditResult::InstancesCreated { instances: copies } = edit(
        &mut engine,
        composition,
        CompositionEdit::DuplicateInstances {
            instances: vec![instances[0].instance],
        },
    ) else {
        panic!()
    };
    let copied = inspect(&mut engine, composition);
    assert_eq!(copied.instances[2].definition, definition);
    assert_eq!(copied.instances[2].reference, copies[0]);
    edit(
        &mut engine,
        composition,
        CompositionEdit::ReorderInstance {
            instance: copies[0].instance,
            after: None,
        },
    );
    assert_eq!(inspect(&mut engine, composition).instances[0].reference, copies[0]);
    edit(
        &mut engine,
        composition,
        CompositionEdit::RenameInstance {
            instance: copies[0].instance,
            label: "Overlay".into(),
        },
    );
    edit(
        &mut engine,
        composition,
        CompositionEdit::RemoveInstance {
            instance: instances[0].instance,
        },
    );
    assert!(engine.undo().unwrap());
    let saved = inspect(&mut engine, composition);
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    crate::media::validate_project(&loaded).unwrap();
    assert_eq!(inspect(&mut loaded, composition), saved);
}

#[test]
fn cycles_invalid_transforms_and_wrong_owners_leave_document_and_history_untouched() {
    let mut engine = fixture();
    let a = create(&mut engine, MediaKind::Composition);
    let b = create(&mut engine, MediaKind::Composition);
    let c = create(&mut engine, MediaKind::Composition);
    let source = create(&mut engine, MediaKind::SolidColor);
    let owned = add(&mut engine, a, vec![b]);
    add(&mut engine, b, vec![c]);
    let mut invalid = InstanceDefinition::default();
    invalid.transform.scale[0] = 0.0;
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for (reference, edit) in [
        (c, CompositionEdit::AddInstances { sources: vec![a] }),
        (a, CompositionEdit::AddInstances { sources: vec![a] }),
        (source, CompositionEdit::Inspect),
        (
            b,
            CompositionEdit::ConfigureInstance {
                instance: owned[0].instance,
                definition: InstanceDefinition::default(),
            },
        ),
        (
            a,
            CompositionEdit::ConfigureInstance {
                instance: owned[0].instance,
                definition: invalid,
            },
        ),
        (
            a,
            CompositionEdit::ReorderInstance {
                instance: owned[0].instance,
                after: Some(owned[0].instance),
            },
        ),
        (a, CompositionEdit::AddInstances { sources: Vec::new() }),
        (
            a,
            CompositionEdit::AddInstances {
                sources: vec![source; 4097],
            },
        ),
    ] {
        assert!(!invoke(&mut engine, MediaEdit::Composition { reference, edit }).success);
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    }
    crate::media::validate_project(&engine).unwrap();
}

#[test]
fn deleting_a_source_preserves_instance_identity_and_undo_restores_the_binding() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::Picture);
    let composition = create(&mut engine, MediaKind::Composition);
    add(&mut engine, composition, vec![source]);
    let before = inspect(&mut engine, composition);
    apply(&mut engine, MediaEdit::RemoveMedia { media: source.media });
    assert_eq!(inspect(&mut engine, composition), before);
    assert!(crate::media::channel(&engine, source).is_err());
    crate::media::validate_project(&engine).unwrap();
    let mut definition = before.instances[0].definition.clone();
    definition.opacity = 0.3;
    edit(
        &mut engine,
        composition,
        CompositionEdit::ConfigureInstance {
            instance: before.instances[0].reference.instance,
            definition,
        },
    );
    assert!(engine.undo().unwrap());
    assert!(engine.undo().unwrap());
    assert!(crate::media::channel(&engine, source).is_ok());
    assert_eq!(inspect(&mut engine, composition), before);
}

#[test]
fn a_thousand_instances_are_one_forest_and_one_undo_without_source_duplication() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let composition = create(&mut engine, MediaKind::Composition);
    let before = engine.nodes.len();
    let history = engine.undo_len();
    let started = std::time::Instant::now();
    let instances = add(&mut engine, composition, vec![source; 1000]);
    eprintln!("Composition 1000-instance forest: {:?}", started.elapsed());
    assert_eq!(engine.nodes.len(), before + 1000);
    assert_eq!(engine.undo_len(), history + 1);
    assert_eq!(inspect(&mut engine, composition).instances.len(), 1000);
    assert!(engine.undo().unwrap());
    assert_eq!(engine.nodes.len(), before);
    assert!(engine.redo().unwrap());
    assert_eq!(
        inspect(&mut engine, composition)
            .instances
            .iter()
            .map(|node| node.reference)
            .collect::<Vec<_>>(),
        instances
    );
}

#[test]
fn an_unprepared_composition_is_unavailable_even_with_a_known_canvas() {
    use crate::media::resolution::{ChannelAvailability, SourceObservation, resolve};
    let definition = ChannelDefinition::for_media(MediaKind::Composition);
    let resolution = resolve(&definition, SourceObservation::Unavailable).unwrap();
    assert_eq!(resolution.extent, Some(crate::media::resolution::default_canvas()));
    assert_eq!(resolution.availability, ChannelAvailability::Unavailable);
}

#[test]
fn resizing_the_canvas_preserves_instances_and_new_centered_positions_survive_reload() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let composition = create(&mut engine, MediaKind::Composition);
    let mut channel = crate::media::channel(&engine, composition).unwrap().definition;
    channel.source = ChannelSource::Composition {
        canvas: golden_video::Extent2D::new(640, 480).unwrap(),
    };
    apply(
        &mut engine,
        MediaEdit::ConfigureChannel {
            reference: composition,
            definition: channel.clone(),
        },
    );
    add(&mut engine, composition, vec![source]);
    assert_eq!(
        inspect(&mut engine, composition).instances[0]
            .definition
            .transform
            .position,
        [320.0, 240.0]
    );
    channel.source = ChannelSource::Composition {
        canvas: golden_video::Extent2D::new(1280, 720).unwrap(),
    };
    apply(
        &mut engine,
        MediaEdit::ConfigureChannel {
            reference: composition,
            definition: channel,
        },
    );
    add(&mut engine, composition, vec![source]);
    let saved = inspect(&mut engine, composition);
    assert_eq!(saved.instances[0].definition.transform.position, [320.0, 240.0]);
    assert_eq!(saved.instances[1].definition.transform.position, [640.0, 360.0]);
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(inspect(&mut loaded, composition), saved);
}
