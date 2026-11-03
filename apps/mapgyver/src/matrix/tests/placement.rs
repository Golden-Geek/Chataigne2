use super::*;

#[test]
fn relative_drop_uses_complete_sibling_order_preserves_subtrees_and_roundtrips_exact_history() {
    let mut engine = fixture();
    let MediaEditResult::Created { reference } = apply(
        &mut engine,
        MediaEdit::CreateMedia {
            media_kind: MediaKind::Matrix,
            label: None,
        },
    ) else {
        panic!()
    };
    let media = reference.media;
    let initial = inspect(&mut engine, media);
    let [a, b, c] = initial.items.iter().map(|row| row.id).collect::<Vec<_>>()[..] else {
        panic!()
    };
    let MediaEditResult::MatrixCreated { items, .. } = edit(
        &mut engine,
        media,
        MatrixEdit::AddGroup {
            parent: None,
            label: Some("Group".into()),
        },
    ) else {
        panic!()
    };
    let group = items[0];
    let MatrixItemId::Group { id: group_id } = group else {
        panic!()
    };
    for intent in [
        MatrixEdit::DropItem {
            item: c,
            target: MatrixItemDropTarget::Before { item: b },
        },
        MatrixEdit::DropItem {
            item: a,
            target: MatrixItemDropTarget::Inside { group: group_id },
        },
        MatrixEdit::DropItem {
            item: b,
            target: MatrixItemDropTarget::Inside { group: group_id },
        },
        MatrixEdit::DropItem {
            item: b,
            target: MatrixItemDropTarget::Before { item: a },
        },
        MatrixEdit::DropItem {
            item: a,
            target: MatrixItemDropTarget::After { item: b },
        },
        MatrixEdit::DropItem {
            item: group,
            target: MatrixItemDropTarget::Before { item: c },
        },
        MatrixEdit::DropItem {
            item: a,
            target: MatrixItemDropTarget::Root,
        },
        MatrixEdit::DropColumn {
            column: initial.columns[2].id,
            target: initial.columns[1].id,
            before: true,
        },
        MatrixEdit::DropColumn {
            column: initial.columns[0].id,
            target: initial.columns[2].id,
            before: false,
        },
    ] {
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        edit(&mut engine, media, intent);
        let after = to_sparse_project_json_pretty(&engine).unwrap();
        assert_eq!(engine.undo_len(), history + usize::from(before != after));
        if before != after {
            assert!(engine.undo().unwrap());
            assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
            assert!(engine.redo().unwrap());
            assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), after);
        }
        crate::media::validate_project(&engine).unwrap();
        let mut loaded = from_sparse_project_json::<AppNode>(&after).unwrap();
        assert_eq!(inspect(&mut loaded, media), inspect(&mut engine, media));
    }
    let final_state = inspect(&mut engine, media);
    assert_eq!(
        final_state.items.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![group, b, c, a]
    );
    assert_eq!(final_state.items[1].parent, Some(group_id));
    assert_eq!(final_state.items[3].parent, None);
    assert_eq!(
        final_state.columns.iter().map(|column| column.id).collect::<Vec<_>>(),
        vec![initial.columns[2].id, initial.columns[0].id, initial.columns[1].id]
    );
    for intent in [
        MatrixEdit::DropItem {
            item: group,
            target: MatrixItemDropTarget::Inside { group: group_id },
        },
        MatrixEdit::DropItem {
            item: group,
            target: MatrixItemDropTarget::After { item: b },
        },
        MatrixEdit::DropItem {
            item: a,
            target: MatrixItemDropTarget::Before { item: a },
        },
        MatrixEdit::DropColumn {
            column: initial.columns[0].id,
            target: initial.columns[0].id,
            before: true,
        },
    ] {
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        assert!(!invoke(&mut engine, MediaEdit::Matrix { media, edit: intent }).success);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
}
