use super::*;
use crate::media::composition::protocol::*;

struct Scene {
    document: String,
    owner: ChannelReference,
    foreign: ChannelReference,
    sources: [ChannelReference; 2],
    instances: Vec<InstanceReference>,
    definition: InstanceDefinition,
}

fn scene() -> Scene {
    let mut engine = fixture();
    let sources = [
        create(&mut engine, MediaKind::SolidColor),
        create(&mut engine, MediaKind::Picture),
    ];
    let owner = create(&mut engine, MediaKind::Composition);
    let foreign = create(&mut engine, MediaKind::Composition);
    let MediaEditResult::InstancesCreated { instances } = apply(
        &mut engine,
        MediaEdit::Composition {
            reference: owner,
            edit: CompositionEdit::AddInstances {
                sources: vec![sources[0]; 2],
            },
        },
    ) else {
        panic!()
    };
    let definition = InstanceDefinition {
        source: Some(ChannelReference {
            media: MediaId(instances[0].instance.0),
            channel: sources[0].channel,
        }),
        transform: Transform2D {
            position: [100.0, 80.0],
            scale: [-2.0, 3.0],
            rotation: 0.0,
            pivot: [0.2, 0.8],
        },
        opacity: 0.4,
        blend: BlendMode::Multiply,
        enabled: true,
    };
    apply(
        &mut engine,
        MediaEdit::Composition {
            reference: owner,
            edit: CompositionEdit::ConfigureInstance {
                instance: instances[0].instance,
                definition: definition.clone(),
            },
        },
    );
    Scene {
        document: to_sparse_project_json_pretty(&engine).unwrap(),
        owner,
        foreign,
        sources,
        instances,
        definition,
    }
}

#[test]
fn composition_authoring_geometry_inspection_and_rejection_match_every_public_caller() {
    let scene = scene();
    let first = scene.instances[0].instance;
    let mut changed = scene.definition.clone();
    changed.source = Some(scene.sources[1]);
    changed.opacity = 0.7;
    changed.blend = BlendMode::Screen;
    let mut cycle = scene.definition.clone();
    cycle.source = Some(scene.owner);
    let mut invalid = scene.definition.clone();
    invalid.transform.scale[0] = 0.0;
    let mut cases = vec![
        (scene.owner, CompositionEdit::Inspect),
        (
            scene.owner,
            CompositionEdit::DropSource {
                copy: false,
                source: scene.sources[1].media.0,
                instance: Some(first),
                position: Point { x: 800., y: 600. },
            },
        ),
        (
            scene.owner,
            CompositionEdit::DropSource {
                copy: false,
                source: scene.owner.channel.0,
                instance: Some(first),
                position: Point { x: 0., y: 0. },
            },
        ),
        (
            scene.owner,
            CompositionEdit::ConfigureInstance {
                instance: first,
                definition: changed,
            },
        ),
        (
            scene.owner,
            CompositionEdit::RenameInstance {
                instance: first,
                label: "Changed instance".into(),
            },
        ),
        (
            scene.owner,
            CompositionEdit::ReorderInstance {
                instance: first,
                after: Some(scene.instances[1].instance),
            },
        ),
        (scene.owner, CompositionEdit::RemoveInstance { instance: first }),
        (
            scene.owner,
            CompositionEdit::ConfigureInstance {
                instance: first,
                definition: cycle,
            },
        ),
        (
            scene.owner,
            CompositionEdit::ConfigureInstance {
                instance: first,
                definition: invalid,
            },
        ),
        (scene.foreign, CompositionEdit::RemoveInstance { instance: first }),
        (
            scene.owner,
            CompositionEdit::AddInstances {
                sources: vec![scene.owner],
            },
        ),
    ];
    for mode in [
        InstanceGeometryMode::Translate,
        InstanceGeometryMode::Rotate,
        InstanceGeometryMode::Scale,
    ] {
        cases.push((
            scene.owner,
            CompositionEdit::EditInstanceGeometry {
                instance: first,
                edit: InstanceGeometryEdit {
                    mode,
                    origin: scene.definition.transform,
                    start: Point { x: 110.0, y: 90.0 },
                    point: Point { x: 130.0, y: 100.0 },
                },
            },
        ));
    }
    for (reference, edit) in cases {
        let operation = MediaEdit::Composition { reference, edit };
        let mut canonical = None;
        for caller in CALLERS {
            let mut engine = loaded(&scene.document);
            let reply = run(&mut engine, caller, operation.clone());
            let actual = (reply, authored(&engine));
            if let Some(expected) = &canonical {
                assert_eq!(&actual, expected, "{caller:?}: {operation:?}");
            } else {
                canonical = Some(actual);
            }
        }
    }
}

#[test]
fn composition_creation_add_and_copy_return_committed_owned_identities_to_every_caller() {
    for caller in CALLERS {
        let mut engine = fixture();
        let reply = run(
            &mut engine,
            caller,
            MediaEdit::CreateMedia {
                media_kind: MediaKind::Composition,
                label: None,
            },
        )
        .unwrap();
        let MediaEditResult::Created { reference } = serde_json::from_value(reply).unwrap() else {
            panic!()
        };
        assert_eq!(authored(&engine).1[0].reference, reference);
        assert!(authored(&engine).1[0].instances.is_empty());
        let scene = scene();
        for edit in [
            CompositionEdit::DropSource {
                copy: true,
                source: scene.sources[0].media.0,
                instance: None,
                position: Point { x: 10., y: 20. },
            },
            CompositionEdit::DropSource {
                copy: false,
                source: scene.sources[1].media.0,
                instance: None,
                position: Point { x: 320., y: 180. },
            },
            CompositionEdit::AddInstances {
                sources: scene.sources.to_vec(),
            },
            CompositionEdit::DuplicateInstances {
                instances: vec![scene.instances[0].instance],
            },
        ] {
            let mut engine = loaded(&scene.document);
            let reply = run(
                &mut engine,
                caller,
                MediaEdit::Composition {
                    reference: scene.owner,
                    edit,
                },
            )
            .unwrap();
            let MediaEditResult::InstancesCreated { instances } = serde_json::from_value(reply).unwrap() else {
                panic!()
            };
            let (_, compositions, _, _) = authored(&engine);
            let current = compositions
                .iter()
                .find(|snapshot| snapshot.reference == scene.owner)
                .unwrap();
            assert_eq!(current.instances.len(), 2 + instances.len());
            for reference in instances {
                assert_eq!(reference.composition, scene.owner);
                assert!(!scene.instances.contains(&reference));
                assert!(current.instances.iter().any(|item| item.reference == reference));
            }
        }
    }
}
