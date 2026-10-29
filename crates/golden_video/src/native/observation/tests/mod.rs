use uuid::Uuid;
use windows::core::HRESULT;

use super::*;

mod notifier;

fn output(id: u64) -> OutputId {
    OutputId::new(id).unwrap()
}
fn token(device: DeviceGeneration, serial: u64) -> CompletionToken {
    CompletionToken { device, serial }
}

#[test]
fn fixed_output_acceptance_is_consumer_and_device_specific_and_ignores_backpressure() {
    let control = D3d11Control::default();
    let device = DeviceGeneration::new(Uuid::new_v4(), 1);
    control.register_outputs(device, [output(1), output(2)].into_iter());
    assert_eq!(
        control.output_snapshots(),
        [
            OutputSnapshot {
                id: output(1),
                ..OutputSnapshot::starting(output(1))
            },
            OutputSnapshot {
                id: output(2),
                ..OutputSnapshot::starting(output(2))
            },
        ]
    );
    control.observe_output(output(2), token(device, 1), Delivery::Accepted);
    for delivery in [
        Delivery::Busy,
        Delivery::Unavailable,
        Delivery::Failed(HRESULT(0x80004005_u32 as i32)),
    ] {
        control.observe_output(output(1), token(device, 1), delivery);
    }
    assert_eq!(
        control.output_snapshots(),
        [
            OutputSnapshot {
                id: output(1),
                ..OutputSnapshot::starting(output(1))
            },
            OutputSnapshot {
                id: output(2),
                last_accepted: Some(token(device, 1)),
                ..OutputSnapshot::starting(output(2))
            },
        ]
    );
    control.observe_output(output(1), token(device, 2), Delivery::Accepted);
    control.observe_output(output(1), token(device, 3), Delivery::Busy);
    control.observe_output(output(99), token(device, 3), Delivery::Accepted);
    assert_eq!(
        control.output_snapshots(),
        [
            OutputSnapshot {
                id: output(1),
                last_accepted: Some(token(device, 2)),
                ..OutputSnapshot::starting(output(1))
            },
            OutputSnapshot {
                id: output(2),
                last_accepted: Some(token(device, 1)),
                ..OutputSnapshot::starting(output(2))
            },
        ],
        "an unknown consumer cannot grow the fixed registry or advance another consumer"
    );
    let replacement = device.next().unwrap();
    control.register_outputs(replacement, [output(1), output(2)].into_iter());
    control.observe_output(output(1), token(device, 99), Delivery::Accepted);
    assert!(
        control
            .output_snapshots()
            .iter()
            .all(|output| output.last_accepted.is_none())
    );
    control.observe_output(output(1), token(replacement, 1), Delivery::Accepted);
    assert_eq!(control.output_snapshots()[0].last_accepted, Some(token(replacement, 1)));
    control.clear_outputs();
    control.observe_output(output(1), token(replacement, 2), Delivery::Accepted);
    assert!(
        control.output_snapshots().is_empty(),
        "retirement must remove acceptance readiness"
    );
}
