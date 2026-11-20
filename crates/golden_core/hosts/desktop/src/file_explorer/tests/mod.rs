use super::*;

#[test]
fn folder_resolution_and_bounded_pages_keep_directories_first_and_check_explorer_root() {
    let root = tempfile::tempdir().unwrap();
    let child = root.path().join("Folder");
    fs::create_dir(&child).unwrap();
    let project = root.path().join("project.mapgyver");
    assert_eq!(root_path(None, project.to_str()), Some(root.path().to_path_buf()));
    assert_eq!(root_path(child.to_str(), project.to_str()), Some(child.clone()));
    assert_eq!(root_path(None, None), None);
    for index in 0..PAGE_SIZE + 2 {
        fs::write(root.path().join(format!("{index:03}.txt")), "").unwrap();
    }
    let first = browse(root.path(), root.path(), None, &[]).unwrap();
    assert_eq!(first.entries.len(), PAGE_SIZE);
    assert_eq!(first.entries[0].name, "Folder");
    assert!(first.entries[0].directory);
    let second = browse(root.path(), root.path(), first.next, &[]).unwrap();
    assert_eq!(second.entries.len(), 3);
    assert!(second.next.is_none());
    assert!(browse(&child, root.path(), None, &[]).is_err());
}

#[test]
fn image_thumbnail_is_a_bounded_png() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("image.png");
    image::RgbImage::new(600, 400).save(&path).unwrap();
    let preview = thumbnail(root.path(), &path).unwrap();
    let decoded = image::load_from_memory_with_format(&preview, ImageFormat::Png).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (144, 96));
}

#[test]
fn file_suffix_filter_keeps_folders_and_applies_before_paging() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("Subfolder")).unwrap();
    for index in 0..PAGE_SIZE + 2 {
        fs::write(root.path().join(format!("{index:03}.json")), "").unwrap();
    }
    fs::write(root.path().join("Glow.SHADERTOY.JSON"), "").unwrap();
    fs::write(root.path().join("Star.frag"), "").unwrap();
    let suffixes = file_suffixes(vec![".SHADERTOY.JSON".into(), ".frag".into()]).unwrap();
    let page = browse(root.path(), root.path(), None, &suffixes).unwrap();
    assert!(page.next.is_none());
    assert_eq!(
        page.entries.iter().map(|entry| entry.name.as_str()).collect::<Vec<_>>(),
        ["Subfolder", "Glow.SHADERTOY.JSON", "Star.frag"],
    );
    assert!(file_suffixes(vec!["../outside".into()]).is_err());
}
