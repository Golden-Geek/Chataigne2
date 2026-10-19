use super::*;

#[test]
fn large_destination_plan_retains_one_datagram_and_accounts_for_allocated_capacity() {
    let bytes = vec![9; UDP_MAX_PAYLOAD];
    let address = bytes.as_ptr();
    let mut command = UdpOutboundSend::new(bytes, false).unwrap();
    for _ in 0..10_000 {
        command
            .push_target(UdpOutputTarget {
                remote_host: "127.0.0.1".into(),
                remote_port: 9000,
            })
            .unwrap();
    }
    assert_eq!(command.len(), 10_000);
    assert_eq!(command.bytes().as_ptr(), address);
    let expected = size_of::<UdpOutboundSend>()
        + command.bytes.capacity()
        + command.targets.capacity() * size_of::<UdpOutputTarget>()
        + command
            .targets
            .iter()
            .map(|target| target.remote_host.capacity())
            .sum::<usize>();
    assert_eq!(command.admission_weight().unwrap(), expected);
    assert!(expected < UDP_COMMAND_BYTES);
}

#[test]
fn oversized_or_invalid_destination_plan_cannot_admit_its_valid_prefix() {
    let mut command = UdpOutboundSend::new(vec![1], false).unwrap();
    loop {
        let target = UdpOutputTarget {
            remote_host: "x".repeat(1024),
            remote_port: 1,
        };
        if command.push_target(target).is_err() {
            break;
        }
    }
    assert!(command.len() > 0);
    assert!(command.admission_weight().is_err());
    assert!(command
        .push_target(UdpOutputTarget {
            remote_host: "127.0.0.1".into(),
            remote_port: 1
        })
        .is_err());
    assert!(UdpOutboundSend::new(Vec::with_capacity(UDP_COMMAND_BYTES), false).is_err());
}
