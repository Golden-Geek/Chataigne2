use super::*;
use crate::screens::protocol::ScreenEdit;
use crate::screens::protocol::{DisplayRefreshRate, ScreenDefinition, ScreenTarget};
use crate::screens::rendering::capture_outputs;
use crate::screens::tests::{apply, create, fixture};
use golden_video::Extent2D;

fn bounds(left: i32) -> PhysicalRectangle {
    PhysicalRectangle {
        left,
        top: 0,
        extent: Extent2D::new(1920, 1080).unwrap(),
    }
}
fn display(id: &str, numerator: u32, denominator: u32, left: i32) -> DisplayInfo {
    DisplayInfo {
        id: id.into(),
        name: id.into(),
        edid: None,
        index: 1,
        primary: left == 0,
        bounds: bounds(left),
        refresh_rate: Some(DisplayRefreshRate { numerator, denominator }),
    }
}

#[test]
fn follows_exact_resolved_display_and_rectangle_overlap_with_truthful_fallback() {
    let displays = [display("laptop", 240, 1, 0), display("external", 60_000, 1_001, 1920)];
    let target = ResolvedTarget::Fullscreen {
        display: "external".into(),
        bounds: bounds(1920),
    };
    assert_eq!(
        resolve(ScreenFrameRate::FollowDisplay, &target, &displays),
        RationalRate::new(60_000, 1_001).unwrap()
    );
    assert_eq!(
        resolve(ScreenFrameRate::Explicit { fps: 30.0 }, &target, &displays),
        RationalRate::new(30, 1).unwrap()
    );
    assert_eq!(
        resolve(
            ScreenFrameRate::FollowDisplay,
            &ResolvedTarget::Rectangle(bounds(1700)),
            &displays
        ),
        RationalRate::new(60_000, 1_001).unwrap()
    );
    assert_eq!(
        resolve(ScreenFrameRate::FollowDisplay, &target, &[]),
        RationalRate::new(60, 1).unwrap()
    );
    let mut changed = displays.clone();
    changed[1].refresh_rate = Some(DisplayRefreshRate {
        numerator: 120,
        denominator: 1,
    });
    assert_eq!(
        resolve(ScreenFrameRate::FollowDisplay, &target, &changed),
        RationalRate::new(120, 1).unwrap()
    );
}

#[test]
fn captured_rate_tracks_live_display_changes_without_rewriting_authored_policy() {
    let mut engine = fixture();
    let (screen, _) = create(&mut engine);
    let original = ScreenDefinition {
        target: Some(ScreenTarget::Fullscreen {
            display: "external".into(),
        }),
        ..Default::default()
    };
    apply(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: original.clone(),
        },
    );
    for (n, d) in [(60_000, 1_001), (120, 1)] {
        let captured = capture_outputs(&engine, 128, 16, &[display("external", n, d, 0)]).unwrap();
        assert_eq!(captured.outputs[0].frame_rate, RationalRate::new(n, d).unwrap());
        assert_eq!(
            crate::screens::inspect(&engine).unwrap().screens[0].definition,
            original
        );
    }
}

#[test]
fn explicit_fractional_fps_is_bounded_and_finite() {
    assert_eq!(
        ScreenFrameRate::explicit_rate(59.94).unwrap(),
        RationalRate::new(2997, 50).unwrap()
    );
    for fps in [f64::NAN, f64::INFINITY, 0.0, -1.0, 1000.01] {
        assert!(ScreenFrameRate::explicit_rate(fps).is_err());
    }
}
