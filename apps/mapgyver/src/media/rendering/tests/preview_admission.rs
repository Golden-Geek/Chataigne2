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
            version: 1,
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
    let retained_regions = preview.layout.regions(|_| Some(TextureId::new(1).unwrap()));
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
