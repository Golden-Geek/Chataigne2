use super::*;

#[test]
fn physical_keys_preserve_the_exact_device_and_validate_persistence() {
    let key = PhysicalAdapterId::from_key("pnp:pci\\ven_10de&dev_2c59\\instance-a").unwrap();
    assert_eq!(key.key(), "pnp:PCI\\VEN_10DE&DEV_2C59\\INSTANCE-A");
    assert_ne!(
        key,
        PhysicalAdapterId::from_key("pnp:PCI\\VEN_10DE&DEV_2C59\\INSTANCE-B").unwrap()
    );
    let json = serde_json::to_string(&key).unwrap();
    assert_eq!(serde_json::from_str::<PhysicalAdapterId>(&json).unwrap(), key);
    for invalid in [
        "NVIDIA",
        "dxgi:00000000:00000001",
        "pnp:*",
        "pnp:PCI\\X\\",
        "pnp:PCI\\X\\*",
        "pnp:PCI\\X\\A\0",
    ] {
        assert!(PhysicalAdapterId::from_key(invalid).is_none());
        assert!(serde_json::from_value::<PhysicalAdapterId>(serde_json::json!(invalid)).is_err());
    }
}

#[test]
#[ignore = "read-only Windows hardware identity mapping, no rendering device or reset"]
fn hardware_runtime_ids_resolve_to_distinct_physical_devices() {
    let adapters = crate::native::adapters().unwrap();
    let mut found = std::collections::HashSet::new();
    for adapter in adapters.iter().filter(|adapter| !adapter.software) {
        let key = physical_id(adapter.id).unwrap();
        eprintln!("physical GPU {:?} {} -> {}", adapter.id, adapter.description, key.key());
        assert!(found.insert(key));
    }
    assert_eq!(found.len(), 2, "qualification machine requires both hardware GPUs");
}
