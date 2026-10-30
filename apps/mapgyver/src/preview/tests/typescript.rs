use std::{collections::BTreeMap, path::Path};

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
fn preview_typescript_matches_rust_source_and_status_contracts() {
    let temp = tempfile::tempdir().unwrap();
    super::super::protocol::export_typescript(temp.path()).unwrap();
    assert_eq!(
        files(temp.path()),
        files(&Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/src/lib/preview/generated"))
    );
}
