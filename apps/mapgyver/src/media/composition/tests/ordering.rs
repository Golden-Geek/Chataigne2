use super::*;

#[test]
fn composition_front_and_back_preserve_authored_content_with_exact_history_copy_and_reload() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let owner = create(&mut engine, MediaKind::Composition);
    let items = add(&mut engine, owner, vec![source; 3]);
    let original = inspect(&mut engine, owner);
    for (action, target, expected) in [
        (
            InstanceArrangeAction::Front,
            items[2],
            vec![items[2], items[0], items[1]],
        ),
        (
            InstanceArrangeAction::Back,
            items[0],
            vec![items[1], items[2], items[0]],
        ),
    ] {
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        edit(
            &mut engine,
            owner,
            CompositionEdit::ArrangeInstance {
                instance: target.instance,
                action,
            },
        );
        let actual = inspect(&mut engine, owner);
        assert_eq!(
            actual.instances.iter().map(|item| item.reference).collect::<Vec<_>>(),
            expected
        );
        for item in &actual.instances {
            assert_eq!(
                item,
                original
                    .instances
                    .iter()
                    .find(|old| old.reference == item.reference)
                    .unwrap()
            );
        }
        assert_eq!(engine.undo_len(), history + 1);
        let saved = to_sparse_project_json_pretty(&engine).unwrap();
        edit(
            &mut engine,
            owner,
            CompositionEdit::ArrangeInstance {
                instance: target.instance,
                action,
            },
        );
        assert_eq!(engine.undo_len(), history + 1, "already at requested end is a no-op");
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
        assert!(engine.undo().unwrap());
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        assert!(engine.redo().unwrap());
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
        let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
        loaded.apply_edits().unwrap();
        assert_eq!(inspect(&mut loaded, owner), actual);
        let MediaEditResult::InstancesCreated { instances } = edit(
            &mut loaded,
            owner,
            CompositionEdit::DuplicateInstances {
                instances: vec![target.instance],
            },
        ) else {
            panic!()
        };
        assert_eq!(
            inspect(&mut loaded, owner).instances.last().unwrap().reference,
            instances[0]
        );
        assert!(engine.undo().unwrap());
    }
}

#[test]
fn composition_arrangement_rejects_foreign_children_without_mutating_either_owner() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let owner = create(&mut engine, MediaKind::Composition);
    let peer = create(&mut engine, MediaKind::Composition);
    let foreign = add(&mut engine, peer, vec![source])[0];
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for action in [InstanceArrangeAction::Front, InstanceArrangeAction::Back] {
        let ack = invoke(
            &mut engine,
            MediaEdit::Composition {
                reference: owner,
                edit: CompositionEdit::ArrangeInstance {
                    instance: foreign.instance,
                    action,
                },
            },
        );
        assert!(!ack.success);
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
}
