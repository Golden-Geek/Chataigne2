use super::super::super::midi_output::MidiOutputRequest;
use super::*;
use crate::app::{MidiSendPacket, MidiSendRequest};
use golden_core::node::Folder;

mod feedback;

fn packet(bytes: &[u8], delay_ms: u64) -> MidiSendPacket {
    MidiSendPacket {
        bytes: bytes.to_vec(),
        description: "test".into(),
        delay_ms,
    }
}
fn full_note(delay_ms: u64) -> MidiSendRequest {
    MidiSendRequest {
        packets: vec![packet(&[0x90, 60, 100], 0), packet(&[0x80, 60, 0], delay_ms)],
    }
}
fn fake_output(pool: &RetirementPool) -> (MidiOutputHandle, Arc<Mutex<Vec<Vec<u8>>>>) {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let copy = sent.clone();
    let output = MidiOutputHandle::spawn_using(output_config(), pool, notify(), move |_| {
        Ok(FakeOutput {
            sent: copy,
            send_gate: None,
            drop_gate: None,
            fail: false,
        })
    })
    .unwrap();
    until(|| output.is_ready());
    (output, sent)
}

#[test]
fn feedback_pairs_share_admission_and_prepare_within_their_fixed_size_reservation() {
    let pool = pool();
    let (mut output, sent) = fake_output(&pool);
    let engine = crate::app::AppEngine::new(Folder::new("producer").into());
    let request = MidiOutputRequest::feedback(
        crate::app::encode_14_bit_control_change(2, 1, 0x1234),
        engine.node_waker(engine.root).unwrap(),
        false,
    )
    .unwrap();
    output.enqueue(request).unwrap();
    output.finish_admission();
    let mut events = Vec::new();
    until(|| output.drain(&mut events).state == PendingDrainState::Disconnected);
    assert_eq!(&*sent.lock().unwrap(), &[vec![0xB1, 1, 0x24], vec![0xB1, 33, 0x34]]);
}

fn module_fixture(
    pool: &RetirementPool,
) -> (
    crate::app::AppEngine,
    golden_core::node::NodeId,
    Arc<Mutex<Vec<Vec<u8>>>>,
) {
    use golden_core::node::Node;
    let mut engine = crate::app::AppEngine::new(Folder::new("root").into());
    engine.add_node(crate::app::MidiModule::create().into(), None);
    for _ in 0..6 {
        engine.apply_edits().unwrap();
    }
    let module = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let (output, sent) = fake_output(pool);
    let module_node = engine
        .nodes
        .get_mut(module)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<crate::app::MidiModule>()
        .unwrap();
    module_node.output = Some(output);
    (engine, module, sent)
}
#[test]
fn all_script_aliases_reach_the_native_worker_with_canonical_arguments() {
    use golden_core::{edit::Edit, parameter::ParamValue, script::ScriptMethodCall};
    let pool = pool();
    let (mut engine, module, sent) = module_fixture(&pool);
    let mut expected = 0;
    for method in [
        "sendNoteOn",
        "sendNoteOff",
        "sendFullNote",
        "sendCC",
        "sendControlChange",
        "sendProgramChange",
        "sendPitchBend",
        "sendChannelPressure",
        "sendPolyPressure",
        "sendSysEx",
        "sendSysex",
        "sendRawBytes",
    ] {
        let args = if matches!(method, "sendSysEx" | "sendSysex" | "sendRawBytes") {
            vec![ParamValue::Str("F0 01 F7".into())]
        } else {
            vec![
                ParamValue::Int(1),
                ParamValue::Int(60),
                ParamValue::Int(100),
                ParamValue::Int(0),
                ParamValue::Int(0),
            ]
        };
        engine.edits.push(Edit::CallNodeScriptMethod {
            node: module,
            call: ScriptMethodCall::prepare(method, args).unwrap(),
        });
        engine.apply_edits().unwrap();
        expected += if method == "sendFullNote" { 2 } else { 1 };
        until(|| sent.lock().unwrap().len() == expected);
    }
    assert_eq!(expected, 13);
    assert_eq!(
        &sent.lock().unwrap()[10..],
        &[vec![0xF0, 1, 0xF7], vec![0xF0, 1, 0xF7], vec![0xF0, 1, 0xF7]]
    );
}

