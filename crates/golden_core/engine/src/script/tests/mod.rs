use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

mod runtime_recovery;

#[test]
fn custom_template_can_include_core_default_template_by_namespace() {
    let root = create_temp_template_root("core-include");
    let template_path = root.join("custom.js");
    fs::write(&template_path, "{{include:core/default.js}}\n").expect("custom template should be written");

    let source = super::template::read_template_from_path(&template_path, &root)
        .expect("namespaced core include should resolve from Golden Core root");

    assert!(source.contains("// Default script template for Golden Core script nodes."));
    assert!(source.contains("function init()"));

    remove_temp_template_root(&root);
}

#[test]
fn custom_template_does_not_fall_back_to_core_without_namespace() {
    let root = create_temp_template_root("local-only-include");
    let template_path = root.join("custom.js");
    fs::write(&template_path, "{{include:snippets/header.js}}\n").expect("custom template should be written");

    let error = super::template::read_template_from_path(&template_path, &root)
        .expect_err("plain includes should stay scoped to the current template root");

    assert!(error.contains("snippets/header.js"));
    assert!(error.contains(&root.join("snippets/header.js").display().to_string()));

    remove_temp_template_root(&root);
}

fn create_temp_template_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("golden-script-template-tests-{label}-{unique}"));
    fs::create_dir_all(&root).expect("temp template root should be created");
    root
}

fn remove_temp_template_root(root: &PathBuf) {
    let _ = fs::remove_dir_all(root);
}
