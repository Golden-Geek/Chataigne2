use super::*;

#[test]
fn native_host_suspension_preserves_exact_remaining_lease_without_replacing_geometry() {
    let (control, reader, session) = fixture();
    layout(&control, session, 1, vec![view()]);
    let before = reader.snapshot();
    let start = Instant::now();
    let original_deadline = start + Duration::from_millis(2500);
    let mut state = control.0.state.lock().unwrap();
    state.session.as_mut().unwrap().deadline = original_deadline;
    assert!(!state.set_host_suspended(true, start + Duration::from_secs(1)));
    assert!(!state.expire(start + Duration::from_secs(10)));
    assert!(!state.set_host_suspended(true, start + Duration::from_secs(10)));
    assert!(!state.set_host_suspended(false, start + Duration::from_secs(11)));
    assert_eq!(
        state.session.as_ref().unwrap().deadline,
        original_deadline + Duration::from_secs(10)
    );
    assert!(!state.expire(start + Duration::from_millis(12499)));
    assert!(before.guard.is_current());
    assert!(state.expire(start + Duration::from_millis(12500)));
    assert!(!before.guard.is_current());
}

#[test]
fn suspension_never_revives_an_expired_owner_or_prevents_explicit_retirement() {
    let (control, reader, session) = fixture();
    layout(&control, session, 1, vec![view()]);
    let before = reader.snapshot();
    {
        let mut state = control.0.state.lock().unwrap();
        let deadline = Instant::now();
        state.session.as_mut().unwrap().deadline = deadline;
        assert!(state.set_host_suspended(true, deadline));
        assert!(state.session.is_none());
    }
    assert!(!before.guard.is_current());
    control.close();
    reader.set_host_suspended(false);
    assert!(reader.snapshot().views.is_empty());
    assert!(matches!(
        control.handle(
            PreviewRequest::Connect {
                client: Uuid::new_v4(),
                version: PREVIEW_PROTOCOL_VERSION
            },
            host()
        ),
        PreviewResponse::Rejected {
            code: PreviewRejection::Closed
        }
    ));
}

#[test]
fn explicit_disconnect_still_withdraws_a_suspended_live_session() {
    let (control, reader, session) = fixture();
    layout(&control, session, 1, vec![view()]);
    let before = reader.snapshot();
    reader.set_host_suspended(true);
    assert!(reader.next_deadline().is_none());
    assert_eq!(
        control.handle(PreviewRequest::Disconnect { session }, host()),
        PreviewResponse::Disconnected
    );
    assert!(!before.guard.is_current());
    reader.set_host_suspended(false);
    assert!(reader.snapshot().views.is_empty());
    assert!(reader.next_deadline().is_none());
}
