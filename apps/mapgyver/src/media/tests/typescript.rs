use std::collections::BTreeMap;
use std::path::Path;

fn files(directory: &Path) -> BTreeMap<String, String> {
    std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (
                path.file_name().unwrap().to_str().unwrap().into(),
                std::fs::read_to_string(path).unwrap().replace("\r\n", "\n"),
            )
        })
        .collect()
}

#[test]
fn checked_in_typescript_matches_rust_contracts() {
    let temp = tempfile::tempdir().unwrap();
    super::super::protocol::export_typescript(temp.path()).unwrap();
    let generated = files(temp.path());
    let retained = files(&Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/src/lib/media/generated"));
    assert!(!generated.is_empty());
    assert_eq!(generated, retained);
}
