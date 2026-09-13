use super::*;

#[test]
fn full_subscription_queue_becomes_one_resync_without_losing_control_messages() {
    let mut client = client_with_subscription_count(0);
    client.subscriptions.insert(
        "workbench".to_string(),
        WsSubscriptionState {
            interest: UiInterest::workbench("view".to_string(), UiSubscriptionScope::WholeGraph),
            cursor: None,
            last_runtime_stats: None,
            pending_value_events: PendingValueEvents::default(),
            awaiting_resync: false,
        },
    );
    let outbound = client.outbound.clone();
    assert_eq!(outbound.push(reliable_message(7)), QueuePushResult::Queued);
    for tick in 0..(DEFAULT_OUTBOUND_CAPACITY - 1) {
        assert_eq!(outbound.push(multi_plane_message(tick as u64)), QueuePushResult::Queued);
    }

    let mut clients = HashMap::from([(7, client)]);
    let WsOutbound::Message(delta) = multi_plane_message(99) else {
        unreachable!()
    };
    send_to_client(&mut clients, 7, delta.clone());

    assert!(clients.contains_key(&7));
    assert!(clients[&7].subscriptions["workbench"].awaiting_resync);
    assert_eq!(outbound.len(), 2);
    assert!(matches!(
        outbound.pop(),
        Some(WsOutbound::Message(WsServerMessage::Hello { .. }))
    ));
    assert!(matches!(
        outbound.pop(),
        Some(WsOutbound::Message(WsServerMessage::ResyncRequired {
            subscription_id,
            plane: None,
            reason,
        })) if subscription_id == "workbench" && reason == "outbound_queue_overflow"
    ));

    send_to_client(&mut clients, 7, delta);
    assert_eq!(outbound.len(), 0);
}

#[test]
fn resync_replacement_preserves_other_subscriptions_and_rejects_if_no_room() {
    let queue = WsOutboundQueue::new(3);
    assert_eq!(queue.push(reliable_message(7)), QueuePushResult::Queued);
    assert_eq!(queue.push(multi_plane_message(1)), QueuePushResult::Queued);
    let mut other = multi_plane_message(2);
    let WsOutbound::Message(WsServerMessage::Delta { subscription_id, .. }) = &mut other else {
        unreachable!()
    };
    *subscription_id = "other".to_string();
    assert_eq!(queue.push(other), QueuePushResult::Queued);
    assert_eq!(
        queue.replace_subscription_with_resync("workbench", "outbound_queue_overflow"),
        QueuePushResult::Queued
    );
    assert_eq!(queue.len(), 3);
    assert!(matches!(
        queue.pop(),
        Some(WsOutbound::Message(WsServerMessage::Hello { .. }))
    ));
    assert!(matches!(
        queue.pop(),
        Some(WsOutbound::Message(WsServerMessage::Delta { subscription_id, .. })) if subscription_id == "other"
    ));
    assert!(matches!(
        queue.pop(),
        Some(WsOutbound::Message(WsServerMessage::ResyncRequired { .. }))
    ));

    let full = WsOutboundQueue::new(1);
    assert_eq!(full.push(reliable_message(8)), QueuePushResult::Queued);
    assert_eq!(
        full.replace_subscription_with_resync("workbench", "outbound_queue_overflow"),
        QueuePushResult::Full
    );
    assert!(matches!(
        full.pop(),
        Some(WsOutbound::Message(WsServerMessage::Hello { .. }))
    ));
}
