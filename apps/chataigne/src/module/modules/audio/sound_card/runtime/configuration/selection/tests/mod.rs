use super::*;
mod fixtures;
use fixtures::*;

#[test]
fn device_selection_without_endpoints_needs_no_physical_inventory() {
    let (engine, module) = source("null", "none", "none");
    let pool = PayloadPool::new("selection-test", 8, 16 * 1024 * 1024).unwrap();
    let plan = SelectionPlan::prepare(
        &engine.process_tree_snapshot(),
        module,
        &AudioDeviceInspectorState::default(),
        4,
        None,
        &pool,
    )
    .unwrap();
    assert_eq!(plan.device_revision, 4);
    assert!(!plan.pending);
    assert!(plan.input_physical.is_empty());
    assert!(plan.output_physical.is_empty());
}

#[test]
fn catalog_pending_plan_reuses_canonical_targets_when_capabilities_arrive() {
    let device = device();
    let value = selected(&device);
    let (engine, module) = source("null", "none", &value);
    let mut inspector = AudioDeviceInspectorState {
        device_catalog: vec![golden_audio::AudioDeviceCatalogEntry::from(&device)],
        ..Default::default()
    };
    let pool = PayloadPool::new("selection-catalog-test", 8, 16 * 1024 * 1024).unwrap();
    let pending = SelectionPlan::prepare(&engine.process_tree_snapshot(), module, &inspector, 1, None, &pool).unwrap();
    assert!(pending.pending);
    assert!(pending.output_physical.is_empty());
    inspector.devices.push(device.clone());
    let ready = SelectionPlan::prepare(
        &engine.process_tree_snapshot(),
        module,
        &inspector,
        2,
        Some(&pending.interests()),
        &pool,
    )
    .unwrap();
    assert!(!ready.pending);
    assert!(ready.same_interests(&pending.interests()));
    assert_eq!(
        ready.output_physical,
        device
            .output_channels
            .iter()
            .map(|channel| channel.key.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn exclusive_driver_selection_plans_disable_before_probing_another_device() {
    let mut device = device();
    let backend = BackendId::from_static("asio");
    device.target = AudioDeviceTargetId::Device {
        backend: backend.clone(),
        device: golden_audio::AudioDeviceId::new("selected").unwrap(),
    };
    let (engine, module) = source("asio", "none", &selected(&device));
    let mut inspector = AudioDeviceInspectorState::default();
    inspector.output.active_target = Some(AudioDeviceTargetId::Device {
        backend,
        device: golden_audio::AudioDeviceId::new("previous").unwrap(),
    });
    let pool = PayloadPool::new("selection-exclusive-test", 8, 16 * 1024 * 1024).unwrap();
    let plan = SelectionPlan::prepare(&engine.process_tree_snapshot(), module, &inspector, 1, None, &pool).unwrap();
    assert!(plan.disable_before_probe);
    assert_eq!(plan.targets.0.arc().len(), 1);
}

#[test]
fn probe_command_arc_keeps_canonical_storage_charged_after_plan_retirement() {
    let device = device();
    let (engine, module) = source("null", "none", &selected(&device));
    let pool = PayloadPool::new("selection-raw-arc-test", 1, 16 * 1024 * 1024).unwrap();
    let plan = SelectionPlan::prepare(
        &engine.process_tree_snapshot(),
        module,
        &AudioDeviceInspectorState::default(),
        1,
        None,
        &pool,
    )
    .unwrap();
    let command_owner = plan.targets.0.arc().clone();
    drop(plan);
    assert!(pool.try_reserve(1).is_err());
    drop(command_owner);
    assert!(pool.wait_for_idle(std::time::Duration::from_secs(2)));
    assert!(pool.try_reserve(1).is_ok());
}

#[test]
fn probe_target_capacity_failure_is_retryable_before_actor_admission() {
    let device = device();
    let (engine, module) = source("null", "none", &selected(&device));
    let pool = PayloadPool::new("selection-full-test", 1, 1).unwrap();
    let error = SelectionPlan::prepare(
        &engine.process_tree_snapshot(),
        module,
        &AudioDeviceInspectorState::default(),
        1,
        None,
        &pool,
    )
    .unwrap_err();
    assert!(error.retryable);
    assert!(error.message.contains("probe-target lifetime capacity"));
}
