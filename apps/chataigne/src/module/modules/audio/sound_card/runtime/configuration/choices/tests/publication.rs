use super::*;
use golden_audio::{AudioDeviceId, AudioDeviceTargetId};
use std::time::Instant;

#[test]
#[ignore = "explicit large native choice publication measurement"]
fn capped_native_catalog_publication_samples() {
    let (mut engine, module) = sound_tests::create_sound_card();
    sound_tests::select_enum_parameter(
        &mut engine,
        module,
        "connection/audio_driver",
        NullBackend::backend_id().as_str(),
    );
    let mut metadata = AudioDeviceInspectorState {
        device_catalog: (0..32_766)
            .map(|index| AudioDeviceCatalogEntry {
                target: AudioDeviceTargetId::Device {
                    backend: NullBackend::backend_id(),
                    device: AudioDeviceId::new(format!("device-{index:05}")).unwrap(),
                },
                label: format!("Device {index:05} with a catalog label"),
            })
            .collect(),
        ..Default::default()
    };
    let pool = PayloadPool::new("choice-publication-scale-test", 512, 512 * 1024 * 1024).unwrap();
    let mut samples = Vec::new();
    for sample in 0..9 {
        metadata.device_catalog.last_mut().unwrap().label = format!("Last device {sample}");
        // Uses the real inventory intake, including its 8 MiB metadata limit.
        let (inventory, _source) =
            crate::app::module_modules_audio_sound_card::runtime::observation::fixture_inventory_with_source(
                metadata.clone(),
                7,
            );
        let plan = ChoicePlan::prepare(&engine.process_tree_snapshot(), module, &inventory, &pool).unwrap();
        assert!(plan.bytes <= MAX_BYTES);
        let publication = plan.take_publication().unwrap();
        assert!(publication.patches.len() <= MAX_PATCHES);
        assert_eq!(
            publication
                .patches
                .iter()
                .filter(|patch| patch.constraints.enum_options.len() == 32_768)
                .count(),
            2
        );
        engine.edits.push(Edit::CheckPreconditions {
            preconditions: publication.preconditions,
        });
        for patch in publication.patches {
            engine.edits.push(Edit::SetParamConstraints {
                node: patch.node,
                constraints: patch.constraints,
            });
        }
        let started = Instant::now();
        engine.apply_edits().unwrap();
        let elapsed = started.elapsed().as_nanos();
        if sample > 0 {
            samples.push(elapsed);
        }
    }
    println!("two 32768-option native domains, late catalog change, complete Core application ns: {samples:?}");
}
