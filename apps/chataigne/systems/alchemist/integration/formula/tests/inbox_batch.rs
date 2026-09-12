use std::sync::Arc;

use golden_core::{
    engine::EngineTime,
    events::{Event, EventFrame, EventKind},
    node::NodeId,
};

use super::super::formula_inbox_requires_bulk;

#[test]
fn coalesces_multiple_structural_events_below_the_size_threshold() {
    let frame = |kinds: Vec<EventKind>| {
        EventFrame::from_shared(
            kinds
                .into_iter()
                .map(|kind| {
                    Arc::new(Event {
                        time: EngineTime { tick: 1, micro: 0, seq: 0 },
                        kind,
                    })
                })
                .collect(),
        )
    };
    let removed = || EventKind::ChildRemoved { parent: NodeId(1), child: NodeId(2) };
    let ordinary = || EventKind::NodeCreated { node: NodeId(2) };

    assert!(!formula_inbox_requires_bulk(&frame(vec![removed()])));
    assert!(formula_inbox_requires_bulk(&frame(vec![removed(), removed()])));
    assert!(!formula_inbox_requires_bulk(&frame((0..31).map(|_| ordinary()).collect())));
    assert!(formula_inbox_requires_bulk(&frame((0..32).map(|_| ordinary()).collect())));
}
