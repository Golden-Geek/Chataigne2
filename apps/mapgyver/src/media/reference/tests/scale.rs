//! Explicit M04 Reference-path acceptance; full Q04 desktop/render qualification remains separate.
use super::*;
use golden_core::{
    edit::{Edit, NodeTree},
    node::Node,
};
use std::time::Instant;

fn percentile(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values[(values.len() * 95).div_ceil(100).saturating_sub(1)]
}

#[test]
#[ignore = "explicit large authored Reference read/edit and bounded capture acceptance"]
fn reference_pages_and_fixed_edits_remain_bounded_with_ten_thousand_inactive_media() {
    let mut engine = fixture();
    let manager = crate::media::authoring::manager(&engine).unwrap();
    let definition = ChannelDefinition::for_media(MediaKind::SolidColor);
    let (mut tree, source) = crate::media::authoring::media_tree("Scale producer".into(), definition.clone());
    for index in 1..512 {
        tree.push_child(
            NodeTree::new(crate::media::ChannelNode::from_definition(
                format!("Variant {index}"),
                definition.clone(),
            ))
            .as_user_item(),
        );
    }
    engine.edits.push(Edit::AddUserItemTree {
        parent: manager,
        prev_sibling: None,
        tree,
    });
    engine.apply_edits().unwrap();
    let mut references = Vec::new();
    for count in [1000, 5000, 10000] {
        let started = Instant::now();
        let folder = crate::media::MediaFolderNode::new(format!("References to {count}"));
        let mut tree = NodeTree::new(folder);
        for index in references.len()..count {
            let node = ReferenceNode::from_source(
                format!("Reference {index}"),
                Some(crate::media::MediaSource {
                    media: source.media,
                    channel: Some(source.channel),
                }),
            );
            references.push(MediaId(node.node_data().meta.uuid));
            tree.push_child(NodeTree::new(node).as_user_item());
        }
        engine.edits.push(Edit::AddUserItemTree {
            parent: manager,
            prev_sibling: None,
            tree,
        });
        engine.apply_edits().unwrap();
        let setup_ms = started.elapsed().as_secs_f64() * 1000.0;
        let history = engine.undo_len();
        let mut query_ms = Vec::new();
        let mut edit_ms = Vec::new();
        let mut payload_bytes = 0;
        for _ in 0..25 {
            let started = Instant::now();
            let MediaEditResult::Channels { page } = apply(
                &mut engine,
                MediaEdit::InspectChannels {
                    query: MediaChannelsQuery {
                        media: *references.last().unwrap(),
                        start: 0,
                        count: 32,
                    },
                },
            ) else {
                panic!()
            };
            query_ms.push(started.elapsed().as_secs_f64() * 1000.0);
            assert_eq!((page.total, page.channels.len()), (1, 1));
            payload_bytes = serde_json::to_vec(&page).unwrap().len();
            assert!(payload_bytes < 32768);
        }
        assert_eq!(engine.undo_len(), history, "reads never change authored history");
        for target in [None, Some(source.media)].into_iter().cycle().take(50) {
            let started = Instant::now();
            apply(
                &mut engine,
                MediaEdit::ConfigureReference {
                    media: references[count / 2],
                    target,
                },
            );
            edit_ms.push(started.elapsed().as_secs_f64() * 1000.0);
        }
        let active = references
            .iter()
            .take(15)
            .map(|media| crate::media::resolve_source_node(&engine, media.0).unwrap())
            .collect::<Vec<_>>();
        let started = Instant::now();
        let captured = crate::media::capture_channels(&engine, &active, 16).unwrap();
        let capture_ms = started.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(
            captured.channels.len(),
            16,
            "fifteen logical consumers capture one shared producer"
        );
        let query_p95_ms = percentile(&mut query_ms);
        let edit_p95_ms = percentile(&mut edit_ms);
        eprintln!(
            "{}",
            serde_json::json!({
                "scope":"M04 Reference backend path; not whole-app Q04", "authored_references":count,
                "actual_nodes":engine.nodes.len(), "selected_outputs":count, "source_channels":512,
                "incremental_batched_setup_ms":setup_ms, "query_samples":query_ms.len(), "query_p95_ms":query_p95_ms,
                "edit_samples":edit_ms.len(), "edit_p95_ms":edit_p95_ms,
                "captured_records":captured.channels.len(), "capture_ms":capture_ms, "page_bytes":payload_bytes,
            })
        );
        assert!(
            query_p95_ms <= 50.0 && edit_p95_ms <= 50.0,
            "Reference receipt budget exceeded at {count}: query={query_p95_ms} edit={edit_p95_ms}"
        );
    }
}
