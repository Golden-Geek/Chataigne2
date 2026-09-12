use std::sync::Arc;

use golden_core::{
    engine::EngineTime,
    events::{CustomEvent, Event, EventFrame, EventKind},
    node::{Node, NodeId, NodeMetaPatch},
};

use super::StateMachineState;

fn frame(kinds: impl IntoIterator<Item = EventKind>) -> EventFrame {
    let time = EngineTime {
        tick: 1,
        micro: 0,
        seq: 0,
    };
    EventFrame::from_shared(
        kinds
            .into_iter()
            .map(|kind| Arc::new(Event { time, kind }))
            .collect(),
    )
}

#[test]
fn state_only_requests_inbox_snapshot_for_its_enabled_change() {
    let state = StateMachineState::new();
    let state_id = state.id();
    let custom = EventKind::Custom(CustomEvent::new("state.changed", None, serde_json::Value::Null));
    assert!(!state.inbox_requires_tree_snapshot(&frame([custom.clone()])));
    assert!(!state.inbox_requires_tree_snapshot(&frame([EventKind::MetaChanged {
        node: NodeId(u64::MAX),
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..NodeMetaPatch::default()
        },
    }])));
    assert!(!state.inbox_requires_tree_snapshot(&frame([EventKind::MetaChanged {
        node: state_id,
        patch: NodeMetaPatch {
            label: Some("renamed".to_owned()),
            ..NodeMetaPatch::default()
        },
    }])));
    assert!(state.inbox_requires_tree_snapshot(&frame([
        custom,
        EventKind::MetaChanged {
            node: state_id,
            patch: NodeMetaPatch {
                enabled: Some(false),
                ..NodeMetaPatch::default()
            },
        },
    ])));
}
