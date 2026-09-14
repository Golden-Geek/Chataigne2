use std::{collections::HashSet, path::Path, sync::Arc};

use golden_core::{
    app::load_sparse_project_file,
    engine::EngineTime,
    events::{CustomEvent, Event, EventFrame, EventKind},
    node::{DeclId, Node, NodeId},
    parameter::ParamValue,
};

use crate::app::AppNode;

use super::super::{StateProcessor, StateProcessorFolder, StateProcessorManager};

#[test]
fn condition_valid_cache_finds_the_real_processor_child() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("samples")
        .join("test_simple_load.noisette");
    let engine = load_sparse_project_file::<AppNode, _>(fixture).expect("product fixture should load");
    let snapshot = engine.process_tree_snapshot();
    let processor = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessor::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("product fixture should contain a processor");
    let condition_manager = snapshot
        .child_ids_slice(processor)
        .iter()
        .copied()
        .find(|child| snapshot.node(*child).is_some_and(|node| node.node_type == "sm_condition_manager"))
        .expect("processor should contain a condition manager");
    let valid = snapshot
        .find_child_by_decl_id(condition_manager, "valid")
        .expect("condition manager should expose its valid result");

    assert_eq!(
        StateProcessor::condition_valid_params_in_snapshot(snapshot.as_ref(), processor),
        HashSet::from([valid]),
    );
}

#[test]
fn condition_valid_updates_skip_surface_snapshot_but_other_edits_do_not() {
    let mut processor = StateProcessor::new();
    let valid = NodeId(42);
    processor.condition_valid_params.insert(valid);
    let time = EngineTime {
        tick: 1,
        micro: 0,
        seq: 0,
    };
    let changed = |param| {
        Arc::new(Event {
            time,
            kind: EventKind::ParamChanged {
                param,
                old_value: ParamValue::Bool(false),
                new_value: ParamValue::Bool(true),
            },
        })
    };
    let custom = Arc::new(Event::custom(
        time,
        CustomEvent::new("unrelated", None, serde_json::Value::Null),
    ));
    assert!(!processor.inbox_requires_tree_snapshot(&EventFrame::from_shared(vec![changed(valid), custom])));
    assert!(processor.inbox_requires_tree_snapshot(&EventFrame::from_shared(vec![changed(NodeId(43))])));
    assert!(processor.inbox_requires_tree_snapshot(&EventFrame::from_shared(vec![Arc::new(Event {
        time,
        kind: EventKind::ChildAdded {
            parent: processor.id(),
            child: NodeId(44),
            decl_id: DeclId("surface".to_owned()),
        },
    })])));
}

#[test]
fn processor_palette_manager_requests_snapshot_only_for_structural_events() {
    let manager = StateProcessorManager::new();
    let folder = StateProcessorFolder::new();
    let time = EngineTime {
        tick: 1,
        micro: 0,
        seq: 0,
    };
    let changed = Arc::new(Event {
        time,
        kind: EventKind::ParamChanged {
            param: NodeId(42),
            old_value: ParamValue::Float(1.0),
            new_value: ParamValue::Float(2.0),
        },
    });
    for palette in [&manager as &dyn Node, &folder as &dyn Node] {
        assert!(!palette.inbox_requires_tree_snapshot(&EventFrame::from_shared(vec![Arc::clone(&changed)])));
    }
    let added = Arc::new(Event {
        time,
        kind: EventKind::ChildAdded {
            parent: NodeId(1),
            child: NodeId(43),
            decl_id: DeclId("formula".to_owned()),
        },
    });
    for palette in [&manager as &dyn Node, &folder as &dyn Node] {
        assert!(palette.inbox_requires_tree_snapshot(&EventFrame::from_shared(vec![Arc::clone(&added)])));
    }
}
