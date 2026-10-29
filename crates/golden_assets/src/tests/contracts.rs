use std::path::Path;

use crate::*;

fn relative(path: &str) -> AssetLocator {
    AssetLocator::ProjectRelative {
        path: path.to_owned().try_into().unwrap(),
    }
}

fn native_absolute() -> &'static str {
    if cfg!(windows) {
        "C:\\Pictures\\still.png"
    } else {
        "/pictures/still.png"
    }
}

#[test]
fn identity_roundtrip_is_transparent_and_relink_preserves_it() {
    let id = AssetId::new();
    assert_eq!(id.get().get_version_num(), 4);
    assert_ne!(id, AssetId::default());
    assert_eq!(serde_json::to_value(id).unwrap(), id.get().to_string());
    let record = AssetRecord {
        id,
        locator: relative("media/first.png"),
    };
    let mut relinked: AssetRecord = serde_json::from_str(&serde_json::to_string(&record).unwrap()).unwrap();
    assert_eq!(record, relinked);
    relinked.locator = relative("assets/relinked.png");
    assert_eq!(record.id, relinked.id);
    assert_ne!(record.locator, relinked.locator);
}

#[test]
fn locators_roundtrip_with_validated_tagged_paths_and_resolve_without_io() {
    let original = relative("media/été.png");
    assert_eq!(
        serde_json::to_value(&original).unwrap(),
        serde_json::json!({ "kind": "project_relative", "path": "media/été.png" })
    );
    let decoded: AssetLocator = serde_json::from_value(serde_json::to_value(&original).unwrap()).unwrap();
    assert_eq!(decoded, original);
    assert_eq!(
        original.resolve(Path::new("unavailable-project")),
        Path::new("unavailable-project").join("media/été.png")
    );
    let absolute = AssetLocator::Absolute {
        path: native_absolute().to_owned().try_into().unwrap(),
    };
    assert_eq!(
        serde_json::to_value(&absolute).unwrap(),
        serde_json::json!({ "kind": "absolute", "path": native_absolute() })
    );
    let decoded: AssetLocator = serde_json::from_value(serde_json::to_value(&absolute).unwrap()).unwrap();
    assert_eq!(decoded, absolute);
    assert_eq!(
        absolute.resolve(Path::new("ignored-project")),
        Path::new(native_absolute())
    );
}

#[test]
fn relative_locators_reject_nonportable_and_traversal_paths_at_construction_and_decode() {
    for path in [
        "",
        "/root.png",
        "../escape.png",
        "dir/../escape.png",
        "./image.png",
        "dir/./image.png",
        "dir//image.png",
        "dir/image.png/",
        "dir\\image.png",
        "C:/image.png",
        "https://host/image.png",
        "image\0.png",
    ] {
        assert!(
            RelativeAssetPath::try_from(path.to_owned()).is_err(),
            "accepted {path:?}"
        );
        assert!(
            serde_json::from_value::<AssetLocator>(serde_json::json!({
                "kind": "project_relative", "path": path,
            }))
            .is_err(),
            "decoded {path:?}"
        );
    }
    let maximum = "a".repeat(MAX_ASSET_PATH_BYTES);
    assert!(RelativeAssetPath::try_from(maximum.clone()).is_ok());
    assert_eq!(RelativeAssetPath::try_from(maximum + "a"), Err(LocatorError::TooLong));
}

#[test]
fn absolute_locators_validate_native_absolute_paths_and_bounded_strings_during_decode() {
    for path in ["", "media/image.png", "./image.png", "image\0.png"] {
        assert!(AbsoluteAssetPath::try_from(path.to_owned()).is_err());
        assert!(
            serde_json::from_value::<AssetLocator>(serde_json::json!({ "kind": "absolute", "path": path })).is_err()
        );
    }
    assert!(AbsoluteAssetPath::try_from(native_absolute().to_owned()).is_ok());
    assert_eq!(
        AbsoluteAssetPath::try_from("a".repeat(MAX_ASSET_PATH_BYTES + 1)),
        Err(LocatorError::TooLong)
    );
    assert!(serde_json::from_value::<AssetLocator>(serde_json::json!({ "kind": "unknown", "path": "x" })).is_err());
    assert!(serde_json::from_value::<AssetId>(serde_json::json!("not-a-uuid")).is_err());
}
