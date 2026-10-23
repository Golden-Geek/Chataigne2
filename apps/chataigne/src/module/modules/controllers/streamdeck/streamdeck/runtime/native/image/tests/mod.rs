use super::*;
#[test]
fn bounded_target_and_visual_validation() {
    let visual = KeyVisual::default();
    for size in [(0, 1), (1, 0), (129, 1), (1, 129)] {
        assert!(compose(size, &visual).is_err());
    }
    let mut invalid = visual.clone();
    invalid.color.0 = f64::NAN;
    assert!(compose((72, 72), &invalid).is_err());
    invalid = visual;
    invalid.image = "x".repeat(4097);
    assert!(compose((72, 72), &invalid).is_err());
}
#[test]
fn solid_background_retains_native_rgb_contract() {
    let visual = KeyVisual {
        color: (1.0, 0.0, 0.5, 0.0),
        ..Default::default()
    };
    let image = compose((72, 72), &visual).unwrap();
    assert_eq!(image.to_rgb8().get_pixel(0, 0).0, [255, 0, 128]);
}

#[test]
fn image_files_are_bounded_and_invalid_or_nonregular_sources_fail() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("image.png");
    let file = File::create(&path).unwrap();
    file.set_len(MAX_FILE as u64 + 1).unwrap();
    drop(file);
    let mut visual = KeyVisual {
        image: path.to_string_lossy().into_owned(),
        ..Default::default()
    };
    assert!(compose((72, 72), &visual).unwrap_err().contains("8 MiB"));
    std::fs::write(&path, b"corrupt image").unwrap();
    assert!(compose((72, 72), &visual).is_err());
    visual.image = directory.path().to_string_lossy().into_owned();
    assert!(compose((72, 72), &visual).is_err());
}

#[test]
fn source_dimensions_and_decoded_allocation_are_limited_before_resize() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("image.png");
    image::RgbaImage::new(2049, 1).save(&path).unwrap();
    let visual = KeyVisual {
        image: path.to_string_lossy().into_owned(),
        ..Default::default()
    };
    assert!(compose((72, 72), &visual).is_err());
    image::ImageBuffer::<image::Rgba<u16>, Vec<u16>>::new(2048, 2048)
        .save(&path)
        .unwrap();
    assert!(std::fs::metadata(&path).unwrap().len() < MAX_FILE as u64);
    assert!(compose((72, 72), &visual).is_err());
}

#[test]
fn alpha_blends_loaded_image_over_the_control_background() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("image.png");
    image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 255, 128]))
        .save(&path)
        .unwrap();
    let visual = KeyVisual {
        image: path.to_string_lossy().into_owned(),
        color: (1.0, 0.0, 0.0, 1.0),
        ..Default::default()
    };
    let image = compose((1, 1), &visual).unwrap();
    assert_eq!(image.to_rgb8().get_pixel(0, 0).0, [127, 0, 128]);
}
