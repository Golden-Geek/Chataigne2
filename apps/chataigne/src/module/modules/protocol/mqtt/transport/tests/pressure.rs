use super::super::super::output::{Method, Request};
use super::*;
use golden_core::{node::Folder, parameter::ParamValue, script::ScriptMethodCall};

#[test]
fn captured_command_bytes_fill_the_mailbox_before_its_item_limit_and_cancel_without_replay() {
    let engine = crate::app::AppEngine::new(Folder::new("producer").into());
    let producer = engine.node_waker(engine.root).unwrap();
    let control = Control::default();
    control.busy.store(true, Ordering::Release);
    let lifetimes = slots();
    let output = PayloadPool::new("mqtt-byte-pressure", 256, 64 * 1024 * 1024).unwrap();
    let mut handle = fake(control.clone(), &lifetimes, output.clone(), pool(8));
    let mut receipts = Vec::new();
    let refusal = loop {
        let call = ScriptMethodCall::prepare(
            "publishText",
            vec![
                ParamValue::Str("test/value".into()),
                ParamValue::Str("x".repeat(96 * 1024)),
                ParamValue::Int(1),
            ],
        )
        .unwrap();
        let request = Request::script_arguments(Method::Text, call.args().clone(), producer.clone()).unwrap();
        match handle.send_output(request, false) {
            Ok(receipt) => receipts.push(receipt),
            Err(error) => break error,
        }
        assert!(receipts.len() <= COMMAND_ITEMS + 8);
    };
    assert_eq!(refusal, "MQTT request queue is full");
    assert!(
        !receipts.is_empty() && receipts.len() < COMMAND_ITEMS,
        "bytes must refuse before the item ceiling"
    );
    assert_eq!(control.published.load(Ordering::Acquire), 0);
    handle.stop();
    drop(handle);
    until(|| {
        receipts
            .iter()
            .all(|receipt| receipt.status() == PublishStatus::FailedUnconfirmed)
    });
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
    assert!(output.wait_for_idle(Duration::from_secs(1)));
    // The same native slot is usable after pressure cancellation.
    let mut recovered = fake(Control::default(), &lifetimes, pool(8), pool(8));
    recovered.stop();
    drop(recovered);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn continuous_native_input_blocks_at_event_capacity_and_stop_releases_the_same_slot() {
    let control = Control::default();
    control.incoming.store(1024, Ordering::Release);
    let lifetimes = slots();
    let input = pool(256);
    let mut handle = fake(control.clone(), &lifetimes, pool(8), input.clone());
    until(|| handle.worker.event_metrics().depth == EVENT_ITEMS);
    let metrics = handle.worker.event_metrics();
    assert!(metrics.retained_weight <= 4 * 1024 * 1024);
    assert!(control.incoming.load(Ordering::Acquire) > 0);
    handle.stop();
    drop(handle);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
    assert!(input.wait_for_idle(Duration::from_secs(1)));
    let mut recovered = fake(Control::default(), &lifetimes, pool(8), input.clone());
    recovered.stop();
    drop(recovered);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
    assert!(input.wait_for_idle(Duration::from_secs(1)));
}
