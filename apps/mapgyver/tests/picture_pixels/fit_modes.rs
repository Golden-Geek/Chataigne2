use super::*;

// An independent CPU oracle checks every native output pixel, including transparent bars,
// center cropping and stretch. Three broad RGB bands avoid color-space ambiguity.
fn oracle(width: u32, height: u32, target: Extent2D, mode: MediaFitMode) -> Vec<[u8; 4]> {
    let tw = target.width();
    let th = target.height();
    let scale_x = f64::from(tw) / f64::from(width);
    let scale_y = f64::from(th) / f64::from(height);
    let (sx, sy) = match mode {
        MediaFitMode::Fit => (scale_x.min(scale_y), scale_x.min(scale_y)),
        MediaFitMode::Fill => (scale_x.max(scale_y), scale_x.max(scale_y)),
        MediaFitMode::Stretch => (scale_x, scale_y),
    };
    (0..th)
        .flat_map(|y| {
            (0..tw).map(move |x| {
                let u = (f64::from(x) + 0.5 - (f64::from(tw) - f64::from(width) * sx) / 2.) / sx;
                let v = (f64::from(y) + 0.5 - (f64::from(th) - f64::from(height) * sy) / 2.) / sy;
                if u < 0. || v < 0. || u >= f64::from(width) || v >= f64::from(height) {
                    return [0; 4];
                }
                // Pictures use sRGB input and straight output. Linear sampling is tested at exact
                // texel centers in Fit/Auto; Fill/Stretch interpolate broad bands at their boundary.
                let coordinate = if width > height { u } else { v };
                let length = width.max(height);
                let left = (coordinate - 0.5).floor();
                let fraction = coordinate - 0.5 - left;
                let color = |index: f64| {
                    let index = index.clamp(0., f64::from(length - 1)) as u32;
                    let mut rgba = [0.; 4];
                    rgba[((index * 3) / length).min(2) as usize] = 1.;
                    rgba[3] = 1.;
                    rgba
                };
                let a = color(left);
                let b = color(left + 1.);
                std::array::from_fn(|i| {
                    let value = a[i] * (1. - fraction) + b[i] * fraction;
                    let encoded = if i == 3 || value <= 0.0031308 {
                        if i == 3 { value } else { 12.92 * value }
                    } else {
                        1.055 * value.powf(1. / 2.4) - 0.055
                    };
                    (encoded * 255.).round() as u8
                })
            })
        })
        .collect()
}

#[test]
#[ignore = "requires native D3D11/Spout: M11 media Fit/Fill/Stretch pixels on every hardware adapter"]
fn two_aspect_picture_channels_share_fit_mode_and_forced_resolution_with_intrinsic_auto() {
    let hardware = adapters()
        .unwrap()
        .into_iter()
        .filter(|adapter| !adapter.software)
        .collect::<Vec<_>>();
    assert!(
        !hardware.is_empty(),
        "native Fit Mode qualification requires a hardware adapter"
    );
    for adapter in hardware {
        let root = tempfile::tempdir().unwrap();
        let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
        engine.apply_edits().unwrap();
        let MediaEditResult::Created { reference: first } = edit(
            &mut engine,
            MediaEdit::CreateMedia {
                media_kind: MediaKind::Picture,
                label: None,
            },
        ) else {
            panic!()
        };
        let MediaEditResult::Created { reference: second } = edit(
            &mut engine,
            MediaEdit::AddChannel {
                media: first.media,
                label: None,
            },
        ) else {
            panic!()
        };
        for (reference, width, height, name) in [(first, 24, 12, "wide.png"), (second, 12, 24, "tall.png")] {
            let pixels = oracle(
                width,
                height,
                Extent2D::new(width, height).unwrap(),
                MediaFitMode::Stretch,
            );
            source_png(&root.path().join(name), width, height, &pixels.concat(), false);
            edit(
                &mut engine,
                MediaEdit::ConfigurePictureFile {
                    reference,
                    file: name.into(),
                },
            );
        }
        let square = Extent2D::new(24, 24).unwrap();
        edit(
            &mut engine,
            MediaEdit::ConfigureMediaResolution {
                media: first.media,
                size: SizePolicy::Fixed { extent: square },
            },
        );
        let mut fixture = Fixture::with_extents(adapter.id, root.path(), [square; 2]);
        for mode in [MediaFitMode::Fit, MediaFitMode::Fill, MediaFitMode::Stretch] {
            edit(
                &mut engine,
                MediaEdit::ConfigureMediaFitMode {
                    media: first.media,
                    mode,
                },
            );
            fixture.publish(&engine, [first, second]);
            fixture.settle([ready(24, 24); 2], None);
            let wide = oracle(24, 12, square, mode);
            let tall = oracle(12, 24, square, mode);
            fixture.pixels(adapter.id, [&wide, &tall], &format!("{}/{mode:?}", adapter.description));
            assert_eq!(fixture.uploaded(), 2, "mode changes reuse both prepared images");
        }
        fixture.close();
        edit(
            &mut engine,
            MediaEdit::ConfigureMediaResolution {
                media: first.media,
                size: SizePolicy::Automatic,
            },
        );
        let extents = [Extent2D::new(24, 12).unwrap(), Extent2D::new(12, 24).unwrap()];
        let mut fixture = Fixture::with_extents(adapter.id, root.path(), extents);
        fixture.publish(&engine, [first, second]);
        fixture.settle([ready(24, 12), ready(12, 24)], None);
        let wide = oracle(24, 12, extents[0], MediaFitMode::Fit);
        let tall = oracle(12, 24, extents[1], MediaFitMode::Fit);
        fixture.pixels(adapter.id, [&wide, &tall], &format!("{}/Auto", adapter.description));
        fixture.close();
    }
}
