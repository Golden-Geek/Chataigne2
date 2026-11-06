use super::*;
use golden_video::runtime::{FailureCode, OutcomeKind};

#[test]
fn product_configuration_admits_eight_4k_sources_and_recovers_from_retained_generation_pressure() {
    let mut engine = engine();
    let references = (0..8)
        .map(|_| {
            let reference = create(&mut engine, MediaKind::SolidColor);
            edit(
                &mut engine,
                MediaEdit::ConfigureMediaResolution {
                    media: reference.media,
                    size: SizePolicy::Fixed {
                        extent: Extent2D::new(4096, 4096).unwrap(),
                    },
                },
            );
            reference
        })
        .collect::<Vec<_>>();
    let config = crate::preview::config::configuration(1024 * 1024 * 1024, None).video;
    let mut renderer = ChannelRenderer::new(
        VideoRuntime::spawn_null(config.clone(), NullConfig::default()).unwrap(),
        config.plan_limits,
    );
    wait(|| renderer.snapshot().resources.is_some());
    let routes = references
        .iter()
        .enumerate()
        .map(|(index, reference)| ChannelRoute {
            output: OutputId::new(index as u64 + 1).unwrap(),
            channel: *reference,
        })
        .collect::<Vec<_>>();
    let first = renderer
        .publish(capture_channels(&engine, &references, 8).unwrap(), routes.clone())
        .unwrap();
    installed(&renderer, first);
    assert_eq!(renderer.channels().len(), 8);
    assert_eq!(renderer.snapshot().resources.unwrap().bytes_used, 512 * 1024 * 1024);
    edit(
        &mut engine,
        MediaEdit::ConfigureMediaResolution {
            media: references[0].media,
            size: SizePolicy::Fixed {
                extent: Extent2D::new(8192, 8192).unwrap(),
            },
        },
    );
    let overload = renderer
        .publish(capture_channels(&engine, &references, 8).unwrap(), routes.clone())
        .unwrap();
    wait(|| {
        let mut outcomes = Vec::new();
        renderer.drain_outcomes(&mut outcomes, NonZeroUsize::new(8).unwrap());
        outcomes.into_iter().any(|outcome| {
            if outcome.request != overload {
                return false;
            }
            let OutcomeKind::Rejected(error) = outcome.result else {
                panic!("overload must be rejected");
            };
            assert_eq!(error.code(), FailureCode::ResourcePressure);
            assert!(error.message().contains("already reserved"));
            true
        })
    });
    assert_eq!(renderer.snapshot().resources.unwrap().bytes_used, 512 * 1024 * 1024);
    edit(
        &mut engine,
        MediaEdit::ConfigureMediaResolution {
            media: references[0].media,
            size: SizePolicy::Fixed {
                extent: Extent2D::new(4096, 4096).unwrap(),
            },
        },
    );
    let recovery = renderer
        .publish(capture_channels(&engine, &references, 8).unwrap(), routes)
        .unwrap();
    let recovered = installed_generation(&renderer, recovery);
    // Installation replaces the live plan before the previous generation's final use retires.
    // Assert steady capacity only after the runtime reports actual retirement, within wait's bound.
    wait(|| {
        renderer.snapshot().resources.is_some_and(|resources| {
            resources
                .active
                .is_some_and(|active| active.generation == recovered.generation)
                && resources.retiring_generations == 0
                && resources.prepared_generations == 0
                && resources.bytes_used == 512 * 1024 * 1024
        })
    });
    assert_eq!(renderer.snapshot().resources.unwrap().bytes_used, 512 * 1024 * 1024);
    assert!(renderer.snapshot().resources.unwrap().peak_bytes <= 1024 * 1024 * 1024);
    close(renderer);
}
