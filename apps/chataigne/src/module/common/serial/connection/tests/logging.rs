use super::*;

#[test]
fn full_completion_feedback_preserves_fifo_input_and_every_retiring_output_result() {
    let pool = pool();
    let (mut handle, state) = connected(&pool);
    for byte in 0..100 {
        handle.send(&[byte], true).unwrap();
    }
    wait(|| handle.worker.event_metrics().rejected > 0);
    assert!(
        state.lock().unwrap().output.len() <= 65,
        "pause output behind its retained completion"
    );
    state.lock().unwrap().input.push_back(b"independent input".to_vec());
    wait(|| state.lock().unwrap().input.is_empty());
    handle.finish_admission();
    let mut logs = Vec::new();
    let mut input = Vec::new();
    wait(|| {
        let mut events = Vec::new();
        let drain = handle.drain_events(&mut events);
        for event in events {
            match event {
                Event::OutgoingLog(message) => logs.push(message),
                Event::Input(prepared) => input.extend(input_bytes(&prepared)),
                _ => {}
            }
        }
        drain.state == golden_io::PendingDrainState::Disconnected
    });
    assert_eq!(state.lock().unwrap().output, (0..100).collect::<Vec<u8>>());
    assert_eq!(input, b"independent input");
    let expected = (0..100)
        .map(|byte| crate::app::module::common::streaming::outgoing::sent_log("serial", &[byte]))
        .collect::<Vec<_>>();
    assert_eq!(logs, expected);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn unconfirmed_timeout_never_produces_a_success_log_or_replays_its_partial_bytes() {
    let pool = pool();
    let (mut handle, state) = connected(&pool);
    state.lock().unwrap().actions = VecDeque::from([Action::Count(2), Action::Fail(io::ErrorKind::TimedOut)]);
    handle.send(b"payload", true).unwrap();
    let mut warnings = 0;
    let mut logs = Vec::new();
    wait(|| {
        let mut events = Vec::new();
        handle.drain_events(&mut events);
        for event in events {
            match event {
                Event::OutgoingLog(message) => logs.push(message),
                Event::Warning(message) if message.contains("partial native effect") => warnings += 1,
                _ => {}
            }
        }
        warnings == 1
    });
    handle.finish_admission();
    wait(|| {
        let mut events = Vec::new();
        let drain = handle.drain_events(&mut events);
        assert!(!events.iter().any(|event| matches!(event, Event::OutgoingLog(_))));
        drain.state == golden_io::PendingDrainState::Disconnected
    });
    assert!(logs.is_empty());
    assert_eq!(state.lock().unwrap().output, b"pa");
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
