use super::*;
use crate::screens::rendering::{ResolvedTarget, capture_outputs};
use golden_core::app::{from_sparse_project_json, to_sparse_project_json_pretty, validate_engine_project_candidate};
use golden_video::Extent2D;

#[test]
fn ndi_screen_target_resolves_typed_size_rate_and_protocol_local_names_through_reload() {
    let mut engine = fixture();
    let (first, _) = create(&mut engine);
    let (second, _) = create(&mut engine);
    let extent = Extent2D::new(320, 240).unwrap();
    let configure_ndi = |screen, sender: Option<&str>| ScreenEdit::ConfigureScreen {
        screen,
        definition: ScreenDefinition {
            target: Some(ScreenTarget::Ndi {
                sender: sender.map(str::to_owned),
                extent,
            }),
            frame_rate: ScreenFrameRate::Explicit { fps: 30.0 },
            ..Default::default()
        },
    };
    apply(&mut engine, configure_ndi(first, None));
    let get = |engine: &AppEngine| {
        capture_outputs(engine, 128, 16, &[])
            .unwrap()
            .outputs
            .into_iter()
            .find(|output| output.screen == first)
            .unwrap()
    };
    let initial = get(&engine);
    assert_eq!(initial.frame_rate, golden_time::RationalRate::new(30, 1).unwrap());
    assert_eq!(
        initial.target,
        ResolvedTarget::Ndi {
            sender: "MapGyver - Screen".into(),
            extent
        }
    );
    apply(
        &mut engine,
        ScreenEdit::RenameScreen {
            screen: first,
            label: "Stage / A".into(),
        },
    );
    assert_eq!(
        get(&engine).target,
        ResolvedTarget::Ndi {
            sender: "MapGyver - Stage   A".into(),
            extent
        }
    );
    assert!(!invoke(&mut engine, configure_ndi(first, Some("invalid:sender"))).success);
    apply(&mut engine, configure_ndi(first, Some("Local shared name")));
    assert!(!invoke(&mut engine, configure_ndi(second, Some("Local shared name"))).success);
    apply(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen: second,
            definition: ScreenDefinition {
                target: Some(ScreenTarget::Spout {
                    sender: "Local shared name".into(),
                    extent,
                }),
                ..Default::default()
            },
        },
    );
    assert_eq!(
        get(&engine).target,
        ResolvedTarget::Ndi {
            sender: "Local shared name".into(),
            extent
        }
    );

    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut reloaded = from_sparse_project_json(&json).unwrap();
    reloaded.apply_edits().unwrap();
    validate_engine_project_candidate(&reloaded).unwrap();
    assert_eq!(get(&reloaded), get(&engine));
}
