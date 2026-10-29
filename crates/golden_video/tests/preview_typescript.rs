use std::{collections::BTreeMap, path::Path};
fn files(directory: &Path) -> BTreeMap<String, String> {
    std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (
                path.file_name().unwrap().to_str().unwrap().to_owned(),
                std::fs::read_to_string(path).unwrap().replace("\r\n", "\n"),
            )
        })
        .collect()
}
#[test]
fn checked_in_preview_control_typescript_matches_rust() {
    let directory = tempfile::tempdir().unwrap();
    golden_video::preview::protocol::export_typescript(directory.path()).unwrap();
    assert_eq!(
        files(directory.path()),
        files(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/golden-video-ui/generated"))
    );
}
