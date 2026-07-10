use std::sync::Arc;

use golden_engine::engine::EngineTime;
use golden_protocol::UiEventBatch;

use super::{TransportMetrics, WsOutbound, WsOutboundQueue, WsServerMessage};

fn batch(subscription_id: &str) -> WsOutbound {
    WsOutbound::Message(WsServerMessage::Batch {
        subscription_id: subscription_id.to_string(),
        batch: UiEventBatch {
            from: None,
            to: Some(EngineTime {
                tick: 1,
                micro: 0,
                seq: 0,
            }),
            runtime: None,
            events: Vec::new(),
        },
    })
}

#[test]
fn outbound_queue_is_bounded_and_degrades_batches_to_resync() {
    let metrics = Arc::new(TransportMetrics::default());
    let queue = WsOutboundQueue::new(2, metrics.clone());
    queue.send(batch("a")).unwrap();
    queue.send(batch("b")).unwrap();
    queue.send(batch("c")).unwrap();

    let mut resyncs = Vec::new();
    while let Ok(outbound) = queue.try_recv() {
        if let WsOutbound::Message(WsServerMessage::ResyncRequired {
            subscription_id,
            reason,
        }) = outbound
        {
            assert_eq!(reason, "outbound_queue_pressure");
            resyncs.push(subscription_id);
        }
    }
    assert_eq!(resyncs, vec!["a", "b"]);
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.dropped_outbound_messages, 3);
    assert_eq!(snapshot.resync_requests, 2);
}

#[test]
fn fifty_slow_clients_keep_bounded_queues_and_receive_resync_markers() {
    let metrics = Arc::new(TransportMetrics::default());
    let clients = (0..50)
        .map(|_| WsOutboundQueue::new(8, metrics.clone()))
        .collect::<Vec<_>>();
    for sequence in 0..1_000 {
        for client in &clients {
            client.send(batch(&format!("scope-{sequence}"))).unwrap();
        }
    }

    for client in clients {
        let mut queued = 0usize;
        let mut saw_resync = false;
        while let Ok(outbound) = client.try_recv() {
            queued += 1;
            saw_resync |= matches!(outbound, WsOutbound::Message(WsServerMessage::ResyncRequired { .. }));
        }
        assert!(queued <= 8);
        assert!(saw_resync);
    }
    assert!(metrics.snapshot().dropped_outbound_messages > 0);
}
