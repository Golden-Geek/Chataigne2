use super::super::replies::{NativeReplies, ReplySource};
use super::*;

#[test]
fn pending_reply_sources_resume_in_one_fifo_and_retirement_delivers_every_result() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    let (resumed_tx, resumed_rx) = mpsc::channel();
    let (finish_tx, finish_rx) = mpsc::channel();
    let worker = BoundedWorker::<UdpOutboundSend, StreamingWorkerEvent>::spawn(
        "udp-reply-order",
        &pool,
        WorkerLimits {
            command_items: 1,
            command_weight: UDP_COMMAND_BYTES,
            event_items: 1,
            event_weight: UDP_EVENT_BYTES,
        },
        move |context| {
            let mut replies = NativeReplies::default();
            for (source, text) in [
                (ReplySource::Input, "first"),
                (ReplySource::Input, "second"),
                (ReplySource::Output, "third"),
                (ReplySource::Interfaces, "fourth"),
            ] {
                assert!(replies.offer(&context, source, StreamingWorkerEvent::Error(text.into())));
            }
            for source in [ReplySource::Input, ReplySource::Output, ReplySource::Interfaces] {
                assert!(!replies.permits(source));
            }
            entered_tx.send(()).unwrap();
            resume_rx.recv().unwrap();
            assert!(replies.flush(&context));
            assert!(replies.permits(ReplySource::Input));
            assert!(!replies.permits(ReplySource::Output));
            assert!(!replies.permits(ReplySource::Interfaces));
            assert!(replies.offer(
                &context,
                ReplySource::Input,
                StreamingWorkerEvent::Error("fifth".into())
            ));
            resumed_tx.send(()).unwrap();
            finish_rx.recv().unwrap();
            assert!(replies.finish(&context));
        },
    )
    .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(worker.event_metrics().depth, 1);
    let mut events = Vec::new();
    worker.drain_events(&mut events, NonZeroUsize::MIN);
    resume_tx.send(()).unwrap();
    resumed_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    finish_tx.send(()).unwrap();
    wait_for_disconnect(&worker, &mut events);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    let texts: Vec<_> = events
        .into_iter()
        .map(|event| match event {
            StreamingWorkerEvent::Error(text) => text,
            other => panic!("unexpected reply {other:?}"),
        })
        .collect();
    assert_eq!(texts, ["first", "second", "third", "fourth", "fifth"]);
}

fn wait_for_disconnect(
    worker: &BoundedWorker<UdpOutboundSend, StreamingWorkerEvent>,
    events: &mut Vec<StreamingWorkerEvent>,
) {
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while worker.drain_events(events, NonZeroUsize::MIN).state != PendingDrainState::Disconnected {
        assert!(std::time::Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn oversized_output_details_become_one_explicit_failure_instead_of_stranding_later_replies() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let worker = BoundedWorker::<UdpOutboundSend, StreamingWorkerEvent>::spawn(
        "udp-oversized-reply",
        &pool,
        WorkerLimits {
            command_items: 1,
            command_weight: UDP_COMMAND_BYTES,
            event_items: 1,
            event_weight: UDP_EVENT_BYTES,
        },
        move |context| {
            let mut replies = NativeReplies::default();
            assert!(replies.offer(
                &context,
                ReplySource::Output,
                StreamingWorkerEvent::OutgoingLog("x".repeat(128 * 1024))
            ));
            assert!(replies.offer(&context, ReplySource::Input, StreamingWorkerEvent::Error("next".into())));
            assert!(replies.finish(&context));
        },
    )
    .unwrap();
    let mut events = Vec::new();
    wait_for_disconnect(&worker, &mut events);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert_eq!(events.len(), 2);
    assert!(
        matches!(&events[0], StreamingWorkerEvent::Error(error) if error.contains("limit") && error.contains("not replayed"))
    );
    assert!(matches!(&events[1], StreamingWorkerEvent::Error(error) if error == "next"));
}
