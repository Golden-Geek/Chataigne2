use std::io::Read;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};

use golden_assets::{
    AssetBinding, AssetDecoder, AssetId, AssetInput, AssetLocator, AssetPreparer, AssetRecord, AssetScope,
    PreparationFailure, PreparationLimits,
};
use golden_time::{ActivationStamp, MonotonicTime, RationalRate};
use golden_video::plan::{PlanLimits, PlanOutput, RenderOperation, RenderPass, RenderPlan};
use golden_video::runtime::{
    FailureCode, LatePolicy, NullConfig, OutcomeKind, PlanActivation, PlanSubmission, ResourceLimits, RuntimeConfig,
    VideoRuntime,
};
use golden_video::{
    AlphaMode, ColorEncoding, Extent2D, LinearColor, OutputId, PassId, PixelFormat, TextureDescriptor, TextureId,
};

const WAIT: Duration = Duration::from_secs(5);

struct DescriptorDecoder;
impl AssetDecoder for DescriptorDecoder {
    type Output = TextureDescriptor;

    fn decode(&mut self, input: &mut AssetInput, maximum_bytes: usize) -> Result<Self::Output, PreparationFailure> {
        if maximum_bytes < size_of::<TextureDescriptor>() || input.length() != 8 {
            return Err(PreparationFailure::Decode);
        }
        let mut bytes = [0; 8];
        input.read_exact(&mut bytes).map_err(PreparationFailure::from)?;
        let width = u32::from_le_bytes(bytes[..4].try_into().unwrap());
        let height = u32::from_le_bytes(bytes[4..].try_into().unwrap());
        Ok(TextureDescriptor {
            extent: Extent2D::new(width, height).map_err(|_| PreparationFailure::Decode)?,
            format: PixelFormat::Rgba8Unorm,
            color_encoding: ColorEncoding::Linear,
            alpha_mode: AlphaMode::Premultiplied,
        })
    }
    fn retained_bytes(&self, _: &Self::Output) -> usize {
        size_of::<TextureDescriptor>()
    }
}

fn wait(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !ready() {
        assert!(Instant::now() < deadline, "public asset/video consumer did not settle");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn public_video_consumer_uses_worker_prepared_extent_and_rechecks_asset_fences_at_installation() {
    let project = tempfile::tempdir().unwrap();
    let bytes = [320_u32.to_le_bytes(), 180_u32.to_le_bytes()].concat();
    std::fs::write(project.path().join("descriptor.bin"), bytes).unwrap();
    let scope = AssetScope::new(project.path()).unwrap();
    let binding = AssetBinding::new(AssetRecord {
        id: AssetId::new(),
        locator: AssetLocator::ProjectRelative {
            path: "descriptor.bin".to_string().try_into().unwrap(),
        },
    });
    let mut preparation = AssetPreparer::new(DescriptorDecoder, PreparationLimits::default()).unwrap();
    preparation.submit(scope.capture(&binding)).unwrap();
    let mut prepared = Vec::new();
    wait(|| {
        preparation.drain(&mut prepared, NonZeroUsize::MIN);
        !prepared.is_empty()
    });
    let asset = prepared.pop().unwrap().result.unwrap();
    assert_eq!(asset.id(), binding.record().id);
    assert_eq!(asset.value().extent, Extent2D::new(320, 180).unwrap());
    let config = RuntimeConfig {
        plan_limits: PlanLimits {
            max_passes: 4,
            max_external_textures: 4,
            max_outputs: 2,
            max_dependencies: 8,
            max_texture_dimension: 1024,
            max_texture_bytes: 1024 * 1024,
        },
        resource_limits: ResourceLimits {
            max_bytes: 1024 * 1024,
            max_in_flight: NonZeroUsize::new(2).unwrap(),
            max_retiring: 2,
        },
        frame_rate: RationalRate::new(60, 1).unwrap(),
        max_outstanding: NonZeroUsize::new(8).unwrap(),
        max_input_bytes: NonZeroUsize::new(16 * 1024).unwrap(),
        max_retained_input_bytes: NonZeroUsize::new(128 * 1024).unwrap(),
        recovery_delay: Duration::from_millis(20),
        max_recovery_delay: Duration::from_millis(100),
        notifier: None,
    };
    let mut video = VideoRuntime::spawn_null(config, NullConfig::default()).unwrap();
    let descriptor = *asset.value();
    let mut submission = PlanSubmission::new(RenderPlan {
        external_textures: Vec::new(),
        passes: vec![RenderPass {
            id: PassId::new(1).unwrap(),
            output: TextureId::new(1).unwrap(),
            descriptor,
            operation: RenderOperation::Clear(LinearColor::TRANSPARENT),
        }],
        outputs: vec![PlanOutput {
            id: OutputId::new(1).unwrap(),
            source: TextureId::new(1).unwrap(),
        }],
    });
    submission.guards = Arc::from(asset.guards());
    let mut outcomes = Vec::new();
    let epoch = video.read_time().unwrap().epoch();
    let installed = video
        .submit(
            video.project_epoch(),
            submission.clone(),
            PlanActivation::NextFrame(epoch),
            LatePolicy::Activate,
        )
        .unwrap();
    wait(|| {
        video.drain_outcomes(&mut outcomes, NonZeroUsize::new(8).unwrap());
        outcomes
            .iter()
            .any(|outcome| outcome.request == installed && matches!(outcome.result, OutcomeKind::Installed { .. }))
    });
    let live = video.snapshot().resources.unwrap().active.unwrap().identity;
    outcomes.clear();
    let now = video.read_time().unwrap();
    let future = MonotonicTime::from_nanos(now.monotonic().as_nanos().checked_add(1_000_000_000).unwrap());
    let request = video
        .submit(
            video.project_epoch(),
            submission,
            PlanActivation::Scheduled(ActivationStamp::new(now.epoch(), future)),
            LatePolicy::Activate,
        )
        .unwrap();
    wait(|| video.snapshot().prepared);
    binding.invalidate();
    assert!(!asset.is_current());
    wait(|| {
        video.drain_outcomes(&mut outcomes, NonZeroUsize::new(8).unwrap());
        !outcomes.is_empty()
    });
    let outcome = outcomes.iter().find(|outcome| outcome.request == request).unwrap();
    assert!(matches!(&outcome.result, OutcomeKind::Rejected(failure) if failure.code() == FailureCode::StaleSource));
    assert_eq!(video.snapshot().resources.unwrap().active.unwrap().identity, live);
    let video_retirement = video.retirement();
    video.close();
    drop(video);
    assert!(video_retirement.wait(WAIT));
    let retirement = preparation.retirement();
    drop(asset);
    drop(preparation);
    assert!(retirement.wait(WAIT));
}
