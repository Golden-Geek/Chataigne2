use super::drop_sources::{media, source};
use super::*;
use crate::{
    app::AppNode,
    media::protocol::{MediaEdit, MediaEditResult},
};
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    edit::{Edit, NodeTree},
    node::Node,
};

fn read(
    engine: &mut AppEngine,
    reference: SurfaceReference,
    cursor: Option<golden_core::node::NodeUuid>,
) -> RelatedSurfacePage {
    let document = engine.nodes.get(engine.root).unwrap().node_data().meta.uuid;
    let ScreenEditResult::RelatedSurfaces { page } = apply(
        engine,
        ScreenEdit::InspectRelatedSurfaces {
            query: RelatedSurfaceQuery {
                document,
                reference,
                cursor,
            },
        },
    ) else {
        panic!()
    };
    page
}

fn assign(
    engine: &mut AppEngine,
    reference: SurfaceReference,
    source: Option<crate::media::protocol::ChannelReference>,
) {
    let mut next = definition(engine, reference);
    next.source = source;
    apply(
        engine,
        ScreenEdit::ConfigureSurface {
            reference,
            definition: next,
        },
    );
}

#[test]
fn related_surfaces_share_canonical_channels_across_screens_and_reference_chains_without_history() {
    let mut engine = fixture();
    let (_, selected) = create(&mut engine);
    let (_, peer) = create(&mut engine);
    let (_, alias_peer) = create(&mut engine);
    let (_, different) = create(&mut engine);
    let leaf = source(&mut engine);
    let MediaEditResult::Created { reference: second } = media(
        &mut engine,
        MediaEdit::AddChannel {
            media: leaf.media,
            label: None,
        },
    ) else {
        panic!()
    };
    let mut target = leaf.media;
    for _ in 0..2 {
        let MediaEditResult::ReferenceCreated { media: next } = media(
            &mut engine,
            MediaEdit::CreateReference {
                target: Some(target),
                label: None,
            },
        ) else {
            panic!()
        };
        target = next;
    }
    let alias = crate::media::resolve_source_node(&engine, target.0).unwrap();
    assign(&mut engine, selected, Some(leaf));
    assign(&mut engine, peer, Some(leaf));
    assign(&mut engine, alias_peer, Some(alias));
    assign(&mut engine, different, Some(second));
    let history = engine.ui_history_state();
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let page = read(&mut engine, selected, None);
    assert_eq!(page.source, Some(leaf));
    assert_eq!(
        page.surfaces
            .iter()
            .map(|row| row.surface.reference)
            .collect::<Vec<_>>(),
        vec![peer, alias_peer]
    );
    assert!(
        page.surfaces
            .iter()
            .all(|row| engine.node_id_by_uuid(row.parameters.crop_origin).is_some())
    );
    assert_eq!(engine.ui_history_state(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(read(&mut loaded, selected, None).surfaces.len(), 2);
    assign(&mut engine, peer, Some(second));
    assert_eq!(read(&mut engine, selected, None).surfaces.len(), 1);
    assert!(engine.undo().unwrap());
    assert_eq!(read(&mut engine, selected, None).surfaces.len(), 2);
    media(
        &mut engine,
        MediaEdit::ConfigureReference {
            media: target,
            target: Some(leaf.media),
        },
    );
    media(
        &mut engine,
        MediaEdit::SelectReferenceChannel {
            media: target,
            channel: Some(second.channel),
        },
    );
    assert_eq!(read(&mut engine, selected, None).surfaces.len(), 1);
    assert!(engine.undo().unwrap());
    assert_eq!(read(&mut engine, selected, None).surfaces.len(), 2);
    media(&mut engine, MediaEdit::RemoveMedia { media: leaf.media });
    assert!(read(&mut engine, alias_peer, None).source.is_none());
    assert!(read(&mut engine, alias_peer, None).surfaces.is_empty());
    assert!(engine.undo().unwrap());
    assert_eq!(read(&mut engine, alias_peer, None).surfaces.len(), 2);
}

#[test]
fn unassigned_deleted_and_foreign_targets_never_form_a_group() {
    let mut engine = fixture();
    let (_, selected) = create(&mut engine);
    create(&mut engine);
    assert!(read(&mut engine, selected, None).surfaces.is_empty());
    let document = golden_core::node::NodeUuid(uuid::Uuid::new_v4());
    let ScreenEditResult::RelatedSurfaces { page } = apply(
        &mut engine,
        ScreenEdit::InspectRelatedSurfaces {
            query: RelatedSurfaceQuery {
                document,
                reference: selected,
                cursor: None,
            },
        },
    ) else {
        panic!()
    };
    assert!(!page.document_matches);
    assert!(page.surfaces.is_empty());
    apply(&mut engine, ScreenEdit::RemoveSurface { reference: selected });
    assert!(read(&mut engine, selected, None).surfaces.is_empty());
    assert!(engine.undo().unwrap());
    assert!(read(&mut engine, selected, None).source.is_none());
}

#[test]
fn scan_and_reply_are_bounded_and_pages_reach_sparse_matches_without_materializing_the_document() {
    let mut engine = fixture();
    let (_, selected) = create(&mut engine);
    let leaf = source(&mut engine);
    assign(&mut engine, selected, Some(leaf));
    let manager = super::super::authoring::manager(&engine).unwrap();
    let mut tree = NodeTree::new(super::super::ScreenNode::from_definition(
        "Many".into(),
        ScreenDefinition::default(),
    ));
    let mut expected = Vec::new();
    for index in 0..700 {
        let surface = super::super::SurfaceNode::from_definition(
            format!("Peer {index}"),
            SurfaceDefinition {
                source: (index >= 550).then_some(leaf),
                ..Default::default()
            },
        );
        if index >= 550 {
            expected.push(SurfaceId(surface.node_data().meta.uuid));
        }
        tree.children.push(NodeTree::new(surface).as_user_item());
    }
    engine.edits.push(Edit::AddUserItemTree {
        parent: manager,
        prev_sibling: engine.nodes.get(manager).unwrap().node_data().last_child,
        tree,
    });
    engine.apply_edits().unwrap();
    let history = engine.ui_history_state();
    let mut cursor = None;
    let mut found = Vec::new();
    let mut pages = 0;
    loop {
        let page = read(&mut engine, selected, cursor);
        assert!(page.surfaces.len() <= 64);
        if pages == 0 {
            assert!(page.surfaces.is_empty());
            assert!(page.next.is_some());
        }
        found.extend(page.surfaces.into_iter().map(|row| row.surface.reference.surface));
        pages += 1;
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
        assert!(pages < 8);
    }
    assert_eq!(found, expected);
    assert_eq!(engine.ui_history_state(), history);
    let root = engine.nodes.get(engine.root).unwrap().node_data().meta.uuid;
    assert!(
        !invoke(
            &mut engine,
            ScreenEdit::InspectRelatedSurfaces {
                query: RelatedSurfaceQuery {
                    document: root,
                    reference: selected,
                    cursor: Some(leaf.channel.0)
                }
            }
        )
        .success
    );
}
