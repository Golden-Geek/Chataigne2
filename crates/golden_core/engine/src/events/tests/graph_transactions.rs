use super::*;
use crate::ui_sync::{UiEventDto, UiEventKind, UiGraphOp, UiGraphTransaction};

#[test]
fn graph_event_clones_share_immutable_payload_and_keep_the_serialized_transaction_shape() {
    let transaction = UiGraphTransaction {
        tx_id: 1,
        epoch: 1,
        base_graph_version: 0,
        next_graph_version: 1,
        ops: vec![UiGraphOp::ChildrenReordered {
            parent: NodeId(1),
            children: vec![NodeId(2), NodeId(3)],
        }],
    };
    let payload = Arc::new(transaction.clone());
    let event = Event {
        time: EngineTime {
            tick: 0,
            micro: 0,
            seq: 1,
        },
        kind: EventKind::GraphTransaction {
            transaction: payload.clone(),
        },
    };
    let copy = event.clone();
    let EventKind::GraphTransaction { transaction: shared } = &copy.kind else {
        panic!()
    };
    assert!(Arc::ptr_eq(&payload, shared));
    let encoded = serde_json::to_value(&event).unwrap();
    assert_eq!(
        encoded["kind"]["GraphTransaction"]["transaction"],
        serde_json::to_value(&transaction).unwrap()
    );
    let decoded: Event = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, event);
    assert_eq!(
        UiEventDto::from(copy).kind,
        UiEventKind::GraphTransaction { transaction }
    );
    assert_eq!(
        payload.ops.len(),
        1,
        "encoding and projection never alter retained payloads"
    );
}
