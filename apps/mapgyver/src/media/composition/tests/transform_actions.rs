use super::*;
use golden_video::Extent2D;

#[test]
fn composition_canvas_actions_preserve_content_state_and_have_exact_history_and_reload() {
    for (source_size, fit_scale) in [([200, 100], 2.0), ([100, 200], 1.5)] {
        for action in [
            InstanceTransformAction::Reset,
            InstanceTransformAction::Fit,
            InstanceTransformAction::Fill,
        ] {
            let mut engine = fixture();
            let source = create(&mut engine, MediaKind::SolidColor);
            let owner = create(&mut engine, MediaKind::Composition);
            for (reference, size) in [(source, source_size), (owner, [400, 300])] {
                apply(
                    &mut engine,
                    MediaEdit::ConfigureMediaResolution {
                        media: reference.media,
                        size: SizePolicy::Fixed {
                            extent: Extent2D::new(size[0], size[1]).unwrap(),
                        },
                    },
                );
            }
            let item = add(&mut engine, owner, vec![source])[0];
            let mut definition = inspect(&mut engine, owner).instances[0].definition.clone();
            definition.transform = Transform2D {
                position: [17., 29.],
                scale: [-0., -4.],
                rotation_degrees: 35.,
                pivot: [0.2, 0.8],
            };
            definition.opacity = 0.3;
            definition.blend = BlendMode::Multiply;
            definition.enabled = false;
            edit(
                &mut engine,
                owner,
                CompositionEdit::ConfigureInstance {
                    instance: item.instance,
                    definition: definition.clone(),
                },
            );
            let before = to_sparse_project_json_pretty(&engine).unwrap();
            let history = engine.undo_len();
            edit(
                &mut engine,
                owner,
                CompositionEdit::TransformInstance {
                    instance: item.instance,
                    action,
                },
            );
            let actual = inspect(&mut engine, owner).instances[0].definition.clone();
            assert_eq!(actual.transform.position, [200., 150.]);
            assert_eq!(actual.transform.pivot, [0.5; 2]);
            assert_eq!(actual.transform.rotation_degrees, 0.);
            assert_eq!(
                actual.transform.scale,
                match action {
                    InstanceTransformAction::Reset => [1.; 2],
                    InstanceTransformAction::Fit => [fit_scale; 2],
                    InstanceTransformAction::Fill => [400. / source_size[0] as f32, 300. / source_size[1] as f32],
                }
            );
            assert_eq!(
                InstanceDefinition {
                    transform: definition.transform,
                    ..actual.clone()
                },
                definition
            );
            let saved = to_sparse_project_json_pretty(&engine).unwrap();
            assert_eq!(engine.undo_len(), history + 1);
            assert!(engine.undo().unwrap());
            assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
            assert!(engine.redo().unwrap());
            assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
            let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
            loaded.apply_edits().unwrap();
            assert_eq!(inspect(&mut loaded, owner).instances[0].definition, actual);
        }
    }
}

#[test]
fn composition_fit_rejects_unavailable_dimensions_atomically_but_reset_remains_available() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SharedTexture);
    let owner = create(&mut engine, MediaKind::Composition);
    let item = add(&mut engine, owner, vec![source])[0];
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for action in [InstanceTransformAction::Fit, InstanceTransformAction::Fill] {
        let ack = invoke(
            &mut engine,
            MediaEdit::Composition {
                reference: owner,
                edit: CompositionEdit::TransformInstance {
                    instance: item.instance,
                    action,
                },
            },
        );
        assert!(!ack.success, "{ack:?}");
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        assert_eq!(engine.undo_len(), history);
    }
    edit(
        &mut engine,
        owner,
        CompositionEdit::TransformInstance {
            instance: item.instance,
            action: InstanceTransformAction::Reset,
        },
    );
}
