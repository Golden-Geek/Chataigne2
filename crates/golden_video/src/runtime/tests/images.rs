use super::*;
use crate::image::tests::prepared_image;
use crate::plan::{ExternalSource, ExternalTexture};

fn scene(asset: &golden_assets::PreparedAsset<crate::image::ImageData>) -> RenderPlan {
    RenderPlan {
        external_textures: vec![ExternalTexture {
            id: TextureId::new(1).unwrap(),
            descriptor: asset.value().descriptor(),
            source: Some(ExternalSource::Image(asset.clone())),
        }],
        outputs: vec![PlanOutput {
            id: OutputId::new(1).unwrap(),
            source: TextureId::new(1).unwrap(),
        }],
        ..Default::default()
    }
}

fn image() -> crate::image::tests::PreparedImageFixture {
    prepared_image(
        TextureDescriptor {
            extent: Extent2D::new(1, 1).unwrap(),
            format: PixelFormat::Rgba8Unorm,
            color_encoding: ColorEncoding::Srgb,
            alpha_mode: AlphaMode::Straight,
        },
        &[255, 0, 0, 128],
    )
}

#[test]
fn image_fences_are_forwarded_automatically_and_rechecked_after_cpu_preparation() {
    let image = image();
    let retirement = image.retirement();
    let mut h = Harness::new(config(), NullConfig::default());
    let now = h.runtime().read_time().unwrap();
    let project = h.runtime().project_epoch();
    let request = h
        .runtime_mut()
        .submit_plan(
            project,
            scene(&image.asset),
            PlanActivation::Scheduled(ActivationStamp::new(
                now.epoch(),
                MonotonicTime::from_nanos(1_000_000_000),
            )),
            LatePolicy::Activate,
        )
        .unwrap();
    h.wait("image prepared before scheduled activation", |h| {
        h.runtime().snapshot().prepared
    });
    image.binding.invalidate();
    h.advance(1_000_000_000);
    assert_eq!(rejected_code(h.outcome(request)), FailureCode::StaleSource);
    assert!(h.runtime().snapshot().resources.unwrap().active.is_none());
    drop(h);
    drop(image);
    assert!(retirement.wait(DEADLINE));
}

#[test]
fn current_image_survives_device_reconstruction_and_relinked_image_cannot_be_resurrected() {
    let image = image();
    let retirement = image.retirement();
    let mut h = Harness::new(config(), NullConfig::default());
    let request = h.submit(scene(&image.asset));
    let (generation, _) = h.installed(request);
    h.null.lose_device();
    h.advance(1);
    h.drive("reconstructed current image", |h| {
        h.runtime()
            .snapshot()
            .resources
            .is_some_and(|resources| resources.last_submitted.device != generation.device && resources.active.is_some())
    });
    let rebuilt = h.runtime().snapshot().resources.unwrap().last_submitted.device;
    image.binding.invalidate();
    h.null.lose_device();
    h.advance(2_000_000_000);
    h.drive("device recovery discards invalid image", |h| {
        h.runtime()
            .snapshot()
            .resources
            .is_some_and(|resources| resources.last_submitted.device != rebuilt && !resources.device_lost)
    });
    assert!(h.runtime().snapshot().resources.unwrap().active.is_none());
    drop(h);
    drop(image);
    assert!(retirement.wait(DEADLINE));
}

#[test]
fn image_retained_storage_and_guards_are_bounded_before_runtime_admission() {
    let image = image();
    let retirement = image.retirement();
    let mut limits = config();
    limits.max_input_bytes = NonZeroUsize::new(256).unwrap();
    let mut h = Harness::new(limits, NullConfig::default());
    let now = h.runtime().read_time().unwrap();
    let project = h.runtime().project_epoch();
    assert_eq!(
        h.runtime_mut().submit_plan(
            project,
            scene(&image.asset),
            PlanActivation::NextFrame(now.epoch()),
            LatePolicy::Activate
        ),
        Err(AdmissionError::InputTooLarge)
    );
    assert_eq!(h.null.snapshot().created, 0);
    drop(h);
    let mut limits = config();
    limits.plan_limits.max_dependencies = 1;
    let mut h = Harness::new(limits, NullConfig::default());
    let now = h.runtime().read_time().unwrap();
    let project = h.runtime().project_epoch();
    assert_eq!(
        h.runtime_mut().submit_plan(
            project,
            scene(&image.asset),
            PlanActivation::NextFrame(now.epoch()),
            LatePolicy::Activate
        ),
        Err(AdmissionError::InputTooLarge)
    );
    drop(h);
    drop(image);
    assert!(retirement.wait(DEADLINE));
}
