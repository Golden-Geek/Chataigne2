use super::*;
use crate::app::AppNode;
use golden_core::{
    edit::{Edit, NodeTree},
    node::Node,
};

fn read(
    engine: &mut AppEngine,
    reference: SurfaceReference,
    cursor: Option<golden_core::node::NodeUuid>,
    include_candidates: bool,
) -> ScreenHandlePage {
    let document = engine.nodes.get(engine.root).unwrap().node_data().meta.uuid;
    let ScreenEditResult::ScreenHandles { page } = apply(
        engine,
        ScreenEdit::InspectScreenHandles {
            query: ScreenHandleQuery {
                document,
                reference,
                cursor,
                include_candidates,
            },
        },
    ) else {
        panic!()
    };
    page
}

#[test]
fn handle_queries_project_real_groups_and_backend_eligibility_without_authoring_edits() {
    let mut engine = fixture();
    let (_, a) = create(&mut engine);
    let b = super::handle_links::peer(&mut engine, a.screen);
    let c = super::handle_links::peer(&mut engine, a.screen);
    let (_, foreign) = create(&mut engine);
    apply(
        &mut engine,
        ScreenEdit::LinkScreenCorners {
            source: ScreenCorner {
                reference: a,
                corner: 0,
            },
            target: ScreenCorner {
                reference: b,
                corner: 1,
            },
        },
    );
    let before = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.ui_history_state();
    let page = read(&mut engine, a, None, true);
    assert!(page.document_matches);
    assert_eq!(page.groups.len(), 1);
    assert_eq!(page.groups[0].corner, 0);
    assert_eq!(page.groups[0].members.len(), 2);
    assert!(!page.groups[0].missing);
    assert_eq!(page.candidates.len(), 8);
    assert!(
        page.candidates
            .iter()
            .all(|candidate| candidate.member.reference.screen == a.screen
                && candidate.member.reference.surface != a.surface)
    );
    assert!(
        page.candidates
            .iter()
            .filter(|candidate| candidate.member.reference == b)
            .all(|candidate| !candidate.linkable[0])
    );
    assert!(
        page.candidates
            .iter()
            .filter(|candidate| candidate.member.reference == c)
            .all(|candidate| candidate.linkable[0])
    );
    let off = read(&mut engine, a, None, false);
    assert_eq!(off.groups.len(), 1);
    assert!(off.candidates.is_empty() && off.next.is_none());
    let ScreenEditResult::Editor { page: editor } = apply(
        &mut engine,
        ScreenEdit::InspectEditor {
            query: ScreenEditorQuery {
                editor: ScreenEditorKind::Screen,
                selected: Some(a.surface.0),
                document: None,
                locked: None,
                retained: None,
                surface: None,
            },
        },
    ) else {
        panic!("expected a Screen editor page");
    };
    assert_eq!(editor.linked_corners.len(), 1);
    assert_eq!(editor.linked_corners[0].corner, off.groups[0].corner);
    assert_eq!(editor.linked_corners[0].members, off.groups[0].members);
    assert_eq!(engine.ui_history_state(), history);
    assert_eq!(
        golden_core::app::to_sparse_project_json_pretty(&engine).unwrap(),
        before
    );
    assert!(
        page.candidates
            .iter()
            .all(|candidate| candidate.member.reference != foreign)
    );
    apply(&mut engine, ScreenEdit::RemoveSurface { reference: b });
    let broken = read(&mut engine, a, None, true);
    assert!(broken.groups[0].missing);
    assert_eq!(broken.groups[0].members.len(), 1);
    assert!(broken.candidates.iter().all(|candidate| !candidate.linkable[0]));
}

#[test]
fn bounded_handle_pages_reach_every_peer_and_reject_foreign_or_old_document_cursors() {
    let mut engine = fixture();
    let (_, selected) = create(&mut engine);
    let manager = super::super::authoring::manager(&engine).unwrap();
    let parent = engine.node_id_by_uuid(selected.screen.0).unwrap();
    let mut items = Vec::new();
    for index in 0..135 {
        items.push(golden_core::edit::UserItemTreeInsertion::new(
            parent,
            NodeTree::new(super::super::SurfaceNode::from_definition(
                format!("Peer {index}"),
                SurfaceDefinition::default(),
            ))
            .as_user_item(),
        ));
    }
    engine.edits.push(Edit::AddUserItemTrees { items });
    engine.settle_edit_notifications(16).unwrap();
    let history = engine.ui_history_state();
    let mut cursor = None;
    let mut found = std::collections::HashSet::new();
    loop {
        let page = read(&mut engine, selected, cursor, true);
        assert!(page.candidates.len() <= 64);
        for candidate in page.candidates {
            assert!(found.insert((candidate.member.reference.surface, candidate.member.corner)));
        }
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(found.len(), (super::super::editor::MAX_EDITOR_SURFACES - 1) * 4);
    assert_eq!(engine.ui_history_state(), history);
    let document = engine.nodes.get(engine.root).unwrap().node_data().meta.uuid;
    assert!(
        !invoke(
            &mut engine,
            ScreenEdit::InspectScreenHandles {
                query: ScreenHandleQuery {
                    document,
                    reference: selected,
                    cursor: Some(document),
                    include_candidates: true
                }
            }
        )
        .success
    );
    let ScreenEditResult::ScreenHandles { page } = apply(
        &mut engine,
        ScreenEdit::InspectScreenHandles {
            query: ScreenHandleQuery {
                document: golden_core::node::NodeUuid(uuid::Uuid::new_v4()),
                reference: selected,
                cursor: None,
                include_candidates: true,
            },
        },
    ) else {
        panic!()
    };
    assert!(!page.document_matches);
    assert!(page.candidates.is_empty() && page.groups.is_empty());
    assert!(matches!(engine.nodes.get(manager), Some(AppNode::ScreenManager(_))));
}
