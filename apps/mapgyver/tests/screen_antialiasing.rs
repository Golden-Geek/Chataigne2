#![cfg(all(windows, feature = "native-qualification"))]
//! Actual app Screen plans, independent polygon coverage and finite SDK output readback.
pub mod support;

use golden_core::{application::apply_project_intent, ui_sync::UiEditIntent};
use golden_video::{Extent2D, plan::Point};
use mapgyver::{
    app::{AppEngine, AppNode},
    media::protocol::*,
    screens::protocol::*,
};
use support::{native_pixels::Fixture, raster::Image};

fn intent(engine: &mut AppEngine, namespace: &str, operation: &str, payload: serde_json::Value) -> serde_json::Value {
    let ack = apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: namespace.into(),
            operation: operation.into(),
            payload,
        },
    );
    assert!(ack.success, "{ack:?}");
    ack.result.unwrap().value
}
fn screen_edit(engine: &mut AppEngine, edit: ScreenEdit) -> ScreenEditResult {
    serde_json::from_value(intent(
        engine,
        SCREEN_NAMESPACE,
        SCREEN_OPERATION,
        serde_json::to_value(edit).unwrap(),
    ))
    .unwrap()
}
fn media_edit(engine: &mut AppEngine, edit: MediaEdit) -> MediaEditResult {
    serde_json::from_value(intent(
        engine,
        MEDIA_NAMESPACE,
        MEDIA_OPERATION,
        serde_json::to_value(edit).unwrap(),
    ))
    .unwrap()
}
fn set_mode(engine: &mut AppEngine, screen: ScreenId, mode: ScreenAntiAliasing) {
    screen_edit(
        engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                target: Some(ScreenTarget::Spout {
                    sender: "AA authored endpoint".into(),
                    extent: Extent2D::new(32, 24).unwrap(),
                }),
                anti_aliasing: mode,
                enabled: true,
            },
        },
    );
}

#[test]
#[ignore = "explicit M12/8 Screen Raw/Smooth output/preview coverage and alpha check on every hardware GPU"]
fn native_screen_raw_smooth_warp_alpha_history_and_reload() {
    let (done, completion) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            completion.recv_timeout(std::time::Duration::from_secs(120)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("Screen AA pixel/retirement watchdog expired");
            std::process::exit(1);
        }
    });
    let adapters = golden_video::native::adapters()
        .unwrap()
        .into_iter()
        .filter(|a| !a.software)
        .collect::<Vec<_>>();
    assert!(!adapters.is_empty());
    for adapter in adapters {
        let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
        engine.apply_edits().unwrap();
        let MediaEditResult::Created { reference } = media_edit(
            &mut engine,
            MediaEdit::CreateMedia {
                media_kind: MediaKind::SolidColor,
                label: None,
            },
        ) else {
            panic!()
        };
        let ScreenEditResult::ScreenCreated { screen, surface } =
            screen_edit(&mut engine, ScreenEdit::CreateScreen { label: None })
        else {
            panic!()
        };
        let quad = [[0.127, 0.093], [0.913, 0.217], [0.771, 0.887], [0.073, 0.713]];
        screen_edit(
            &mut engine,
            ScreenEdit::ConfigureSurface {
                reference: surface,
                definition: SurfaceDefinition {
                    source: Some(reference),
                    quad: Quad {
                        corners: quad.map(|p| Point {
                            x: p[0] as f32,
                            y: p[1] as f32,
                        }),
                    },
                    ..Default::default()
                },
            },
        );
        let mut fixture = Fixture::new(adapter.id);
        for alpha in [1.0, 0.5] {
            let rgba = [0.5, 0.125, 1.0, alpha];
            media_edit(
                &mut engine,
                MediaEdit::ConfigureChannel {
                    reference,
                    source: ChannelSource::SolidColor { rgba },
                },
            );
            media_edit(
                &mut engine,
                MediaEdit::ConfigureMediaResolution {
                    media: reference.media,
                    size: SizePolicy::Fixed {
                        extent: Extent2D::new(8, 8).unwrap(),
                    },
                },
            );
            for mode in [ScreenAntiAliasing::Raw, ScreenAntiAliasing::Smooth2x] {
                set_mode(&mut engine, screen, mode);
                fixture.publish_screen(&engine, screen);
                let samples = if mode == ScreenAntiAliasing::Raw { 1 } else { 2 };
                let expected = Image::screen_quad(32, 24, rgba.map(f64::from), quad, samples);
                fixture.pixels(adapter.id, &expected, &format!("{mode:?}-alpha-{alpha}"));
            }
        }
        let smooth = Image::screen_quad(32, 24, [0.5, 0.125, 1.0, 0.5], quad, 2);
        assert!(engine.undo().unwrap());
        fixture.publish_screen(&engine, screen);
        fixture.pixels(
            adapter.id,
            &Image::screen_quad(32, 24, [0.5, 0.125, 1.0, 0.5], quad, 1),
            "undo-Raw",
        );
        assert!(engine.redo().unwrap());
        fixture.publish_screen(&engine, screen);
        fixture.pixels(adapter.id, &smooth, "redo-Smooth");
        drop(fixture);
        let saved = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
        let mut loaded = golden_core::app::from_sparse_project_json::<AppNode>(&saved).unwrap();
        loaded.apply_edits().unwrap();
        let mut fixture = Fixture::new(adapter.id);
        fixture.publish_screen(&loaded, screen);
        fixture.pixels(adapter.id, &smooth, "reload-Smooth");
        drop(fixture);
        eprintln!(
            "Screen AA adapter={:?} description={} completed",
            adapter.id, adapter.description
        );
    }
    done.send(()).unwrap();
}