#[test]
fn command_execute_overrides_and_trigger_both_dispatch_complete_frozen_full_notes() {
    use crate::app::module_command::{
        ModuleCommandDeliveryPolicy, ModuleCommandExecuteEvent, ModuleCommandParamOverride,
        MODULE_COMMAND_EXECUTE_TOPIC,
    };
    use golden_core::{
        edit::Edit, events::CustomEvent, node::Node, parameter::ParamValue, process_ctx::ExecutionPhase,
    };
    let pool = pool();
    let (mut engine, module, sent) = module_fixture(&pool);
    engine.add_node(crate::app::MidiSendFullNoteCommand::create().into(), Some(module));
    for _ in 0..6 {
        engine.apply_edits().unwrap();
        engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).unwrap();
    }
    let snapshot = engine.process_tree_snapshot();
    let command = snapshot
        .child_ids_slice(module)
        .iter()
        .copied()
        .find(|id| snapshot.node(*id).unwrap().node_type == crate::app::MidiSendFullNoteCommand::NODE_TYPE)
        .unwrap();
    let pitch = snapshot.find_child_by_decl_id(command, "pitch").unwrap();
    let duration = snapshot.find_child_by_decl_id(command, "duration_ms").unwrap();
    let trigger = snapshot.find_child_by_decl_id(command, "trigger").unwrap();
    engine.edits.push(Edit::SetParam {
        node: duration,
        value: ParamValue::Int(0),
        behaviour: Default::default(),
    });
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).unwrap();
    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new(
            MODULE_COMMAND_EXECUTE_TOPIC,
            Some(command),
            serde_json::to_value(ModuleCommandExecuteEvent {
                command_id: command,
                param_overrides: vec![ModuleCommandParamOverride {
                    param_id: pitch,
                    value: ParamValue::Int(43),
                }],
                invocation_id: None,
                delivery_policy: ModuleCommandDeliveryPolicy::Standard,
            })
            .unwrap(),
        ),
    });
    for _ in 0..5 {
        engine.apply_edits().unwrap();
        engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).unwrap();
    }
    until(|| sent.lock().unwrap().len() == 2);
    assert_eq!(&*sent.lock().unwrap(), &[vec![0x90, 43, 127], vec![0x80, 43, 0]]);
    assert_eq!(
        engine.nodes.get(pitch).unwrap().engine_param_snapshot().unwrap().value,
        ParamValue::Int(60)
    );
    engine.edits.push(Edit::SetParam {
        node: trigger,
        value: ParamValue::Trigger(),
        behaviour: golden_core::parameter::ParameterEventBehaviour::Append,
    });
    for _ in 0..5 {
        engine.apply_edits().unwrap();
        engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).unwrap();
    }
    until(|| sent.lock().unwrap().len() == 4);
    assert_eq!(&sent.lock().unwrap()[2..], &[vec![0x90, 60, 127], vec![0x80, 60, 0]]);
}
#[test]
fn a_late_invalid_packet_rejects_the_whole_batch_before_its_first_native_effect() {
    for invalid_deadline in [false, true] {
        let pool = pool();
        let (mut output, sent) = fake_output(&pool);
        let mut request = full_note(1);
        if invalid_deadline {
            request.packets[1].delay_ms = u64::MAX;
        } else {
            request.packets[1].bytes = vec![0; MESSAGE_BYTES + 1];
        }
        output.enqueue(MidiOutputRequest::for_test(request)).unwrap();
        output.finish_admission();
        let mut events = Vec::new();
        until(|| output.drain(&mut events).state == PendingDrainState::Disconnected);
        assert!(sent.lock().unwrap().is_empty());
        assert_eq!(
            events.iter().filter(|e| matches!(e, MidiOutputEvent::Error(_))).count(),
            1
        );
    }
}
#[test]
fn full_queue_rejects_a_full_note_as_one_request_without_sending_a_prefix() {
    let pool = pool();
    let gate = Gate::default();
    let _release = Release(gate.clone());
    let blocked = gate.clone();
    let (started, seen) = mpsc::channel();
    let sent = Arc::new(Mutex::new(Vec::new()));
    let copy = sent.clone();
    let mut output = MidiOutputHandle::spawn_using(output_config(), &pool, notify(), move |_| {
        Ok(FakeOutput {
            sent: copy,
            send_gate: Some((started, blocked)),
            drop_gate: None,
            fail: false,
        })
    })
    .unwrap();
    until(|| output.is_ready());
    output.send(&[0xF8], "block", false).unwrap();
    seen.recv_timeout(Duration::from_secs(2)).unwrap();
    for _ in 0..256 {
        output.send(&[0xF8], "queued", false).unwrap();
    }
    assert!(output
        .enqueue(MidiOutputRequest::for_test(full_note(20)))
        .unwrap_err()
        .contains("capacity"));
    output.finish_admission();
    gate.release();
    let mut events = Vec::new();
    until(|| output.drain(&mut events).state == PendingDrainState::Disconnected);
    let sent = sent.lock().unwrap();
    assert_eq!(sent.len(), 257);
    assert!(sent.iter().all(|b| b == &[0xF8]));
}
#[test]
fn due_followup_and_its_receipts_survive_a_full_reply_mailbox_without_replay() {
    let pool = pool();
    let (mut output, sent) = fake_output(&pool);
    for _ in 0..256 {
        output.send(&[0xF8], "fill", false).unwrap();
    }
    // Ready + 255 Sent fill the 256-slot mailbox; the next native effect
    // retains its reply while this complete full-note request stays admitted.
    until(|| sent.lock().unwrap().len() == 256);
    output.enqueue(MidiOutputRequest::for_test(full_note(20))).unwrap();
    std::thread::sleep(Duration::from_millis(30));
    output.finish_admission();
    let mut events = Vec::new();
    until(|| output.drain(&mut events).state == PendingDrainState::Disconnected);
    let sent = sent.lock().unwrap();
    assert_eq!(sent.len(), 258);
    assert_eq!(&sent[256..], &[vec![0x90, 60, 100], vec![0x80, 60, 0]]);
    assert_eq!(
        events.iter().filter(|e| matches!(e, MidiOutputEvent::Sent(_))).count(),
        258
    );
}
#[test]
fn delayed_packets_do_not_block_later_immediate_work_or_need_actor_ticks() {
    let pool = pool();
    let (mut output, sent) = fake_output(&pool);
    output.enqueue(MidiOutputRequest::for_test(full_note(200))).unwrap();
    until(|| sent.lock().unwrap().len() == 1);
    output.send(&[0xC0, 7], "program", false).unwrap();
    until(|| sent.lock().unwrap().len() == 3);
    assert_eq!(
        &*sent.lock().unwrap(),
        &[vec![0x90, 60, 100], vec![0xC0, 7], vec![0x80, 60, 0]]
    );
    output.finish_admission();
    let mut events = Vec::new();
    until(|| output.drain(&mut events).state == PendingDrainState::Disconnected);
}
#[test]
fn source_deletion_after_note_on_preserves_the_accepted_note_off_on_the_old_lifetime() {
    let pool = pool();
    let (mut output, sent) = fake_output(&pool);
    let engine = crate::app::AppEngine::new(Folder::new("producer").into());
    let producer = engine.node_waker(engine.root).unwrap();
    output
        .enqueue(MidiOutputRequest::for_test_source(full_note(200), producer.clone()))
        .unwrap();
    until(|| sent.lock().unwrap().len() == 1);
    drop(engine);
    assert!(producer.is_closed());
    output.finish_admission();
    let mut events = Vec::new();
    until(|| output.drain(&mut events).state == PendingDrainState::Disconnected);
    assert_eq!(&*sent.lock().unwrap(), &[vec![0x90, 60, 100], vec![0x80, 60, 0]]);
}
#[test]
fn source_retirement_before_preparation_cancels_the_complete_request() {
    let pool = pool();
    let gate = Gate::default();
    let _release = Release(gate.clone());
    let blocked = gate.clone();
    let (started, seen) = mpsc::channel();
    let sent = Arc::new(Mutex::new(Vec::new()));
    let copy = sent.clone();
    let mut output = MidiOutputHandle::spawn_using(output_config(), &pool, notify(), move |_| {
        Ok(FakeOutput {
            sent: copy,
            send_gate: Some((started, blocked)),
            drop_gate: None,
            fail: false,
        })
    })
    .unwrap();
    until(|| output.is_ready());
    output.send(&[0xF8], "block", false).unwrap();
    seen.recv_timeout(Duration::from_secs(2)).unwrap();
    let engine = crate::app::AppEngine::new(Folder::new("producer").into());
    output
        .enqueue(MidiOutputRequest::for_test_source(
            full_note(10),
            engine.node_waker(engine.root).unwrap(),
        ))
        .unwrap();
    drop(engine);
    gate.release();
    output.finish_admission();
    let mut events = Vec::new();
    until(|| output.drain(&mut events).state == PendingDrainState::Disconnected);
    assert_eq!(&*sent.lock().unwrap(), &[vec![0xF8]]);
    assert!(events
        .iter()
        .any(|e| matches!(e,MidiOutputEvent::Error(error) if error.contains("retired"))));
}
#[test]
fn delayed_admission_remains_charged_after_commands_leave_the_mailbox() {
    let pool = pool();
    let (output, sent) = fake_output(&pool);
    let mut admitted = 0;
    let deadline = Instant::now() + Duration::from_secs(10);
    while admitted < 2048 {
        let request = MidiOutputRequest::for_test(MidiSendRequest {
            packets: vec![packet(&[0xF8], 60_000)],
        });
        if output.enqueue(request).is_ok() {
            admitted += 1;
        } else {
            std::thread::yield_now();
        }
        assert!(Instant::now() < deadline);
    }
    let error = output.enqueue(MidiOutputRequest::for_test(full_note(0))).unwrap_err();
    assert!(error.contains("2048"), "{error}");
    assert!(sent.lock().unwrap().is_empty());
    let start = Instant::now();
    drop(output);
    assert!(start.elapsed() < Duration::from_millis(100));
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
