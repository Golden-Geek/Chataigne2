use super::*;

fn port(index: usize, label: &str) -> DiscoveredMidiPort {
    DiscoveredMidiPort {
        variant_id: midi_port_variant_id(index, label),
        label: label.into(),
        details: String::new(),
    }
}

#[test]
fn native_inventory_is_admitted_before_name_conversion() {
    let mut calls = 0;
    let result = describe_ports(vec![0u8; 65], |_| {
        calls += 1;
        Ok::<_, String>("fake".into())
    });
    assert!(result.is_err());
    assert_eq!(calls, 0);
    assert!(describe_ports(vec![0u8], |_| Ok::<_, String>("x".repeat(513))).is_err());
    let ports = describe_ports(vec![0u8], |_| Ok::<_, String>("x".repeat(512))).unwrap();
    assert_eq!(ports.len(), 1);
}

#[test]
fn availability_matches_connection_resolution_without_retargeting_reindexed_name() {
    let ports = || Ok(vec![port(0, "same")]);
    assert!(!midi_port_available("1|same", ports).unwrap());
    assert!(resolve_port(vec![0u8], "1|same", |_, _| Ok("same".into())).is_err());
    assert!(midi_port_available("0|same", ports).unwrap());
    assert!(resolve_port(vec![0u8], "0|same", |_, _| Ok("same".into()))
        .unwrap()
        .is_some());
    assert!(midi_port_available("same", ports).unwrap());
    assert!(resolve_port(vec![0u8], "same", |_, _| Ok("same".into()))
        .unwrap()
        .is_some());
}
