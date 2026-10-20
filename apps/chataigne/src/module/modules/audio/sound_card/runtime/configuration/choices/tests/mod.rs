use super::*;
use crate::app::module_modules_audio_sound_card::{runtime::observation::fixture_inventory, tests as sound_tests};
use golden_audio::{AudioBackend, AudioDeviceCatalogEntry, AudioDeviceInspectorState, NullBackend};
use golden_core::edit::Edit;

fn inventory() -> DeviceInventory {
    let devices = NullBackend.discover().unwrap();
    let device_catalog = devices.iter().map(AudioDeviceCatalogEntry::from).collect();
    fixture_inventory(
        AudioDeviceInspectorState {
            devices,
            device_catalog,
            ..Default::default()
        },
        7,
    )
}

#[test]
fn prepared_choices_move_once_and_exported_constraints_keep_native_option_quota() {
    let (mut engine, module) = sound_tests::create_sound_card();
    let pool = PayloadPool::new("choice-plan-test", 16, 128 * 1024 * 1024).unwrap();
    let plan = ChoicePlan::prepare(&engine.process_tree_snapshot(), module, &inventory(), &pool).unwrap();
    assert_eq!(plan.device_revision, 7);
    assert!(plan.bytes <= MAX_BYTES);
    let patches = plan.take_patches().unwrap();
    assert!(!patches.is_empty());
    assert!(patches.len() <= MAX_PATCHES);
    assert!(plan.take_patches().is_none());
    let node = patches[0].node;
    let canonical = patches[0].constraints.enum_options.clone();
    for patch in patches {
        engine.edits.push(Edit::SetParamConstraints {
            node: patch.node,
            constraints: patch.constraints,
        });
    }
    engine.apply_edits().unwrap();
    let current = engine.process_tree_snapshot();
    assert!(current
        .node(node)
        .unwrap()
        .param_constraints
        .as_ref()
        .unwrap()
        .enum_options
        .same_storage(&canonical));
    let exported = current.node(node).unwrap().param_constraints.clone().unwrap();
    drop((engine, current, canonical, plan));
    assert!(
        !pool.wait_for_idle(std::time::Duration::from_millis(10)),
        "exported constraints retain canonical native admission"
    );
    drop(exported);
    assert!(pool.wait_for_idle(std::time::Duration::from_secs(3)));
}

#[test]
fn native_comparison_omits_already_applied_domains() {
    let (mut engine, module) = sound_tests::create_sound_card();
    let inventory = inventory();
    let pool = PayloadPool::new("choice-diff-test", 16, 128 * 1024 * 1024).unwrap();
    let plan = ChoicePlan::prepare(&engine.process_tree_snapshot(), module, &inventory, &pool).unwrap();
    for patch in plan.take_patches().unwrap() {
        engine.edits.push(Edit::SetParamConstraints {
            node: patch.node,
            constraints: patch.constraints,
        });
    }
    engine.apply_edits().unwrap();
    let unchanged = ChoicePlan::prepare(&engine.process_tree_snapshot(), module, &inventory, &pool).unwrap();
    assert!(unchanged.take_patches().unwrap().is_empty());
}

#[test]
fn numeric_domains_preserve_empty_authored_values_without_a_publication_retry_loop() {
    use golden_core::parameter::ParameterEventBehaviour;
    let (mut engine, module) = sound_tests::create_sound_card();
    let node = find_path(&engine.process_tree_snapshot(), module, "connection/sample_rate").unwrap();
    let mut constraints = engine
        .process_tree_snapshot()
        .node(node)
        .unwrap()
        .param_constraints
        .clone()
        .unwrap();
    constraints.enum_options = Default::default();
    engine.edits.push(Edit::SetParamConstraints { node, constraints });
    engine.edits.push(Edit::SetParam {
        node,
        value: ParamValue::Enum(String::new()),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    let pool = PayloadPool::new("choice-empty-value-test", 16, 128 * 1024 * 1024).unwrap();
    let plan = ChoicePlan::prepare(&engine.process_tree_snapshot(), module, &inventory(), &pool).unwrap();
    let patches = plan.take_patches().unwrap();
    let patch = patches.iter().find(|patch| patch.node == node).unwrap();
    assert!(patch
        .constraints
        .enum_options
        .iter()
        .any(|option| option.variant_id.is_empty() && option.tags.iter().any(|tag| tag == "missing")));
    for patch in patches {
        engine.edits.push(Edit::SetParamConstraints {
            node: patch.node,
            constraints: patch.constraints,
        });
    }
    engine.apply_edits().unwrap();
    assert_eq!(
        engine.process_tree_snapshot().node(node).unwrap().param_value,
        Some(ParamValue::Enum(String::new()))
    );
}
