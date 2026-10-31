use super::*;

#[test]
fn an_unlaunched_matrix_is_ready_and_transparent_alongside_a_healthy_output() {
    let mut engine = engine();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let healthy = create(&mut engine, MediaKind::SolidColor);
    resize(&mut engine, matrix, 4);
    resize(&mut engine, healthy, 4);
    let targets = [
        ChannelRoute {
            output: OutputId::new(1).unwrap(),
            channel: matrix,
        },
        ChannelRoute {
            output: OutputId::new(2).unwrap(),
            channel: healthy,
        },
    ];
    let mut owner = renderer();
    let request = owner
        .publish(
            capture_channels(&engine, &[matrix, healthy], 16).unwrap(),
            targets.to_vec(),
        )
        .unwrap();
    installed(&owner, request);
    assert_eq!(
        owner
            .channels()
            .iter()
            .find(|channel| channel.reference == matrix)
            .unwrap()
            .resolution
            .availability,
        ChannelAvailability::Ready
    );
    assert_eq!(
        owner
            .channels()
            .iter()
            .find(|channel| channel.reference == healthy)
            .unwrap()
            .resolution
            .availability,
        ChannelAvailability::Ready
    );
    close(owner);
}
use crate::media::resolution::ChannelAvailability;

#[test]
fn unavailable_composition_preserves_healthy_output_publication() {
    let mut engine = engine();
    let composition = create(&mut engine, MediaKind::Composition);
    let solid = create(&mut engine, MediaKind::SolidColor);
    edit(
        &mut engine,
        MediaEdit::Composition {
            reference: composition,
            edit: crate::media::composition::protocol::CompositionEdit::AddInstances {
                sources: vec![solid; 65],
            },
        },
    );
    resize(&mut engine, composition, 4);
    resize(&mut engine, solid, 2);
    let mut renderer = renderer();
    let request = renderer
        .publish(
            capture_channels(&engine, &[composition, solid], 16).unwrap(),
            vec![
                ChannelRoute {
                    output: OutputId::new(1).unwrap(),
                    channel: composition,
                },
                ChannelRoute {
                    output: OutputId::new(2).unwrap(),
                    channel: solid,
                },
            ],
        )
        .unwrap();
    installed(&renderer, request);
    assert_eq!(
        renderer
            .channels()
            .iter()
            .find(|channel| channel.reference == composition)
            .unwrap()
            .resolution
            .availability,
        ChannelAvailability::Unavailable
    );
    assert_eq!(
        renderer
            .channels()
            .iter()
            .find(|channel| channel.reference == solid)
            .unwrap()
            .resolution
            .availability,
        ChannelAvailability::Ready
    );
    assert_eq!(renderer.snapshot().resources.unwrap().active.unwrap().output_count, 2);
    close(renderer);
}

#[test]
fn shared_consumers_resize_undo_reload_and_resource_recreation_preserve_channel_identity() {
    let mut engine = engine();
    let reference = create(&mut engine, MediaKind::SolidColor);
    resize(&mut engine, reference, 2);
    let mut renderer = renderer();
    let request = renderer
        .publish(capture_channels(&engine, &[reference], 16).unwrap(), routes(reference))
        .unwrap();
    installed(&renderer, request);
    assert_eq!(renderer.channels().len(), 1);
    let first = renderer.channels()[0];
    assert_eq!(first.reference, reference);
    assert_eq!(renderer.snapshot().resources.unwrap().active.unwrap().output_count, 2);
    let generation = renderer.snapshot().resources.unwrap().active.unwrap().generation;
    resize(&mut engine, reference, 3);
    let request = renderer
        .publish(capture_channels(&engine, &[reference], 16).unwrap(), routes(reference))
        .unwrap();
    installed(&renderer, request);
    assert_eq!(renderer.channels()[0].target, first.target);
    assert_eq!(
        renderer.channels()[0].resolution.extent,
        Some(Extent2D::new(3, 2).unwrap())
    );
    assert!(renderer.snapshot().resources.unwrap().active.unwrap().generation > generation);
    let old = renderer.source_observations();
    let device = renderer.snapshot().resources.unwrap().last_submitted.device;
    renderer.null_control().unwrap().lose_device();
    wait(|| {
        renderer
            .snapshot()
            .resources
            .is_some_and(|snapshot| snapshot.last_submitted.device != device && snapshot.active.is_some())
    });
    assert!(
        renderer
            .publish_observed(
                capture_channels(&engine, &[reference], 16).unwrap(),
                routes(reference),
                old
            )
            .is_err()
    );
    let request = renderer.refresh().unwrap().unwrap();
    installed(&renderer, request);
    assert_eq!(renderer.channels()[0].reference, reference);
    engine.undo().unwrap();
    let request = renderer
        .publish(capture_channels(&engine, &[reference], 16).unwrap(), routes(reference))
        .unwrap();
    installed(&renderer, request);
    assert_eq!(
        renderer.channels()[0].resolution.extent,
        Some(Extent2D::new(2, 2).unwrap())
    );
    let old = renderer.source_observations();
    let project = renderer.snapshot().project;
    let json = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let mut restored = golden_core::app::from_sparse_project_json::<crate::app::AppNode>(&json).unwrap();
    restored.apply_edits().unwrap();
    let request = renderer
        .publish(
            capture_channels(&restored, &[reference], 16).unwrap(),
            routes(reference),
        )
        .unwrap();
    installed(&renderer, request);
    assert_ne!(renderer.snapshot().project, project);
    assert_eq!(renderer.channels()[0].reference, reference);
    assert!(
        renderer
            .publish_observed(
                capture_channels(&restored, &[reference], 16).unwrap(),
                routes(reference),
                old
            )
            .is_err()
    );
    assert!(renderer.refresh().unwrap().is_none());
    close(renderer);
}

