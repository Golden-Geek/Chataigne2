use super::*;
use crate::preview::protocol::PreviewSource;
use golden_time::{ActivationStamp, MonotonicTime};
use golden_video::preview::{
    HostViewport, PreviewController, PreviewLimits,
    protocol::{CssRect, PreviewRequest, PreviewResponse, PreviewView},
};
use golden_video::runtime::{FailureCode, OutcomeKind, PlanActivation};
use golden_video::{AlphaMode, ColorEncoding, PixelFormat, TextureDescriptor};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[derive(Default)]
struct Clock(AtomicU64);

#[test]
fn nine_procedural_preview_sources_keep_every_clear_and_texture_binding() {
    use golden_video::plan::{RegionTile, RenderOperation, UnitRect};
    use golden_video::preview::{PreviewSnapshot, ResolvedView};
    let mut engine = engine();
    let colors = [
        [1., 0., 0., 1.],
        [0., 1., 0., 1.],
        [0., 0., 1., 1.],
        [1., 1., 0., 1.],
        [0., 1., 1., 1.],
        [1., 0., 1., 1.],
        [1., 1., 1., 1.],
        [0.5, 0.5, 0.5, 1.],
        [0., 0., 0., 1.],
    ];
    let references = colors
        .iter()
        .map(|rgba| {
            let reference = create(&mut engine, MediaKind::SolidColor);
            edit(
                &mut engine,
                MediaEdit::ConfigureMediaResolution {
                    media: reference.media,
                    size: SizePolicy::Fixed {
                        extent: Extent2D::new(64, 64).unwrap(),
                    },
                },
            );
            edit(
                &mut engine,
                MediaEdit::ConfigureChannel {
                    reference,
                    source: ChannelSource::SolidColor { rgba: *rgba },
                },
            );
            reference
        })
        .collect::<Vec<_>>();
    let revision = golden_io::RevisionSource::default();
    let reader = revision.reader();
    let preview = PreviewLayout {
        output: OutputId::new(1).unwrap(),
        descriptor: TextureDescriptor {
            extent: Extent2D::new(128, 128).unwrap(),
            format: PixelFormat::Rgba8Unorm,
            color_encoding: ColorEncoding::Linear,
            alpha_mode: AlphaMode::Premultiplied,
        },
        layout: PreviewSnapshot::new(
            reader.current(),
            references
                .iter()
                .enumerate()
                .map(|(index, reference)| ResolvedView {
                    id: uuid::Uuid::new_v4(),
                    source: serde_json::to_string(&PreviewSource::Channel { reference: *reference }).unwrap(),
                    destination: UnitRect::FULL,
                    source_rect: UnitRect::FULL,
                    backdrop: UnitRect::FULL,
                    overlays: Arc::from([]),
                    overlay_scale: 1.,
                    overlay_capacity: 0,
                    tile_capacity: 1,
                    tiles: Some(Arc::from([RegionTile {
                        destination: UnitRect {
                            x: (index % 3) as f32 / 3.,
                            y: (index / 3) as f32 / 3.,
                            width: 1. / 3.,
                            height: 1. / 3.,
                        },
                        source_rect: UnitRect::FULL,
                    }])),
                })
                .collect(),
            reader.guard_at(reader.current()),
        ),
    };
    let probe = renderer();
    let observations = probe.source_observations();
    close(probe);
    let limits = crate::preview::config::configuration(1024 * 1024 * 1024, None)
        .video
        .plan_limits;
    let mut next = PreviewLayout::RESERVED_TEXTURES;
    let prepared = super::super::plan::build(
        &capture_channels(&engine, &references, crate::preview::config::MAX_CAPTURE_CHANNELS).unwrap(),
        super::super::plan::Targets {
            routes: &[],
            screens: &[],
            preview: Some(&preview),
        },
        super::super::plan::Previous {
            channels: &HashMap::new(),
            screens: &HashMap::new(),
        },
        &mut next,
        limits,
        &observations,
        None,
    )
    .unwrap();
    assert_eq!(prepared.channels.len(), 9);
    for (reference, rgba) in references.iter().zip(colors) {
        let channel = prepared
            .channels
            .iter()
            .find(|channel| channel.reference == *reference)
            .unwrap();
        assert_eq!(
            channel.resolution.availability,
            crate::media::resolution::ChannelAvailability::Ready
        );
        let pass = prepared
            .submission
            .plan
            .passes
            .iter()
            .find(|pass| pass.output == channel.target)
            .unwrap();
        assert!(matches!(pass.operation, RenderOperation::Clear(color)
            if color == golden_video::LinearColor::from_straight(rgba[0],rgba[1],rgba[2],rgba[3]).unwrap()));
    }
    assert!(prepared.submission.plan.external_textures.is_empty());
    assert_eq!(prepared.submission.plan.passes.len(), 14);
    assert!(prepared.submission.plan.compile(&limits).is_ok());
}
impl golden_time::Clock for Clock {
    fn now(&self) -> Result<MonotonicTime, golden_time::TimeError> {
        Ok(MonotonicTime::from_nanos(self.0.load(Ordering::Acquire)))
    }
}

