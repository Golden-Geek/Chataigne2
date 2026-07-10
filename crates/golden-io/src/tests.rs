use super::*;

#[test]
fn recovery_is_event_driven_and_exponentially_bounded() {
    let mut recovery = RecoveryStateMachine::new(RecoveryPolicy {
        initial_delay_ms: 100,
        maximum_delay_ms: 250,
        multiplier: 2,
    })
    .unwrap();
    assert!(recovery.begin_connect());
    assert!(recovery.disconnected(1_000));
    assert!(!recovery.retry_due(1_099));
    assert!(recovery.retry_due(1_100));
    assert!(recovery.begin_connect());
    assert!(recovery.disconnected(2_000));
    assert_eq!(
        recovery.state(),
        ConnectionState::WaitingToRetry {
            attempt: 3,
            retry_at_ms: 2_200,
        }
    );
}

#[test]
fn ingress_policy_never_grows_past_its_bound() {
    let mut latest = BoundedIngress::new(2, IngressPolicy::LatestWins).unwrap();
    for value in 0..10_000 {
        latest.push(value).unwrap();
    }
    assert_eq!(latest.len(), 2);
    assert_eq!(latest.pop(), Some(9_998));
    assert_eq!(latest.pop(), Some(9_999));

    let mut lossless = BoundedIngress::new(1, IngressPolicy::Lossless).unwrap();
    lossless.push(1).unwrap();
    assert_eq!(lossless.push(2), Err(IngressError::Full));
}

#[test]
fn endpoint_recovery_survives_a_long_disconnect_reconnect_soak() {
    let mut recovery = RecoveryStateMachine::new(RecoveryPolicy::default()).unwrap();
    for cycle in 0..100_000_u64 {
        assert!(recovery.begin_connect());
        recovery.connected(cycle * 2);
        assert!(recovery.disconnected(cycle * 2 + 1));
        assert!(recovery.retry_due(cycle * 2 + 1 + 250));
        assert!(recovery.begin_connect());
        recovery.connected(cycle * 2 + 1 + 250);
        recovery.stop();
        recovery = RecoveryStateMachine::new(RecoveryPolicy::default()).unwrap();
    }
    assert_eq!(recovery.state(), ConnectionState::Disconnected);
}
