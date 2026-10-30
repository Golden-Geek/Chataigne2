mod capture;
mod pictures;
mod preview;
mod publication;
mod screens;

use super::*;
use crate::app::AppEngine;
use crate::media::protocol::*;
use crate::media::{capture_channels, channel};
use golden_core::app::create_new_project_engine;
use golden_core::application::apply_project_intent;
use golden_core::ui_sync::UiEditIntent;
use golden_video::plan::PlanLimits;
use golden_video::runtime::{NullConfig, ResourceLimits, RuntimeConfig, VideoRuntime};
use golden_video::{Extent2D, OutputId};
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

fn config() -> RuntimeConfig {
    RuntimeConfig {
        plan_limits: PlanLimits {
            max_passes: 16,
            max_external_textures: 16,
            max_outputs: 16,
            max_dependencies: 64,
            max_texture_dimension: 4096,
            max_texture_bytes: 64 * 1024 * 1024,
        },
        resource_limits: ResourceLimits {
            max_bytes: 128 * 1024 * 1024,
            max_in_flight: NonZeroUsize::new(2).unwrap(),
            max_retiring: 2,
        },
        frame_rate: golden_time::RationalRate::new(60, 1).unwrap(),
        max_outstanding: NonZeroUsize::new(16).unwrap(),
        max_input_bytes: NonZeroUsize::new(65536).unwrap(),
        max_retained_input_bytes: NonZeroUsize::new(262144).unwrap(),
        recovery_delay: Duration::from_millis(10),
        max_recovery_delay: Duration::from_millis(100),
        notifier: None,
    }
}

fn engine() -> AppEngine {
    let mut engine = create_new_project_engine().unwrap();
    engine.apply_edits().unwrap();
    engine
}
fn edit(engine: &mut AppEngine, edit: MediaEdit) -> MediaEditResult {
    let result = apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(edit).unwrap(),
        },
    );
    assert!(result.success, "{result:?}");
    serde_json::from_value(result.result.unwrap().value).unwrap()
}
fn create(engine: &mut AppEngine, kind: MediaKind) -> ChannelReference {
    let MediaEditResult::Created { reference } = edit(
        engine,
        MediaEdit::CreateMedia {
            media_kind: kind,
            label: None,
        },
    ) else {
        panic!()
    };
    reference
}
fn resize(engine: &mut AppEngine, reference: ChannelReference, width: u32) {
    let mut definition = channel(engine, reference).unwrap().definition;
    definition.size = SizePolicy::Fixed {
        extent: Extent2D::new(width, 2).unwrap(),
    };
    edit(engine, MediaEdit::ConfigureChannel { reference, definition });
}
fn routes(reference: ChannelReference) -> Vec<ChannelRoute> {
    vec![
        ChannelRoute {
            output: OutputId::new(1).unwrap(),
            channel: reference,
        },
        ChannelRoute {
            output: OutputId::new(2).unwrap(),
            channel: reference,
        },
    ]
}
fn wait(mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !check() {
        assert!(Instant::now() < deadline, "channel runtime deadline");
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn renderer() -> ChannelRenderer {
    let config = config();
    let renderer = ChannelRenderer::new(
        VideoRuntime::spawn_null(config.clone(), NullConfig::default()).unwrap(),
        config.plan_limits,
    );
    wait(|| renderer.snapshot().resources.is_some());
    renderer
}
fn installed(renderer: &ChannelRenderer, request: RequestId) {
    wait(|| {
        let mut outcomes = Vec::new();
        renderer.drain_outcomes(&mut outcomes, NonZeroUsize::new(16).unwrap());
        outcomes.iter().any(|outcome| {
            if outcome.request != request {
                return false;
            }
            assert!(
                matches!(outcome.result, golden_video::runtime::OutcomeKind::Installed { .. }),
                "{outcome:?}"
            );
            true
        })
    });
}
fn close(mut renderer: ChannelRenderer) {
    let retirement = renderer.retirement();
    renderer.close();
    drop(renderer);
    assert!(retirement.wait(Duration::from_secs(5)));
}
