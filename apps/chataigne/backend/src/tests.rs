use super::*;

#[test]
fn new_projects_begin_at_the_canonical_zero_revision() {
    assert_eq!(ChataigneProjectIdentity::new().revision, Revision::ZERO);
}

#[test]
fn phase_seven_module_catalog_owns_complete_endpoint_and_script_contracts() {
    let catalog = chataigne_module_catalog();
    for required in [
        "osc",
        "midi",
        "artnet",
        "sacn",
        "serial",
        "mqtt",
        "http",
        "tcp_client",
        "tcp_server",
        "udp",
        "websocket_client",
        "websocket_server",
        "gamepad",
        "joycon",
        "keyboard",
        "mouse",
        "kinect2",
        "streamdeck",
        "ultraleap",
        "buttplug",
        "signal",
        "metronome",
        "spatializer",
        "app_control",
        "os",
        "node",
    ] {
        let module = catalog.get(required).unwrap_or_else(|| panic!("missing {required}"));
        module.endpoint.validate().unwrap();
        assert!(!module.commands.is_empty());
        assert!(catalog.scripts().get(required).is_some());
    }
}

#[test]
fn spatializer_projects_to_a_bounded_stable_neighborhood() {
    let finite = |value| golden_values::FiniteF64::new(value).unwrap();
    let spatializer = Spatializer::compile(vec![
        SpatialTarget {
            id: 3,
            position: [finite(0.0), finite(1.0)],
        },
        SpatialTarget {
            id: 1,
            position: [finite(0.0), finite(0.0)],
        },
        SpatialTarget {
            id: 4,
            position: [finite(1.0), finite(1.0)],
        },
        SpatialTarget {
            id: 2,
            position: [finite(1.0), finite(0.0)],
        },
    ])
    .unwrap();
    let weights = spatializer.project([finite(0.5), finite(0.5)]);
    assert_eq!(weights.len(), 3);
    let total = weights.iter().map(|target| target.weight.get()).sum::<f64>();
    assert!((total - 1.0).abs() < 1.0e-12);
}

#[test]
fn desktop_headless_and_open_lan_are_product_level_host_choices() {
    let desktop = chataigne_host(golden_host::HostMode::Desktop, false, 4242).unwrap();
    let headless = chataigne_host(golden_host::HostMode::Headless, true, 4242).unwrap();
    assert_eq!(desktop.mode, golden_host::HostMode::Desktop);
    assert_eq!(headless.mode, golden_host::HostMode::Headless);
    assert!(headless.advertise_mdns);
}

#[test]
fn phase_eight_project_schema_round_trips_without_compiled_runtime_state() {
    let project = ChataigneProjectV1::empty();
    let codec = chataigne_project_codec();
    let validate = |project: &ChataigneProjectV1| project.validate().map_err(|error| error.to_string());
    let encoded = codec
        .encode(
            &golden_persistence::SaveSnapshot::new(Revision::new(7), project.clone()),
            &validate,
        )
        .unwrap();
    let decoded = codec.decode(&encoded, &validate).unwrap();
    assert_eq!(decoded.revision, Revision::new(7));
    assert_eq!(decoded.document(), &project);
    let json = String::from_utf8(encoded).unwrap();
    assert!(!json.contains("compiled_generation"));
}

#[test]
fn built_in_formula_policy_is_versioned_and_immutable() {
    for asset in builtin_formula_manifests() {
        assert!(asset.immutable);
        assert_eq!(asset.file_schema_version, golden_alchemist::FORMULA_FILE_VERSION);
        assert!(!asset.application_version.is_empty());
    }
}

#[test]
fn converted_v1_fixture_loads_saves_and_reloads_identically() {
    let codec = chataigne_project_codec();
    let validate = |project: &ChataigneProjectV1| project.validate().map_err(|error| error.to_string());
    let fixture = include_bytes!("../fixtures/minimal.golden.json");
    let loaded = codec.decode(fixture, &validate).unwrap();
    let saved = codec.encode(&loaded, &validate).unwrap();
    let reloaded = codec.decode(&saved, &validate).unwrap();
    assert_eq!(loaded.revision, reloaded.revision);
    assert_eq!(loaded.document(), reloaded.document());
}
