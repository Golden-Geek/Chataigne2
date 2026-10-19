use super::*;

fn port() -> DiscoveredSerialPort {
    DiscoveredSerialPort {
        port_name: "fixture".into(),
        label: "USB Fixture".into(),
        tags: vec!["usb".into()],
        details: "test metadata".into(),
    }
}
#[test]
fn catalog_bounds_include_spare_capacity_and_every_retained_field() {
    let good = SerialCatalog::prepare(vec![port()]).unwrap();
    assert_eq!(good.options.len(), 2);
    assert_eq!(good.options[1].variant_id, "fixture");
    let too_many = Vec::with_capacity(129);
    assert!(SerialCatalog::prepare(too_many).is_err());
    for field in ["name", "label", "details", "tags", "tag", "empty", "nul", "reserved"] {
        let mut value = port();
        match field {
            "name" => value.port_name.reserve(1025),
            "label" => value.label.reserve(513),
            "details" => value.details.reserve(4097),
            "tags" => value.tags.reserve(9),
            "tag" => value.tags[0].reserve(65),
            "empty" => value.port_name.clear(),
            "nul" => value.port_name.push('\0'),
            "reserved" => value.port_name = "none".into(),
            _ => unreachable!(),
        }
        assert!(SerialCatalog::prepare(vec![value]).is_err(), "{field}");
    }
}
#[test]
fn unchanged_catalog_reuses_its_lease_and_bad_metadata_keeps_the_last_good_options() {
    let service = Service::new(|| panic!("manual publisher"));
    let notified = Arc::new(AtomicUsize::new(0));
    let count = notified.clone();
    let reentrant = Arc::downgrade(&service);
    service.state.lock().unwrap().subscribers.insert(
        1,
        WorkerNotifier::new(move || {
            let service = reentrant.upgrade().unwrap();
            let _header = service.state.lock().unwrap().snapshot.clone();
            count.fetch_add(1, Ordering::AcqRel);
        }),
    );
    service.publish(0, vec![port()], None);
    let good = service.state.lock().unwrap().snapshot.clone();
    service.publish(0, vec![port()], None);
    assert_eq!(notified.load(Ordering::Acquire), 1);
    assert_eq!(service.state.lock().unwrap().snapshot.version, good.version);
    service.publish(0, Vec::with_capacity(129), None);
    let bad = service.state.lock().unwrap().snapshot.clone();
    assert!(bad.error.as_ref().unwrap().contains("128"));
    assert!(std::ptr::eq(
        &**bad.catalog.as_ref().unwrap(),
        &**good.catalog.as_ref().unwrap()
    ));
    service.publish(0, vec![port()], None);
    assert!(service.state.lock().unwrap().snapshot.error.is_none());
    assert_eq!(notified.load(Ordering::Acquire), 3);
    service.publish(9, vec![], Some("stale".into()));
    assert_eq!(notified.load(Ordering::Acquire), 3);
}
