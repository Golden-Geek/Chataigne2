use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};

use super::*;

#[test]
fn output_notifications_coalesce_backpressure_and_frame_serials_but_wake_first_readiness() {
    let control = D3d11Control::default();
    let device = DeviceGeneration::new(Uuid::new_v4(), 1);
    control.register_outputs(device, [output(1)].into_iter());
    let wakes = Arc::new(AtomicUsize::new(0));
    let observed = wakes.clone();
    let outputs = Arc::downgrade(&control.outputs);
    control.set_output_notifier(golden_io::WorkerNotifier::new(move || {
        // A notification consumer can read immediately: the registry lock is not retained.
        let registry = Weak::upgrade(&outputs).unwrap();
        assert!(registry.try_lock().is_ok());
        observed.fetch_add(1, Ordering::SeqCst);
    }));
    let publish = |state, generation, attempts, hresult| {
        control.output_state(
            output(1),
            device,
            OutputStatus {
                state,
                generation,
                attempts,
                hresult,
            },
        )
    };
    publish(OutputState::Ready, 1, 1, None);
    assert_eq!(wakes.load(Ordering::SeqCst), 1);
    control.observe_output(output(1), token(device, 1), Delivery::Accepted);
    assert_eq!(
        wakes.load(Ordering::SeqCst),
        2,
        "first actual readiness wakes consumers"
    );
    for serial in 2..20 {
        publish(OutputState::Busy, 1, 1, None);
        publish(OutputState::Ready, 1, 1, None);
        control.observe_output(output(1), token(device, serial), Delivery::Accepted);
    }
    publish(OutputState::Ready, 1, 2, None);
    assert_eq!(wakes.load(Ordering::SeqCst), 2);
    publish(OutputState::Unavailable, 1, 2, None);
    assert_eq!(wakes.load(Ordering::SeqCst), 3);
    publish(OutputState::Recovering, 1, 2, Some(-1));
    assert_eq!(wakes.load(Ordering::SeqCst), 4);
    assert_eq!(control.output_snapshots()[0].last_accepted, None);
    publish(OutputState::Ready, 2, 3, None);
    control.observe_output(output(1), token(device, 20), Delivery::Accepted);
    assert_eq!(wakes.load(Ordering::SeqCst), 6);
    control.clear_outputs();
    control.clear_outputs();
    assert_eq!(wakes.load(Ordering::SeqCst), 7);
}
