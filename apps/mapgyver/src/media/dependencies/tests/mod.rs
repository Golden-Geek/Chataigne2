use super::*;
use crate::media::{
    MediaManagerNode,
    authoring::{manager, media_tree},
    composition::{InstanceNode, protocol::*},
    protocol::*,
    tests::{apply, create, fixture, invoke},
};
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    edit::{Edit, NodeTree},
};

fn reference(index: u128) -> ChannelReference {
    ChannelReference {
        media: MediaId(golden_core::node::NodeUuid(uuid::Uuid::from_u128(index))),
        channel: ChannelId(golden_core::node::NodeUuid(uuid::Uuid::from_u128(index))),
    }
}

#[test]
fn a_hundred_thousand_dependencies_validate_on_a_small_stack_and_detect_a_closing_cycle() {
    std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(|| {
            const COUNT: u128 = 100_000;
            let mut graph = (1..=COUNT)
                .map(|index| (reference(index), vec![reference(index + 1)]))
                .collect::<HashMap<_, _>>();
            // The last source is deliberately unresolved: missing resources remain authored leaves.
            assert!(validate(graph.clone()).is_ok());
            graph.get_mut(&reference(COUNT)).unwrap().push(reference(1));
            assert!(validate(graph).is_err());

            let mut graph = (2..=COUNT)
                .map(|index| (reference(index), Vec::new()))
                .collect::<HashMap<_, _>>();
            let sources = (2..=COUNT).flat_map(|index| [reference(index); 2]).collect();
            graph.insert(reference(1), sources);
            // Repeated instances of one source are edges, never duplicate owned media.
            assert!(validate(graph).is_ok());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn a_deep_authored_composition_forest_rejects_feedback_without_mutation_or_source_duplication() {
    const DEPTH: usize = 4096;
    let mut engine = fixture();
    let (leaf_tree, leaf) = media_tree(
        "Shared leaf".into(),
        ChannelDefinition::for_media(MediaKind::SolidColor),
    );
    let old_manager = manager(&engine).unwrap();
    let mut forest = NodeTree::new(MediaManagerNode::new()).with_child(leaf_tree);
    let before_nodes = engine.nodes.len();
    let mut next = leaf;
    let mut bottom = None;
    for index in 0..DEPTH {
        let (mut tree, owner) = media_tree(
            format!("Nested {index}"),
            ChannelDefinition::for_media(MediaKind::Composition),
        );
        for (instance, source) in [next, leaf].into_iter().enumerate() {
            tree.children[0].push_child(
                NodeTree::new(InstanceNode::from_definition(
                    format!("Instance {instance}"),
                    InstanceDefinition {
                        source: Some(source),
                        ..InstanceDefinition::default()
                    },
                ))
                .as_user_item(),
            );
        }
        bottom.get_or_insert(owner);
        next = owner;
        forest.push_child(tree);
    }
    // One detached forest avoids thousands of full snapshot rebuilds during fixture creation.
    let started = std::time::Instant::now();
    engine.edits.push(Edit::RemoveNode { node: old_manager });
    engine.edits.push(Edit::AddNodeTree {
        parent: engine.root,
        prev_sibling: None,
        tree: forest,
    });
    engine.apply_edits().unwrap();
    // Media/channel each have six Parameters plus their folder; two full instances per branch.
    assert_eq!(engine.nodes.len(), before_nodes + 16 + DEPTH * 38);
    crate::media::validate_project(&engine).unwrap();
    eprintln!("4096-deep Composition forest and validation: {:?}", started.elapsed());
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let ack = invoke(
        &mut engine,
        MediaEdit::Composition {
            reference: bottom.unwrap(),
            edit: CompositionEdit::AddInstances { sources: vec![next] },
        },
    );
    assert!(!ack.success);
    assert!(
        ack.error_message
            .as_deref()
            .is_some_and(|message| message.contains("dependency cycle")),
        "{ack:?}"
    );
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    assert!(crate::media::channel(&engine, leaf).is_ok());
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    crate::media::validate_project(&loaded).unwrap();
    assert_eq!(loaded.nodes.len(), engine.nodes.len());
    assert!(crate::media::channel(&loaded, next).is_ok());
}

#[test]
fn every_media_kind_remains_a_reference_after_source_removal_and_undo() {
    for kind in [
        MediaKind::SolidColor,
        MediaKind::Picture,
        MediaKind::SharedTexture,
        MediaKind::Composition,
        MediaKind::Matrix,
    ] {
        let mut engine = fixture();
        let source = create(&mut engine, kind);
        let owner = create(&mut engine, MediaKind::Composition);
        apply(
            &mut engine,
            MediaEdit::Composition {
                reference: owner,
                edit: CompositionEdit::AddInstances {
                    sources: vec![source, source],
                },
            },
        );
        let inspect = |engine: &mut AppEngine| {
            apply(
                engine,
                MediaEdit::Composition {
                    reference: owner,
                    edit: CompositionEdit::Inspect,
                },
            )
        };
        let before = serde_json::to_value(inspect(&mut engine)).unwrap();
        let before_nodes = engine.nodes.len();
        let source_before = crate::media::channel(&engine, source).unwrap();
        apply(&mut engine, MediaEdit::RemoveMedia { media: source.media });
        crate::media::validate_project(&engine).unwrap();
        assert!(crate::media::channel(&engine, source).is_err(), "{kind:?}");
        assert_eq!(serde_json::to_value(inspect(&mut engine)).unwrap(), before);
        assert!(engine.undo().unwrap());
        crate::media::validate_project(&engine).unwrap();
        assert_eq!(engine.nodes.len(), before_nodes);
        assert_eq!(crate::media::channel(&engine, source).unwrap(), source_before);
        assert_eq!(serde_json::to_value(inspect(&mut engine)).unwrap(), before);
        assert!(engine.redo().unwrap());
        crate::media::validate_project(&engine).unwrap();
        assert!(crate::media::channel(&engine, source).is_err());
        assert_eq!(serde_json::to_value(inspect(&mut engine)).unwrap(), before);
    }
}
