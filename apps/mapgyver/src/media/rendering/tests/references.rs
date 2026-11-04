use super::*;

fn plan(engine: &AppEngine, references: &[ChannelReference]) -> super::super::plan::Prepared {
    let probe = renderer();
    let observations = probe.source_observations();
    close(probe);
    let routes = references
        .iter()
        .enumerate()
        .map(|(index, channel)| ChannelRoute {
            channel: *channel,
            output: OutputId::new(index as u64 + 1).unwrap(),
        })
        .collect::<Vec<_>>();
    super::super::plan::build(
        &capture_channels(engine, references, 32).unwrap(),
        super::super::plan::Targets {
            routes: &routes,
            screens: &[],
            preview: None,
        },
        super::super::plan::Previous {
            channels: &HashMap::new(),
            screens: &HashMap::new(),
        },
        &mut 0,
        config().plan_limits,
        super::super::plan::Sources {
            observations: &observations,
            pictures: None,
            calibration: &mut Default::default(),
        },
    )
    .unwrap()
}

#[test]
fn reference_passthrough_reuses_target_and_forced_resolution_adds_one_shared_pass() {
    let mut engine = engine();
    let source = create(&mut engine, MediaKind::SolidColor);
    edit(
        &mut engine,
        MediaEdit::ConfigureMediaResolution {
            media: source.media,
            size: SizePolicy::Fixed {
                extent: Extent2D::new(7, 3).unwrap(),
            },
        },
    );
    let MediaEditResult::ReferenceCreated { media } = edit(
        &mut engine,
        MediaEdit::CreateReference {
            target: Some(source.media),
            label: None,
        },
    ) else {
        panic!()
    };
    let alias = ChannelReference {
        media,
        channel: source.channel,
    };
    let passthrough = plan(&engine, &[source, alias, alias]);
    assert_eq!(passthrough.submission.plan.passes.len(), 1);
    assert!(passthrough.submission.plan.external_textures.is_empty());
    assert_eq!(
        passthrough.submission.plan.outputs[0].source,
        passthrough.submission.plan.outputs[1].source
    );
    assert_eq!(
        passthrough.submission.plan.outputs[1].source,
        passthrough.submission.plan.outputs[2].source
    );
    edit(
        &mut engine,
        MediaEdit::ConfigureMediaResolution {
            media,
            size: SizePolicy::Fixed {
                extent: Extent2D::new(3, 2).unwrap(),
            },
        },
    );
    let sampled = plan(&engine, &[source, alias, alias]);
    assert_eq!(sampled.submission.plan.passes.len(), 2);
    assert_ne!(
        sampled.submission.plan.outputs[0].source,
        sampled.submission.plan.outputs[1].source
    );
    assert_eq!(
        sampled.submission.plan.outputs[1].source,
        sampled.submission.plan.outputs[2].source
    );
    assert_eq!(
        sampled.submission.plan.passes.last().unwrap().descriptor.extent,
        Extent2D::new(3, 2).unwrap()
    );
    assert!(engine.undo().unwrap());
    assert_eq!(plan(&engine, &[alias]).submission.plan.passes.len(), 1);
}
