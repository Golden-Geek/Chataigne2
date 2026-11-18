use super::{row_geometry::group, *};

fn page(engine: &mut AppEngine, media: MediaId, row_start: u32) -> MatrixViewportSnapshot {
    let MediaEditResult::MatrixViewport { snapshot } = edit(
        engine,
        media,
        MatrixEdit::InspectViewport {
            viewport: MatrixViewport {
                row_layout: None,
                row_start,
                row_count: 1,
                axis_start: 0,
                axis_count: 1,
            },
            selection: None,
        },
    ) else {
        panic!()
    };
    snapshot
}

#[test]
fn offscreen_ancestors_have_complete_spans_without_transferring_their_descendants() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let outer = group(&mut engine, media, None);
    let inner = group(&mut engine, media, Some(outer));
    layers(&mut engine, media, Some(inner), 100);
    group(&mut engine, media, None);
    let value = page(&mut engine, media, 50);
    assert_eq!(value.rows.len(), 1);
    assert_eq!(value.row_groups.len(), 2);
    let a = value.row_groups[0];
    let b = value.row_groups[1];
    assert_eq!(
        (a.id, a.depth, a.start, a.end, a.start_groups, a.end_groups),
        (outer, 0, 0, 102, 0, 2)
    );
    assert_eq!(
        (b.id, b.depth, b.start, b.end, b.start_groups, b.end_groups),
        (inner, 1, 1, 102, 1, 2)
    );
    assert!(value.rows[0].visible_index.unwrap() > b.start);
}

#[test]
fn collapse_reparent_and_reorder_update_only_the_current_group_intervals_and_history_restores_them() {
    use golden_core::node::Node;
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let outer = group(&mut engine, media, None);
    let inner = group(&mut engine, media, Some(outer));
    layers(&mut engine, media, Some(inner), 1);
    let other = group(&mut engine, media, None);
    edit(
        &mut engine,
        media,
        MatrixEdit::MoveItem {
            item: MatrixItemId::Group { id: inner },
            parent: Some(other),
            after: None,
        },
    );
    let moved = page(&mut engine, media, 3);
    assert_eq!(
        moved.row_groups.iter().map(|span| span.id).collect::<Vec<_>>(),
        [other, inner]
    );
    assert_eq!((moved.row_groups[0].start, moved.row_groups[0].end), (1, 4));
    assert!(engine.undo().unwrap());
    assert_eq!(page(&mut engine, media, 2).row_groups[0].id, outer);
    edit(
        &mut engine,
        media,
        MatrixEdit::MoveItem {
            item: MatrixItemId::Group { id: other },
            parent: None,
            after: None,
        },
    );
    let reordered = page(&mut engine, media, 3);
    assert_eq!((reordered.row_groups[0].id, reordered.row_groups[0].start), (outer, 1));
    let id = engine.node_id_by_uuid(outer.0).unwrap();
    let mut presentation = engine.nodes.get(id).unwrap().node_data().meta.presentation.clone();
    presentation.collapsed = true;
    assert!(
        apply_project_intent(
            &mut engine,
            UiEditIntent::PatchMeta {
                node: id,
                patch: golden_core::node::NodeMetaPatch {
                    presentation: Some(presentation),
                    ..Default::default()
                }
                .into()
            }
        )
        .success
    );
    let closed = page(&mut engine, media, 1);
    assert_eq!(closed.row_groups.len(), 1);
    assert_eq!(
        (
            closed.row_groups[0].start,
            closed.row_groups[0].end,
            closed.row_groups[0].start_groups,
            closed.row_groups[0].end_groups
        ),
        (1, 2, 1, 2)
    );
}