#[test]
fn missing_and_unconfigured_channels_publish_explicit_transparent_placeholders() {
    let mut engine = engine();
    let reference = create(&mut engine, MediaKind::SharedTexture);
    let capture = capture_channels(&engine, &[reference], 16).unwrap();
    let probe = renderer();
    let observations = probe.source_observations();
    close(probe);
    let built = super::super::plan::build(
        &capture,
        super::super::plan::Targets {
            screens: &[],
            routes: &routes(reference),
            preview: None,
        },
        super::super::plan::Previous {
            channels: &HashMap::new(),
            screens: &HashMap::new(),
        },
        &mut 0,
        config().plan_limits,
        &observations,
        None,
    )
    .unwrap();
    assert_eq!(
        built.channels[0].resolution.availability,
        ChannelAvailability::Unconfigured
    );
    assert_eq!(built.channels[0].resolution.extent, None);
    assert_eq!(
        built.submission.plan.passes[0].descriptor.extent,
        Extent2D::new(1, 1).unwrap()
    );
    assert!(built.submission.source_demands.is_empty());
    let wrong = ChannelReference {
        channel: ChannelId(golden_core::model::NodeUuid(uuid::Uuid::new_v4())),
        ..reference
    };
    let built = super::super::plan::build(
        &capture_channels(&engine, &[wrong], 16).unwrap(),
        super::super::plan::Targets {
            screens: &[],
            routes: &routes(wrong),
            preview: None,
        },
        super::super::plan::Previous {
            channels: &HashMap::new(),
            screens: &HashMap::new(),
        },
        &mut 0,
        config().plan_limits,
        &observations,
        None,
    )
    .unwrap();
    assert_eq!(built.channels[0].resolution.availability, ChannelAvailability::Missing);
    assert!(built.channels[0].source.is_none());
}

#[test]
fn automatic_spout_warms_without_inventing_an_extent_and_changed_selector_gets_a_fresh_binding() {
    let mut engine = engine();
    let reference = create(&mut engine, MediaKind::SharedTexture);
    let mut definition = channel(&engine, reference).unwrap().definition;
    let ChannelSource::Spout { sender, .. } = &mut definition.source else {
        panic!()
    };
    *sender = Some("A".into());
    edit(
        &mut engine,
        MediaEdit::ConfigureChannel {
            reference,
            definition: definition.clone(),
        },
    );
    let mut renderer = renderer();
    let request = renderer
        .publish(capture_channels(&engine, &[reference], 16).unwrap(), routes(reference))
        .unwrap();
    installed(&renderer, request);
    let first = renderer.channels()[0];
    assert_eq!(first.resolution.extent, None);
    assert!(first.source.is_some());
    let ChannelSource::Spout { sender, .. } = &mut definition.source else {
        panic!()
    };
    *sender = Some("B".into());
    edit(&mut engine, MediaEdit::ConfigureChannel { reference, definition });
    let stale = renderer.desired.as_ref().unwrap().capture.clone();
    assert!(!stale.is_current());
    assert!(renderer.publish(stale, routes(reference)).is_err());
    let request = renderer
        .publish(capture_channels(&engine, &[reference], 16).unwrap(), routes(reference))
        .unwrap();
    installed(&renderer, request);
    assert_ne!(renderer.channels()[0].source, first.source);
    assert_eq!(renderer.channels()[0].reference, reference);
    close(renderer);
}
