use super::*;
use std::collections::HashSet;

fn group(engine: &mut AppEngine, media: MediaId, parent: Option<GroupId>) -> (MatrixItemId, GroupId) {
    let MediaEditResult::MatrixCreated { items, .. } =
        edit(engine, media, MatrixEdit::AddGroup { parent, label: None })
    else {
        panic!()
    };
    let MatrixItemId::Group { id } = items[0] else { panic!() };
    (items[0], id)
}

fn channel(engine: &mut AppEngine, media: MediaId, item: MatrixItemId) -> ChannelReference {
    inspect(engine, media)
        .items
        .iter()
        .find(|row| row.id == item)
        .unwrap()
        .channel
}

fn draw_order(engine: &AppEngine, reference: ChannelReference, output: MatrixOutput) -> Vec<ChannelReference> {
    crate::matrix::capture::capture(
        engine,
        reference,
        &output,
        &mut 4096,
        &mut HashSet::new(),
        &mut Vec::new(),
        Some(super::cell_playback::time()),
    )
    .unwrap()
    .unwrap()
    .inputs
    .iter()
    .map(|input| input.source)
    .collect()
}

fn launch(engine: &mut AppEngine, media: MediaId, column: ColumnId) {
    edit(
        engine,
        media,
        MatrixEdit::LaunchColumn {
            column,
            mode: LaunchMode::Cut,
        },
    );
}

fn final_output() -> MatrixOutput {
    MatrixOutput::Final {
        definition: MatrixDefinition::default(),
    }
}

#[test]
fn first_authored_layer_is_frontmost_after_root_group_reorder_history_and_reload() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let rows = layers(&mut engine, matrix.media, None, 3);
    let (group, group_id) = group(&mut engine, matrix.media, None);
    let children = layers(&mut engine, matrix.media, Some(group_id), 2);
    let column = columns(&mut engine, matrix.media, 1)[0];
    let mut assignments = Vec::new();
    for row in rows.iter().chain(&children) {
        let source = create(&mut engine, MediaKind::SolidColor);
        assignments.push(CellAssignment {
            layer: layer(*row),
            column,
            source: Some(source),
        });
    }
    edit(&mut engine, matrix.media, MatrixEdit::SetCells { assignments });
    launch(&mut engine, matrix.media, column);
    let [a, b, c] = rows
        .iter()
        .map(|row| channel(&mut engine, matrix.media, *row))
        .collect::<Vec<_>>()[..]
    else {
        panic!()
    };
    let group_channel = channel(&mut engine, matrix.media, group);
    let d = channel(&mut engine, matrix.media, children[0]);
    let e = channel(&mut engine, matrix.media, children[1]);
    assert_eq!(
        draw_order(&engine, matrix, final_output()),
        vec![group_channel, c, b, a]
    );
    assert_eq!(
        draw_order(&engine, group_channel, MatrixOutput::Group { group: group_id }),
        vec![e, d]
    );

    for (intent, expected) in [
        (
            MatrixEdit::MoveItem {
                item: rows[0],
                parent: None,
                after: Some(rows[1]),
            },
            vec![group_channel, c, a, b],
        ),
        (
            MatrixEdit::DropItem {
                item: group,
                target: MatrixItemDropTarget::Before { item: rows[1] },
            },
            vec![c, a, b, group_channel],
        ),
    ] {
        let before = inspect(&mut engine, matrix.media);
        let previous = draw_order(&engine, matrix, final_output());
        let history = engine.undo_len();
        edit(&mut engine, matrix.media, intent);
        let after = inspect(&mut engine, matrix.media);
        assert_eq!(engine.undo_len(), history + 1);
        assert_eq!(draw_order(&engine, matrix, final_output()), expected);
        assert!(engine.undo().unwrap());
        assert_eq!(inspect(&mut engine, matrix.media), before);
        assert_eq!(draw_order(&engine, matrix, final_output()), previous);
        assert!(engine.redo().unwrap());
        assert_eq!(inspect(&mut engine, matrix.media), after);
        assert_eq!(draw_order(&engine, matrix, final_output()), expected);
    }
    let authored = inspect(&mut engine, matrix.media);
    assert_eq!(
        authored.items.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![group, children[0], children[1], rows[1], rows[0], rows[2]]
    );
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(inspect(&mut loaded, matrix.media), authored);
    launch(&mut loaded, matrix.media, column);
    assert_eq!(
        draw_order(&loaded, matrix, final_output()),
        vec![c, a, b, group_channel]
    );
    assert_eq!(
        draw_order(&loaded, group_channel, MatrixOutput::Group { group: group_id }),
        vec![e, d]
    );
}

#[test]
fn front_first_order_is_local_to_each_nested_group_and_survives_reparenting() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let (outer, outer_id) = group(&mut engine, matrix.media, None);
    let outer_rows = layers(&mut engine, matrix.media, Some(outer_id), 2);
    let (inner, inner_id) = group(&mut engine, matrix.media, Some(outer_id));
    let inner_rows = layers(&mut engine, matrix.media, Some(inner_id), 2);
    let column = columns(&mut engine, matrix.media, 1)[0];
    let source = create(&mut engine, MediaKind::SolidColor);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: outer_rows
                .iter()
                .chain(&inner_rows)
                .map(|row| CellAssignment {
                    layer: layer(*row),
                    column,
                    source: Some(source),
                })
                .collect(),
        },
    );
    launch(&mut engine, matrix.media, column);
    let outer_channel = channel(&mut engine, matrix.media, outer);
    let inner_channel = channel(&mut engine, matrix.media, inner);
    let a = channel(&mut engine, matrix.media, outer_rows[0]);
    let b = channel(&mut engine, matrix.media, outer_rows[1]);
    let c = channel(&mut engine, matrix.media, inner_rows[0]);
    let d = channel(&mut engine, matrix.media, inner_rows[1]);
    assert_eq!(draw_order(&engine, matrix, final_output()), vec![outer_channel]);
    assert_eq!(
        draw_order(&engine, outer_channel, MatrixOutput::Group { group: outer_id }),
        vec![inner_channel, b, a]
    );
    assert_eq!(
        draw_order(&engine, inner_channel, MatrixOutput::Group { group: inner_id }),
        vec![d, c]
    );
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::MoveItem {
            item: outer_rows[0],
            parent: Some(inner_id),
            after: None,
        },
    );
    assert_eq!(
        draw_order(&engine, outer_channel, MatrixOutput::Group { group: outer_id }),
        vec![inner_channel, b]
    );
    assert_eq!(
        draw_order(&engine, inner_channel, MatrixOutput::Group { group: inner_id }),
        vec![d, c, a]
    );
}
