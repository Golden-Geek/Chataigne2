use super::*;

#[test]
fn processing_generation_discards_partial_frames_even_when_no_bytes_arrive_while_disabled() {
    let pool = pool();
    let (handle, state) = connected(&pool);
    state.lock().unwrap().input.push_back(b"old ".to_vec());
    until(
        &handle,
        |event| matches!(event, Event::Input(input) if input_bytes(input) == b"old " && input.values.as_ref().unwrap().is_empty()),
    );
    let mut input = config().input;
    input.processing_enabled = false;
    handle.update_input(input.clone()).unwrap();
    input.processing_enabled = true;
    handle.update_input(input).unwrap();
    state.lock().unwrap().input.push_back(b"fresh 9\n".to_vec());
    until(&handle, |event| match event {
        Event::Input(input) if input_bytes(input) == b"fresh 9\n" => {
            assert_eq!(input.rejected_lines, 1);
            assert_eq!(input.values.as_ref().unwrap()[0].path_segments, ["fresh"]);
            assert_eq!(input.callbacks.len(), 3);
            assert!(input.callbacks[0].payload()["args"][1].is_null());
            true
        }
        _ => false,
    });
}
#[test]
fn retiring_input_keeps_prepared_callbacks_and_reports_its_partial_frame() {
    let pool = pool();
    let (mut handle, state) = connected(&pool);
    state.lock().unwrap().input.push_back(b"partial".to_vec());
    // The connection status is already queued; wait for the admitted input too.
    wait(|| handle.worker.event_metrics().depth >= 2);
    handle.finish_admission();
    let mut received = Vec::new();
    let mut rejected = 0;
    wait(|| {
        let mut events = Vec::new();
        let drain = handle.drain_events(&mut events);
        for event in events {
            match event {
                Event::Input(input) => received.extend(input_bytes(&input)),
                Event::RejectedInput(count) => rejected += count,
                _ => {}
            }
        }
        drain.state == golden_io::PendingDrainState::Disconnected
    });
    assert_eq!(received, b"partial");
    assert_eq!(rejected, 1);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn disabled_processing_keeps_binary_callbacks_without_a_text_hint_or_values() {
    let pool = pool();
    let (handle, state) = connected(&pool);
    let mut input = config().input;
    input.processing_enabled = false;
    handle.update_input(input).unwrap();
    state.lock().unwrap().input.push_back(vec![255, 0, 1]);
    until(&handle, |event| match event {
        Event::Input(input) => {
            assert_eq!(input_bytes(input), [255, 0, 1]);
            assert_eq!(input.callbacks.len(), 2);
            assert!(input.values.as_ref().unwrap().is_empty());
            true
        }
        _ => false,
    });
}
