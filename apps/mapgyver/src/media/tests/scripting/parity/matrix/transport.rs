use super::*;

#[test]
fn layer_navigation_matches_named_and_generic_script_and_transport_callers() {
    let scene = scene();
    let layer = scene.snapshot.cells[0].layer;
    let anchor = scene.snapshot.columns[0].id;
    let target = scene.snapshot.columns[1].id;
    for caller in CALLERS {
        let mut engine = loaded(&scene.document);
        matrix_edit(
            &mut engine,
            scene.root,
            MatrixEdit::LaunchColumn {
                column: anchor,
                mode: LaunchMode::Cut,
            },
        );
        service_script_app_operations(&mut engine).unwrap();
        let value = run(
            &mut engine,
            caller,
            MediaEdit::Matrix {
                media: scene.root.media,
                edit: MatrixEdit::NavigateCell {
                    layer,
                    direction: MatrixNavigationDirection::Next,
                },
            },
        )
        .unwrap();
        let MediaEditResult::MatrixControlInvoked { owner, parameter } = serde_json::from_value(value).unwrap() else {
            panic!("{caller:?} did not invoke the Layer trigger")
        };
        assert_eq!(owner, layer.0);
        assert!(engine.node_id_by_uuid(parameter).is_some());
        let MediaEditResult::MatrixPlayback { snapshot } =
            crate::matrix::apply(&mut engine, scene.root.media, MatrixEdit::InspectPlayback).unwrap()
        else {
            panic!("playback snapshot")
        };
        assert_eq!(snapshot.desired_column, Some(anchor));
        assert!(
            snapshot
                .layers
                .iter()
                .any(|request| request.layer == layer && request.column == Some(target))
        );
    }
}

#[test]
fn matrix_navigation_has_the_same_playback_and_callback_for_every_caller() {
    use MatrixNavigationDirection::{Next, Previous};

    let scene = scene();
    let mut authored = loaded(&scene.document);
    let mut groups = Vec::new();
    for _ in 0..2 {
        let MediaEditResult::MatrixColumnGroupCreated { group } = matrix_edit(
            &mut authored,
            scene.root,
            MatrixEdit::AddColumnGroup {
                parent: None,
                label: None,
            },
        ) else {
            panic!("top-level Column Group")
        };
        let MediaEditResult::MatrixCreated { columns, .. } = matrix_edit(
            &mut authored,
            scene.root,
            MatrixEdit::AddColumns {
                parent: Some(group),
                count: 1,
            },
        ) else {
            panic!("grouped Column")
        };
        groups.push(columns[0]);
    }
    let document = to_sparse_project_json_pretty(&authored).unwrap();
    let root_column = scene.snapshot.columns[1].id;
    let cases = [
        (MatrixEdit::NavigateColumn { direction: Next }, root_column, groups[0]),
        (
            MatrixEdit::NavigateColumn { direction: Previous },
            groups[0],
            root_column,
        ),
        (
            MatrixEdit::NavigateColumnGroup { direction: Next },
            groups[0],
            groups[1],
        ),
        (
            MatrixEdit::NavigateColumnGroup { direction: Previous },
            groups[1],
            groups[0],
        ),
    ];
    for (edit, anchor, target) in cases {
        for caller in CALLERS {
            let mut engine = loaded(&document);
            matrix_edit(
                &mut engine,
                scene.root,
                MatrixEdit::LaunchColumn {
                    column: anchor,
                    mode: LaunchMode::Cut,
                },
            );
            service_script_app_operations(&mut engine).unwrap();
            let value = run(
                &mut engine,
                caller,
                MediaEdit::Matrix {
                    media: scene.root.media,
                    edit: edit.clone(),
                },
            )
            .unwrap();
            let MediaEditResult::MatrixControlInvoked { owner, parameter } = serde_json::from_value(value).unwrap()
            else {
                panic!("{caller:?} did not invoke the Matrix trigger")
            };
            assert_eq!(owner, scene.root.media.0);
            assert!(engine.node_id_by_uuid(parameter).is_some());
            let MediaEditResult::MatrixPlayback { snapshot } =
                crate::matrix::apply(&mut engine, scene.root.media, MatrixEdit::InspectPlayback).unwrap()
            else {
                panic!("playback snapshot")
            };
            assert_eq!(snapshot.desired_column, Some(target), "{caller:?}");
            assert_eq!(snapshot.transition.duration_ms, 250);
        }
    }
}