#[test]
fn source_swap_rejects_pending_preview_install_without_revoking_retained_view_pixels() {
    let mut engine = engine();
    let first = create(&mut engine, MediaKind::SolidColor);
    let second = create(&mut engine, MediaKind::SolidColor);
    let capture = capture_channels(&engine, &[first, second], 8).unwrap();
    let (controller, receiver) = PreviewController::new(PreviewLimits::default(), None).unwrap();
    let host = HostViewport {
        client_width: 128,
        client_height: 64,
        webview_origin_x: 0,
        webview_origin_y: 0,
        webview_width: 128,
        webview_height: 64,
        css_scale: 1.,
    };
    let PreviewResponse::Connected { session, .. } = controller.handle(
        PreviewRequest::Connect {
            client: uuid::Uuid::new_v4(),
            version: golden_video::preview::protocol::PREVIEW_PROTOCOL_VERSION,
        },
        host,
    ) else {
        panic!("preview session missing")
    };
    let rect = CssRect {
        x: 0.,
        y: 0.,
        width: 128.,
        height: 64.,
    };
    let mut view = PreviewView {
        id: uuid::Uuid::new_v4(),
        source: serde_json::to_string(&PreviewSource::Channel { reference: first }).unwrap(),
        rect,
        clip: rect,
        visible: true,
        overlays: Vec::new(),
        overlay_capacity: 0,
        tiles: None,
        tile_capacity: 0,
    };
    controller.handle(
        PreviewRequest::Layout {
            session,
            revision: 1,
            views: vec![view.clone()],
        },
        host,
    );
    let preview = PreviewLayout {
        output: OutputId::new(9).unwrap(),
        descriptor: TextureDescriptor {
            extent: Extent2D::new(128, 64).unwrap(),
            format: PixelFormat::Rgba8Unorm,
            color_encoding: ColorEncoding::Linear,
            alpha_mode: AlphaMode::Premultiplied,
        },
        layout: receiver.snapshot(),
    };
    let clock = Arc::new(Clock::default());
    let config = config();
    let mut renderer = ChannelRenderer::new(
        VideoRuntime::spawn_null_with_clock(config.clone(), NullConfig::default(), clock.clone()).unwrap(),
        config.plan_limits,
    );
    wait(|| renderer.snapshot().resources.is_some());
    let mut next = PreviewLayout::RESERVED_TEXTURES;
    let prepared = super::super::plan::build(
        &capture,
        super::super::plan::Targets {
            screens: &[],
            routes: &[],
            preview: Some(&preview),
        },
        super::super::plan::Previous {
            channels: &HashMap::new(),
            screens: &HashMap::new(),
        },
        &mut next,
        config.plan_limits,
        &renderer.source_observations(),
        None,
    )
    .unwrap();
    let retained_regions = preview
        .layout
        .regions(preview.output, |_| Some(TextureId::new(1).unwrap()));
    let admission = prepared.submission.guards.clone();
    let time = renderer.runtime.read_time().unwrap();
    let project = renderer.runtime.project_epoch();
    let request = renderer
        .runtime
        .submit(
            project,
            prepared.submission,
            PlanActivation::Scheduled(ActivationStamp::new(
                time.epoch(),
                MonotonicTime::from_nanos(1_000_000_000),
            )),
            LatePolicy::Activate,
        )
        .unwrap();
    wait(|| renderer.snapshot().prepared);
    view.rect.x = 12.;
    controller.handle(
        PreviewRequest::Layout {
            session,
            revision: 2,
            views: vec![view.clone()],
        },
        host,
    );
    assert!(
        admission.iter().all(|guard| guard.is_current()),
        "camera movement must not starve prepared installation"
    );
    view.source = serde_json::to_string(&PreviewSource::Channel { reference: second }).unwrap();
    controller.handle(
        PreviewRequest::Layout {
            session,
            revision: 3,
            views: vec![view],
        },
        host,
    );
    assert!(admission.iter().any(|guard| !guard.is_current()));
    assert!(
        retained_regions
            .iter()
            .all(|region| region.visibility.as_ref().unwrap().is_current())
    );
    clock.0.store(1_000_000_000, Ordering::Release);
    renderer.runtime.wake();
    let mut outcomes = Vec::new();
    wait(|| {
        renderer.drain_outcomes(&mut outcomes, NonZeroUsize::new(16).unwrap());
        outcomes.iter().any(|outcome| outcome.request == request)
    });
    assert!(outcomes.iter().any(|outcome| outcome.request == request
        && matches!(&outcome.result, OutcomeKind::Rejected(failure) if failure.code() == FailureCode::StaleSource)));
    assert!(renderer.snapshot().resources.unwrap().active.is_none());
    close(renderer);
}
