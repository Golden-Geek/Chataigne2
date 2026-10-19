use super::*;
use crate::app::module::common::streaming::parser::StreamingSeparator;

fn next_input(handle: &TcpServerTransportHandle) -> RetainedPayload<PreparedStreamInput> {
    let mut packet = None;
    until(handle, |event| match event {
        TcpServerWorkerEvent::Input(input) => {
            packet = Some(input.clone());
            true
        }
        TcpServerWorkerEvent::Error(error) => panic!("{error}"),
        _ => false,
    });
    packet.unwrap()
}

#[test]
fn interleaved_clients_have_independent_partial_frames_and_callback_source_ids() {
    let handle = spawn(config(0)).unwrap();
    let address = listening(&handle);
    let mut left = connect(&handle, address);
    let mut right = connect(&handle, address);
    left.write_all(b"left ").unwrap();
    let partial = next_input(&handle);
    assert!(partial.values.as_ref().unwrap().is_empty());
    assert_eq!(
        partial.callbacks[2].payload()["args"][1],
        left.local_addr().unwrap().to_string()
    );
    right.write_all(b"right 2\n").unwrap();
    let complete = next_input(&handle);
    assert_eq!(complete.values.as_ref().unwrap()[0].path_segments, ["right"]);
    assert_eq!(
        complete.callbacks[2].payload()["args"][1],
        right.local_addr().unwrap().to_string()
    );
    left.write_all(b"1\n").unwrap();
    let complete = next_input(&handle);
    assert_eq!(complete.values.as_ref().unwrap()[0].path_segments, ["left"]);
    assert_eq!(complete.rejected_lines, 0);
}

#[test]
fn grammar_and_processing_changes_fence_each_client_partial_frame_without_rebinding() {
    let handle = spawn(config(0)).unwrap();
    let address = listening(&handle);
    let mut left = connect(&handle, address);
    let mut right = connect(&handle, address);
    for client in [&mut left, &mut right] {
        client.write_all(b"old ").unwrap();
        assert!(next_input(&handle).values.as_ref().unwrap().is_empty());
    }
    let mut input = handle.input.snapshot().config;
    input.parse.name_separator = Some(StreamingSeparator::Colon);
    input.log_incoming = true;
    handle.update_input(input.clone()).unwrap();
    for client in [&mut left, &mut right] {
        client.write_all(b"new:2\n").unwrap();
        let new = next_input(&handle);
        assert_eq!(new.rejected_lines, 1);
        assert_eq!(new.values.as_ref().unwrap()[0].path_segments, ["new"]);
        assert!(new
            .log_message
            .as_ref()
            .unwrap()
            .starts_with("Received TCP server from "));
    }
    left.write_all(b"abandoned:").unwrap();
    assert!(next_input(&handle).values.as_ref().unwrap().is_empty());
    input.processing_enabled = false;
    handle.update_input(input.clone()).unwrap();
    input.processing_enabled = true;
    handle.update_input(input).unwrap();
    left.write_all(b"fresh:3\n").unwrap();
    let fresh = next_input(&handle);
    assert_eq!(fresh.rejected_lines, 1);
    assert_eq!(fresh.values.as_ref().unwrap()[0].path_segments, ["fresh"]);
    assert_eq!(handle.clients.load(Ordering::Acquire), 2);
}

#[test]
fn disconnect_prepares_one_callback_and_reports_abandoned_input_before_a_new_client() {
    let handle = spawn(config(0)).unwrap();
    let address = listening(&handle);
    let mut client = connect(&handle, address);
    let label = client.local_addr().unwrap().to_string();
    client.write_all(b"old ").unwrap();
    assert!(next_input(&handle).values.as_ref().unwrap().is_empty());
    drop(client);
    until(&handle, |event| match event {
        TcpServerWorkerEvent::ClientDisconnected {
            client_id,
            rejected_lines,
            callback,
            ..
        } => {
            assert_eq!(client_id, &label);
            assert_eq!(*rejected_lines, 1);
            let payload = callback.as_ref().unwrap().payload();
            assert_eq!(payload["callback"], "clientDisconnected");
            assert_eq!(payload["args"][0], label);
            true
        }
        _ => false,
    });
    let mut replacement = connect(&handle, address);
    replacement.write_all(b"fresh 4\n").unwrap();
    assert_eq!(next_input(&handle).values.as_ref().unwrap()[0].path_segments, ["fresh"]);
}

#[test]
fn disabled_processing_retains_source_callbacks_and_log_only_edits_keep_partial_frames() {
    let handle = spawn(config(0)).unwrap();
    let address = listening(&handle);
    let mut client = connect(&handle, address);
    client.write_all(b"kept ").unwrap();
    assert!(next_input(&handle).values.as_ref().unwrap().is_empty());
    let mut input = handle.input.snapshot().config;
    input.log_incoming = true;
    handle.update_input(input.clone()).unwrap();
    client.write_all(b"7\n").unwrap();
    let kept = next_input(&handle);
    assert_eq!(kept.rejected_lines, 0);
    assert_eq!(kept.values.as_ref().unwrap()[0].path_segments, ["kept"]);
    input.processing_enabled = false;
    handle.update_input(input).unwrap();
    client.write_all(b"ignored 9\n").unwrap();
    let ignored = next_input(&handle);
    assert!(ignored.values.as_ref().unwrap().is_empty());
    assert_eq!(ignored.callbacks.len(), 3);
    assert_eq!(
        ignored.callbacks[2].payload()["args"][1],
        client.local_addr().unwrap().to_string()
    );
    assert!(ignored.log_message.as_ref().unwrap().ends_with("(processing disabled)"));
}

#[test]
fn invalid_input_update_keeps_the_previous_generation_and_live_parser_settings() {
    let handle = spawn(config(0)).unwrap();
    let address = listening(&handle);
    let mut client = connect(&handle, address);
    let before = handle.input.snapshot();
    let mut invalid = before.config.clone();
    invalid.parse.mode.reserve(17);
    assert!(handle.update_input(invalid).is_err());
    let after = handle.input.snapshot();
    assert_eq!(after.generation, before.generation);
    assert_eq!(after.config, before.config);
    client.write_all(b"unchanged 3\n").unwrap();
    assert_eq!(
        next_input(&handle).values.as_ref().unwrap()[0].path_segments,
        ["unchanged"]
    );
    assert_eq!(handle.clients.load(Ordering::Acquire), 1);
}
