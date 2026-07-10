use std::{
    fs,
    path::{Path, PathBuf},
};

use super::*;

fn temporary_root() -> PathBuf {
    std::env::temp_dir().join(format!("golden-codegen-{}", std::process::id()))
}

#[test]
fn write_and_check_share_one_normalized_artifact_contract() {
    let root = temporary_root();
    let _ = fs::remove_dir_all(&root);
    let artifact = GeneratedArtifact {
        relative_path: "generated/types.ts".into(),
        contents: "export type Id = string;\r\n".to_owned(),
    };
    apply_artifacts(&root, [artifact.clone()], CodegenMode::Write).unwrap();
    apply_artifacts(&root, [artifact], CodegenMode::Check).unwrap();
    assert_eq!(
        fs::read_to_string(root.join("generated/types.ts")).unwrap(),
        "export type Id = string;\n"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn paths_cannot_escape_the_declared_output_root() {
    let error = apply_artifacts(
        Path::new("workspace"),
        [GeneratedArtifact {
            relative_path: "../outside".into(),
            contents: String::new(),
        }],
        CodegenMode::Check,
    )
    .unwrap_err();
    assert!(matches!(error, CodegenError::UnsafePath(_)));
}
