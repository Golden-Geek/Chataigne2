use super::*;

#[test]
fn large_destination_plan_retains_one_datagram_and_accounts_for_allocated_capacity() {
    let bytes = vec![9; UDP_MAX_PAYLOAD];
    let address = bytes.as_ptr();
    let mut builder = OutputPlanBuilder::default();
    for _ in 0..10_000 {
        builder
            .push_target(UdpOutputTarget {
                remote_host: "127.0.0.1".into(),
                remote_port: 9000,
            })
            .unwrap();
    }
    let plan = builder.finish().unwrap();
    let command = UdpOutboundSend::new(bytes, plan.clone(), false).unwrap();
    assert_eq!(command.len(), 10_000);
    assert_eq!(command.bytes().as_ptr(), address);
    let expected = TICKET_HEAP_BYTES
        + size_of::<UdpOutboundSend>()
        + command.bytes.weight()
        + size_of::<DestinationPlan>()
        + plan.0.targets.capacity() * size_of::<UdpOutputTarget>()
        + plan
            .0
            .targets
            .iter()
            .map(|target| target.remote_host.capacity())
            .sum::<usize>();
    assert_eq!(command.weight(), expected);
    assert!(expected < UDP_COMMAND_BYTES);
    let another = UdpOutboundSend::new(vec![1], plan.clone(), false).unwrap();
    assert!(std::ptr::eq(
        &*command.plan.as_ref().unwrap().0,
        &*another.plan.as_ref().unwrap().0
    ));
}

#[test]
fn oversized_or_invalid_destination_plan_cannot_admit_its_valid_prefix() {
    let mut builder = OutputPlanBuilder::default();
    loop {
        let target = UdpOutputTarget {
            remote_host: "x".repeat(1024),
            remote_port: 1,
        };
        if builder.push_target(target).is_err() {
            break;
        }
    }
    assert!(!builder.targets.is_empty());
    assert!(builder
        .push_target(UdpOutputTarget {
            remote_host: "127.0.0.1".into(),
            remote_port: 1
        })
        .is_err());
    assert!(builder.finish().is_err());
    let mut valid = OutputPlanBuilder::default();
    valid
        .push_target(UdpOutputTarget {
            remote_host: "127.0.0.1".into(),
            remote_port: 1,
        })
        .unwrap();
    let error =
        UdpOutboundSend::new(Vec::with_capacity(UDP_COMMAND_BYTES), valid.finish().unwrap(), false).unwrap_err();
    assert!(
        error.contains("retained-byte limit"),
        "allocated datagram capacity must contribute even when its length is zero"
    );
}
