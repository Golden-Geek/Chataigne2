use crate::{
    media::protocol::{MEDIA_NAMESPACE, MEDIA_OPERATION, MediaEdit, MediaEditResult, MediaKind},
    preview::{
        capture::capture_presentation,
        config::{MAX_CAPTURE_CHANNELS, configuration},
        protocol::PreviewSource,
    },
};
use golden_core::{app::create_new_project_engine, application::apply_project_intent, ui_sync::UiEditIntent};
use golden_video::plan::{ExternalTexture, GraphLimit, PlanError, RenderPlan};
use golden_video::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor, TextureId};

#[test]
fn logical_preview_capture_does_not_treat_procedural_channels_as_physical_inputs() {
    let mut engine = create_new_project_engine::<crate::app::AppNode>().unwrap();
    engine.apply_edits().unwrap();
    let sources = (0..9)
        .map(|_| {
            let ack = apply_project_intent(
                &mut engine,
                UiEditIntent::InvokeAppOperation {
                    namespace: MEDIA_NAMESPACE.into(),
                    operation: MEDIA_OPERATION.into(),
                    payload: serde_json::to_value(MediaEdit::CreateMedia {
                        media_kind: MediaKind::SolidColor,
                        label: None,
                    })
                    .unwrap(),
                },
            );
            assert!(ack.success, "{ack:?}");
            let MediaEditResult::Created { reference } = serde_json::from_value(ack.result.unwrap().value).unwrap()
            else {
                panic!()
            };
            PreviewSource::Channel { reference }
        })
        .collect::<Vec<_>>();
    assert!(
        capture_presentation(&engine, &sources, MAX_CAPTURE_CHANNELS, 128, &[])
            .unwrap()
            .channels
            .is_current()
    );
    assert!(capture_presentation(&engine, &sources, 8, 128, &[]).is_err());
}

#[test]
fn desktop_physical_input_and_graph_limits_remain_in_force_after_metadata_capture() {
    let config = configuration(1024 * 1024 * 1024, None);
    let descriptor = TextureDescriptor {
        extent: Extent2D::new(1, 1).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    };
    let plan = RenderPlan {
        external_textures: (1..=9)
            .map(|id| ExternalTexture {
                id: TextureId::new(id).unwrap(),
                descriptor,
                source: None,
            })
            .collect(),
        ..Default::default()
    };
    assert!(matches!(
        plan.compile(&config.video.plan_limits),
        Err(PlanError::GraphLimitExceeded {
            kind: GraphLimit::ExternalTextures,
            actual: 9,
            limit: 8,
        })
    ));
    assert_eq!(config.video.plan_limits.max_passes, 24);
    assert_eq!(config.video.plan_limits.max_dependencies, 128);
}
