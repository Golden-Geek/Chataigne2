use super::*;
use golden_core::{
    application::apply_project_intent,
    edit::Edit,
    parameter::{ParamValue, ParameterEventBehaviour},
    ui_sync::UiEditIntent,
};

fn selected(engine: &mut AppEngine, owner: ChannelReference, instance: InstanceReference) -> CompositionEditorPage {
    let MediaEditResult::CompositionEditor { page } = apply(
        engine,
        MediaEdit::InspectCompositionEditor {
            query: CompositionEditorQuery {
                selected: Some(instance.instance.0),
                document: None,
                locked: None,
                retained: None,
                instance: None,
                start: 0,
                count: 1,
            },
        },
    ) else {
        panic!()
    };
    assert_eq!(page.selected.as_ref().unwrap().reference, instance);
    assert_eq!(page.composition.as_ref().unwrap().reference, owner);
    page
}

#[test]
fn zero_scale_items_remain_inspectable_copyable_and_roundtrip_with_exact_history() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let owner = create(&mut engine, MediaKind::Composition);
    let instance = add(&mut engine, owner, vec![source])[0];
    for scale in [[0.0, 1.0], [1.0, 0.0], [0.0, 0.0], [-0.0, -2.0], [-1.0, 1.0]] {
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        let mut definition = inspect(&mut engine, owner).instances[0].definition.clone();
        definition.transform.scale = scale;
        edit(
            &mut engine,
            owner,
            CompositionEdit::ConfigureInstance {
                instance: instance.instance,
                definition,
            },
        );
        assert_eq!(engine.undo_len(), history + 1);
        let page = selected(&mut engine, owner, instance);
        assert_eq!(page.selected.unwrap().definition.transform.scale, scale);
        assert!(
            page.parameters
                .iter()
                .any(|binding| binding.instance == instance.instance)
        );
        let saved = to_sparse_project_json_pretty(&engine).unwrap();
        let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
        loaded.apply_edits().unwrap();
        crate::media::validate_project(&loaded).unwrap();
        assert_eq!(inspect(&mut loaded, owner), inspect(&mut engine, owner));
        assert!(engine.undo().unwrap());
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        assert!(engine.redo().unwrap());
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
        let MediaEditResult::InstancesCreated { instances } = edit(
            &mut engine,
            owner,
            CompositionEdit::DuplicateInstances {
                instances: vec![instance.instance],
            },
        ) else {
            panic!()
        };
        assert_eq!(
            selected(&mut engine, owner, instances[0])
                .selected
                .unwrap()
                .definition
                .transform
                .scale,
            scale
        );
        assert!(engine.undo().unwrap());
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    }
}

#[test]
fn inspector_and_low_level_parameter_writes_allow_zero_and_recovery() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let owner = create(&mut engine, MediaKind::Composition);
    let instance = add(&mut engine, owner, vec![source])[0];
    let document = to_sparse_project_json_pretty(&engine).unwrap();
    for scale in [[0.0, 1.0], [1.0, 0.0], [0.0, 0.0], [-1.0, 2.0]] {
        // Raw engine edits do not own a UI history transaction. Keep caller fixtures independent.
        let mut engine = from_sparse_project_json::<AppNode>(&document).unwrap();
        engine.apply_edits().unwrap();
        let binding = selected(&mut engine, owner, instance).parameters.remove(0);
        let parameter = engine.node_id_by_uuid(binding.scale).unwrap();
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::SetParam {
                node: parameter,
                value: ParamValue::Vec2(scale[0], scale[1]),
                behaviour: ParameterEventBehaviour::Coalesce,
            },
        );
        assert!(ack.success, "{ack:?}");
        assert_eq!(
            selected(&mut engine, owner, instance)
                .selected
                .unwrap()
                .definition
                .transform
                .scale,
            scale.map(|value| value as f32)
        );
        assert!(engine.undo().unwrap());
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        engine.edits.push(Edit::SetParam {
            node: parameter,
            value: ParamValue::Vec2(scale[0], scale[1]),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        engine.apply_edits().unwrap();
        assert_eq!(
            inspect(&mut engine, owner).instances[0].definition.transform.scale,
            scale.map(|value| value as f32)
        );
        crate::media::validate_project(&engine).unwrap();
    }
}
