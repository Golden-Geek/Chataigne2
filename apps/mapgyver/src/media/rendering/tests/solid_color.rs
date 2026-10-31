use super::*;
use golden_video::{AlphaMode, ColorEncoding, LinearColor};

#[test]
fn solid_color_channels_keep_backend_extents_linear_alpha_and_one_production_per_shared_channel() {
    let mut engine = engine();
    let first = create(&mut engine, MediaKind::SolidColor);
    let MediaEditResult::Created { reference: second } = edit(
        &mut engine,
        MediaEdit::AddChannel {
            media: first.media,
            label: None,
        },
    ) else {
        panic!()
    };
    let colors = [[0.25, 0.5, 0.75, 0.5], [0.75, 0.5, 0.25, 0.25]];
    let extents = [crate::media::resolution::default_canvas(), Extent2D::new(7, 3).unwrap()];
    for (index, reference) in [first, second].into_iter().enumerate() {
        let mut definition = channel(&engine, reference).unwrap().definition;
        definition.source = ChannelSource::SolidColor { rgba: colors[index] };
        if index == 1 {
            definition.size = SizePolicy::Fixed { extent: extents[1] };
        }
        edit(&mut engine, MediaEdit::ConfigureChannel { reference, definition });
    }
    let probe = renderer();
    let observations = probe.source_observations();
    close(probe);
    let routes = [first, first, second]
        .into_iter()
        .enumerate()
        .map(|(index, channel)| ChannelRoute {
            output: OutputId::new(index as u64 + 1).unwrap(),
            channel,
        })
        .collect::<Vec<_>>();
    let prepared = super::super::plan::build(
        &capture_channels(&engine, &[first, second], 4).unwrap(),
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
        &observations,
        None,
    )
    .unwrap();
    let plan = prepared.submission.plan;
    assert_eq!(plan.passes.len(), 2);
    assert_eq!(plan.outputs.len(), 3);
    assert_eq!(plan.outputs[0].source, plan.outputs[1].source);
    assert_ne!(plan.outputs[0].source, plan.outputs[2].source);
    assert!(plan.external_textures.is_empty());
    for (index, pass) in plan.passes.iter().enumerate() {
        assert_eq!(pass.descriptor.extent, extents[index]);
        assert_eq!(pass.descriptor.color_encoding, ColorEncoding::Linear);
        assert_eq!(pass.descriptor.alpha_mode, AlphaMode::Premultiplied);
        let rgba = colors[index];
        let golden_video::plan::RenderOperation::Clear(color) = &pass.operation else {
            panic!()
        };
        assert_eq!(
            *color,
            LinearColor::from_straight(rgba[0], rgba[1], rgba[2], rgba[3]).unwrap()
        );
    }
}
