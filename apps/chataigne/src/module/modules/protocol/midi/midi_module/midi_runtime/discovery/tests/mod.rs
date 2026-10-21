use super::*;

fn raw(count: usize) -> MidiPortOptions {
    MidiPortOptions {
        inputs: (0..count)
            .map(|n| DiscoveredMidiPort {
                variant_id: format!("{n}|port"),
                label: format!("port {n}"),
                details: String::new(),
            })
            .collect(),
        outputs: vec![],
    }
}
fn pool(items: usize) -> PayloadPool {
    PayloadPool::new("midi-catalog-test", items, 1024 * 1024).unwrap()
}

#[test]
fn maximum_inventory_preserves_missing_selection_within_measured_bounds() {
    let pool = pool(4);
    let selected = "x".repeat(1024);
    let catalog = PreparedCatalog::prepare(raw(64), &Selections::new(&selected, "none").unwrap(), None, &pool).unwrap();
    assert_eq!(catalog.input.options.len(), 66);
    assert_eq!(catalog.output.options.len(), 1);
    let missing = catalog.input.options.iter().find(|o| o.variant_id == selected).unwrap();
    assert_eq!(missing.value, golden_core::parameter::ParamValue::Enum(selected));
    assert!(missing.tags.iter().any(|tag| tag == "missing"));
    assert!(catalog.input.options.retained_bytes() < 256 * 1024);
    assert!(catalog.weight() < 1024 * 1024);
}

#[test]
fn admission_rejects_spare_capacity_duplicates_and_oversized_selections() {
    let pool = pool(8);
    let selected = Selections::new("none", "none").unwrap();
    let mut source = raw(0);
    source.inputs = Vec::with_capacity(65);
    assert!(PreparedCatalog::prepare(source, &selected, None, &pool).is_err());
    let mut source = raw(1);
    source.inputs[0].label = String::with_capacity(513);
    assert!(PreparedCatalog::prepare(source, &selected, None, &pool).is_err());
    let mut source = raw(2);
    source.inputs[1].variant_id = source.inputs[0].variant_id.clone();
    assert!(PreparedCatalog::prepare(source, &selected, None, &pool).is_err());
    assert!(Selections::new(&"x".repeat(1025), "none").is_err());
}

#[test]
fn canonical_options_reuse_identity_and_hold_capacity_through_snapshot_owners() {
    let pool = pool(2);
    let selected = Selections::new("none", "none").unwrap();
    let first = PreparedCatalog::prepare(raw(1), &selected, None, &pool).unwrap();
    let second = PreparedCatalog::prepare(raw(1), &selected, Some(&first), &pool).unwrap();
    assert!(Arc::ptr_eq(first.input.options.shared(), second.input.options.shared()));
    assert!(Arc::ptr_eq(
        first.output.options.shared(),
        second.output.options.shared()
    ));
    assert!(PreparedCatalog::prepare(raw(2), &selected, Some(&first), &pool).is_err());
    let snapshot = (first.input.options.clone(), first.output.options.clone());
    drop(first);
    drop(second);
    assert!(pool.try_reserve(1).is_err());
    drop(snapshot);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert!(PreparedCatalog::prepare(raw(2), &selected, None, &pool).is_ok());
}

#[test]
fn selection_changes_are_prepared_by_generation_even_with_a_full_result_slot() {
    let lifetimes = RetirementPool::new(NonZeroUsize::MIN);
    let discovery =
        MidiDiscovery::spawn_using(&lifetimes, WorkerNotifier::new(|| {}), "none", "none", || Ok(raw(1))).unwrap();
    let until = Instant::now() + Duration::from_secs(2);
    while !discovery.has_pending() {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
    discovery.select("new|selected", "other|selected").unwrap();
    let generation = discovery.generation();
    let until = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(event) = discovery.take().0 {
            if event.generation == generation {
                let catalog = event.result.unwrap();
                assert_eq!(catalog.input.selection, "new|selected");
                assert_eq!(catalog.output.selection, "other|selected");
                assert!(catalog.input.options.iter().any(|p| p.variant_id == "new|selected"));
                break;
            }
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
    drop(discovery);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(2)));
}
