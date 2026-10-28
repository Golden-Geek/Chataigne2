use std::fs;

use golden_codegen_support::generate_ui_protocol_bindings;
use golden_protocol::UI_PROTOCOL_VERSION;

#[test]
fn discovery_and_product_identity_are_generated_from_the_public_protocol() {
    let fixture = tempfile::tempdir().expect("temporary protocol bindings");
    let output = fixture.path().join("bindings");
    generate_ui_protocol_bindings(&output);

    let discovery = fs::read_to_string(output.join("UiDiscoveryDto.ts")).unwrap();
    let identity = fs::read_to_string(output.join("UiApplicationInfoDto.ts")).unwrap();
    assert!(discovery.contains("application: UiApplicationInfoDto"));
    assert!(discovery.contains("protocol_version: string"));
    assert!(identity.contains("data_directory_name: string"));
    assert!(identity.contains("display_name: string"));
    let version = fs::read_to_string(output.join("protocol-version.ts")).unwrap();
    assert!(version.contains(UI_PROTOCOL_VERSION));
}
