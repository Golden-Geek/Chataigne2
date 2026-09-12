use std::fs;

use golden_codegen_support::{AppNodeSourceRoot, generate_app_nodes_from_roots};

#[test]
fn node_bearing_child_uses_owning_module_in_generated_registry() {
    let fixture = tempfile::tempdir().expect("temporary codegen fixture");
    let source_root = fixture.path().join("src");
    let feature_dir = source_root.join("feature");
    fs::create_dir_all(&feature_dir).expect("feature directory");
    fs::write(
        feature_dir.join("mod.rs"),
        "mod child;\npub use child::ChildNode;\n#[node(\"root\")]\npub struct RootNode;\n",
    )
    .expect("root module");
    fs::write(
        feature_dir.join("child.rs"),
        "#[node(\"child\")]\npub struct ChildNode;\n",
    )
    .expect("child module");
    fs::write(
        source_root.join("standalone.rs"),
        "#[node(\"standalone\")]\npub struct StandaloneNode;\n",
    )
    .expect("standalone module");

    let output = fixture.path().join("app_nodes.rs");
    generate_app_nodes_from_roots(&[AppNodeSourceRoot::new(&source_root, "")], &output);
    let generated = fs::read_to_string(output).expect("generated registry");

    assert!(generated.contains("pub mod feature;"));
    assert!(generated.contains("pub use feature::ChildNode;"));
    assert!(generated.contains("pub use feature::RootNode;"));
    assert!(generated.contains("pub mod standalone;"));
    assert!(!generated.contains("pub mod feature_child;"));
    assert_eq!(generated.matches("pub mod feature;").count(), 1);
}

#[test]
fn filesystem_descendant_without_mod_declaration_remains_standalone() {
    let fixture = tempfile::tempdir().expect("temporary codegen fixture");
    let source_root = fixture.path().join("src");
    let parent_dir = source_root.join("module");
    let child_dir = parent_dir.join("modules");
    fs::create_dir_all(&child_dir).expect("module directories");
    fs::write(parent_dir.join("mod.rs"), "#[node(\"base\")]\npub struct ModuleBase;\n").expect("parent module");
    fs::write(
        child_dir.join("device.rs"),
        "#[node(\"device\")]\npub struct DeviceNode;\n",
    )
    .expect("independently registered node");

    let output = fixture.path().join("app_nodes.rs");
    generate_app_nodes_from_roots(&[AppNodeSourceRoot::new(&source_root, "")], &output);
    let generated = fs::read_to_string(output).expect("generated registry");

    assert!(generated.contains("pub mod module;"));
    assert!(generated.contains("pub mod module_modules_device;"));
    assert!(generated.contains("pub use module_modules_device::DeviceNode;"));
}
