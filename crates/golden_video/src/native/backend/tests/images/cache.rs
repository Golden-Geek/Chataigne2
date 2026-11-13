use super::*;
use crate::image::{ImageData, ImageStream};
use crate::native::{DiagnosticDelivery, DiagnosticEvent, DiagnosticsConfig, OutputTrace};
use crate::plan::{BlitRegion, Layer, OpacityEnvelope, RegionPlacement, UnitRect};
use std::num::NonZeroUsize;

fn time() -> TimeSnapshot {
    TimeAuthority::new(SystemClock::new(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL)
        .unwrap()
        .sample()
        .unwrap()
}

fn frame(renderer: &mut Renderer, time: TimeSnapshot, presentation: bool) -> (Vec<[u8; 4]>, (u64, u64), OutputTrace) {
    renderer.serial += 1;
    let token = CompletionToken {
        device: renderer.device,
        serial: renderer.serial,
    };
    let resources = renderer.resources.as_ref().unwrap();
    if presentation {
        renderer
            .backend
            .present(resources, &renderer.plan, token, time)
            .unwrap();
    } else {
        renderer.backend.submit(resources, &renderer.plan, token, time).unwrap();
    }
    let deadline = Instant::now() + WAIT;
    while renderer.backend.completion().unwrap().serial < token.serial {
        assert!(Instant::now() < deadline, "cache fixture GPU completion");
        std::thread::sleep(Duration::from_millis(1));
    }
    let pixels = crate::native::tests::pixels::readback(
        &renderer.backend.native.as_ref().unwrap().device,
        &resources.outputs[&OutputId::new(1).unwrap()],
    );
    let mut events = Vec::new();
    renderer
        .backend
        .control
        .drain_diagnostics(&mut events, NonZeroUsize::new(1024).unwrap());
    let work = events
        .iter()
        .find_map(|event| match event {
            DiagnosticEvent::FrameCpu(row) if row.token == token => Some((row.rendered_passes, row.reused_passes)),
            _ => None,
        })
        .unwrap();
    let mut outputs = events.iter().filter_map(|event| match event {
        DiagnosticEvent::Output(row) if row.token == token => Some(*row),
        _ => None,
    });
    let output = outputs.next().unwrap();
    assert!(outputs.next().is_none(), "one due SDK delivery per submission");
    assert_eq!(output.outcome, DiagnosticDelivery::Accepted);
    assert!(output.consumer_api.is_some());
    assert!(output.spout_image.is_some(), "actual versioned SDK image was sent");
    if output.conversion_reused {
        assert!(output.conversion.is_none());
    }
    (pixels, work, output)
}

fn install(renderer: &mut Renderer, plan: CompiledPlan) -> (D3d11Resources, CompiledPlan) {
    let resources = renderer
        .backend
        .prepare(
            &plan,
            super::super::generation(&renderer.backend, &plan, renderer.device),
        )
        .unwrap();
    let previous = renderer.resources.replace(resources).unwrap();
    let previous_plan = std::mem::replace(&mut renderer.plan, plan);
    (previous, previous_plan)
}

fn operation_scene(image: &PreparedImageFixture, operation: RenderOperation) -> CompiledPlan {
    RenderPlan {
        external_textures: vec![ExternalTexture {
            id: TextureId::new(1).unwrap(),
            descriptor: image.asset.value().descriptor(),
            source: Some(ExternalSource::Image(image.asset.clone())),
        }],
        passes: vec![RenderPass {
            id: PassId::new(1).unwrap(),
            output: TextureId::new(2).unwrap(),
            descriptor: descriptor(2, ColorEncoding::Linear, AlphaMode::Premultiplied),
            operation,
        }],
        outputs: vec![PlanOutput {
            id: OutputId::new(1).unwrap(),
            source: TextureId::new(2).unwrap(),
        }],
    }
    .compile(&PlanLimits {
        max_passes: 4,
        max_external_textures: 4,
        max_outputs: 4,
        max_dependencies: 16,
        max_texture_dimension: 8,
        max_texture_bytes: 65536,
    })
    .unwrap()
}

#[test]
#[ignore = "explicit Intel native cache pixels, actual command counts, temporal/live invalidation and retirement"]
fn hardware_intel_render_cache_preserves_pixels_live_changes_and_shared_target_ownership() {
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|gpu| gpu.vendor_id == 0x8086 && !gpu.software)
        .unwrap();
    let desc = descriptor(2, ColorEncoding::Srgb, AlphaMode::Straight);
    let red = prepared_image(desc, &[255, 0, 0, 255, 255, 0, 0, 255]);
    let green = prepared_image(desc, &[0, 255, 0, 255, 0, 255, 0, 255]);
    let mut renderer = Renderer::with_diagnostics(
        adapter.id,
        &red,
        2,
        Some(DiagnosticsConfig {
            capacity: NonZeroUsize::new(1024).unwrap(),
        }),
    );
    let first = frame(&mut renderer, time(), false);
    expect(&first.0, &[[255, 0, 0, 255]; 2]);
    assert_eq!(first.1, (1, 0));
    assert!(!first.2.conversion_reused);
    let held = frame(&mut renderer, time(), false);
    expect(&held.0, &first.0);
    assert_eq!(held.1, (0, 1));
    assert!(held.2.conversion_reused);
    let first_image = first.2.spout_image.unwrap();
    let held_image = held.2.spout_image.unwrap();
    assert_eq!(held_image.shared_handle, first_image.shared_handle);
    assert_eq!(
        held_image.frame,
        first_image.frame + 1,
        "reuse preserves the due SDK send"
    );

    // This generation writes only the converted target, leaving the old pass untouched.
    let direct_plan = RenderPlan {
        external_textures: vec![ExternalTexture {
            id: TextureId::new(1).unwrap(),
            descriptor: green.asset.value().descriptor(),
            source: Some(ExternalSource::Image(green.asset.clone())),
        }],
        passes: vec![],
        outputs: vec![PlanOutput {
            id: OutputId::new(1).unwrap(),
            source: TextureId::new(1).unwrap(),
        }],
    }
    .compile(&PlanLimits {
        max_passes: 1,
        max_external_textures: 1,
        max_outputs: 1,
        max_dependencies: 1,
        max_texture_dimension: 8,
        max_texture_bytes: 65536,
    })
    .unwrap();
    let (old, old_plan) = install(&mut renderer, direct_plan);
    assert!(Rc::ptr_eq(
        &old.outputs[&OutputId::new(1).unwrap()],
        &renderer.resources.as_ref().unwrap().outputs[&OutputId::new(1).unwrap()]
    ));
    let direct = frame(&mut renderer, time(), false);
    expect(&direct.0, &[[0, 255, 0, 255]; 2]);
    assert_eq!(direct.1, (0, 0));
    assert!(!direct.2.conversion_reused);
    let held_direct = frame(&mut renderer, time(), false);
    expect(&held_direct.0, &direct.0);
    assert!(held_direct.2.conversion_reused);
    let replaced = renderer.resources.replace(old);
    renderer.plan = old_plan;
    let restored_conversion = frame(&mut renderer, time(), false);
    expect(&restored_conversion.0, &first.0);
    assert_eq!(restored_conversion.1, (0, 1));
    assert!(
        !restored_conversion.2.conversion_reused,
        "shared converted target was overwritten"
    );
    drop(replaced);

    let (old, old_plan) = install(&mut renderer, scene(&green, 2));
    assert!(Rc::ptr_eq(
        &old.textures[&TextureId::new(2).unwrap()],
        &renderer.resources.as_ref().unwrap().textures[&TextureId::new(2).unwrap()]
    ));
    expect(&frame(&mut renderer, time(), false).0, &[[0, 255, 0, 255]; 2]);
    let replaced = renderer.resources.replace(old);
    renderer.plan = old_plan;
    let restored = frame(&mut renderer, time(), false);
    expect(&restored.0, &first.0);
    assert_eq!(
        restored.1,
        (1, 0),
        "another generation's write invalidates the cached target"
    );
    drop(replaced);

    let stream = ImageStream::new(desc);
    stream
        .publish(Some(ImageData::new(desc, [0, 0, 255, 255].repeat(2)).unwrap()))
        .unwrap();
    super::streams::install(&mut renderer, stream.clone());
    expect(&frame(&mut renderer, time(), false).0, &[[0, 0, 255, 255]; 2]);
    assert_eq!(frame(&mut renderer, time(), false).1, (0, 1));
    stream
        .publish(Some(ImageData::new(desc, [0, 255, 0, 128].repeat(2)).unwrap()))
        .unwrap();
    let changed = frame(&mut renderer, time(), false);
    expect(&changed.0, &[[0, 128, 0, 128]; 2]);
    assert_eq!(changed.1, (1, 0));
    assert!(!changed.2.conversion_reused);
    stream.publish(None).unwrap();
    expect(&frame(&mut renderer, time(), false).0, &[[0; 4]; 2]);

    #[cfg(feature = "ffmpeg")]
    {
        use crate::native::frames::tests::{completed, prepared, submit};
        let budget = crate::native::GpuFrameBudget::new(1024 * 1024);
        let (mut publisher, source) = prepared(adapter.id, crate::decode::D3d11Storage::Nv12, 0, 126, &budget);
        let stream = publisher.stream();
        super::gpu_streams::install(&mut renderer, stream.clone());
        assert!(submit(&mut publisher, &source));
        stream.publish(Some(completed(&mut publisher))).unwrap();
        let copied = frame(&mut renderer, time(), false);
        assert_eq!(copied.1, (1, 0));
        let gray = ((((110.0_f64 / 219.0 + 0.099) / 1.099).powf(1.0 / 0.45)) * 255.0).round() as u8;
        expect(&copied.0, &[[gray, gray, gray, 255]; 2]);
        let held = frame(&mut renderer, time(), false);
        expect(&held.0, &copied.0);
        assert_eq!(held.1, (0, 1));
        assert!(held.2.conversion_reused);
        assert!(submit(&mut publisher, &source));
        stream.publish(Some(completed(&mut publisher))).unwrap();
        assert_eq!(frame(&mut renderer, time(), false).1, (1, 0));
        stream.publish(None).unwrap();
        let withdrawn = frame(&mut renderer, time(), false);
        expect(&withdrawn.0, &[[0; 4]; 2]);
        assert_eq!(withdrawn.1, (1, 0));
        drop(publisher);
        // Drop publishes a new withdrawal. Its next submission clears and prunes retired imports.
        let retired = frame(&mut renderer, time(), false);
        expect(&retired.0, &[[0; 4]; 2]);
        assert_eq!(retired.1, (1, 0));
        assert_eq!(budget.retained_bytes(), 0);
    }

    let anchor = time();
    let envelope = OpacityEnvelope::new(
        anchor,
        Duration::from_secs(1),
        crate::Opacity::TRANSPARENT,
        crate::Opacity::OPAQUE,
        crate::plan::InterpolationCurve::Linear,
    )
    .unwrap();
    let fade = operation_scene(
        &red,
        RenderOperation::Composite {
            layers: vec![Layer {
                source: TextureId::new(1).unwrap().into(),
                opacity: crate::Opacity::OPAQUE,
                opacity_envelope: Some(envelope),
                blend: BlendMode::Normal,
                filter: SampleFilter::Nearest,
                mapping: None,
            }],
        },
    );
    drop(install(&mut renderer, fade));
    for (milliseconds, rgba) in [(0, [0; 4]), (500, [128, 0, 0, 128]), (1000, [255, 0, 0, 255])] {
        let at = anchor
            .at_monotonic(
                anchor
                    .monotonic()
                    .checked_add(Duration::from_millis(milliseconds))
                    .unwrap(),
            )
            .unwrap();
        let actual = frame(&mut renderer, at, false);
        expect(&actual.0, &[rgba; 2]);
        assert_eq!(actual.1, (1, 0));
        assert!(!actual.2.conversion_reused);
    }

    let placement = RegionPlacement::new(UnitRect::FULL, UnitRect::FULL).unwrap();
    let visibility = golden_io::RevisionSource::default();
    let reader = visibility.reader();
    let regions = operation_scene(
        &red,
        RenderOperation::Regions {
            regions: vec![BlitRegion {
                source: TextureId::new(1).unwrap(),
                destination: UnitRect::FULL,
                source_rect: UnitRect::FULL,
                filter: SampleFilter::Nearest,
                visibility: None,
                placement: Some(placement.clone()),
            }],
            visibility: Some(reader.guard_at(reader.current())),
        },
    );
    drop(install(&mut renderer, regions));
    expect(&frame(&mut renderer, time(), false).0, &[[255, 0, 0, 255]; 2]);
    assert!(placement.update(
        UnitRect {
            width: 0.5,
            ..UnitRect::FULL
        },
        UnitRect::FULL
    ));
    expect(&frame(&mut renderer, time(), true).0, &[[255, 0, 0, 255], [0; 4]]);
    visibility.advance();
    expect(&frame(&mut renderer, time(), false).0, &[[0; 4]; 2]);
    let control = renderer.backend.control.clone();
    drop(renderer);
    assert!(!control.snapshot().initialized);
    assert_eq!(control.allocations_snapshot().plan.live, 0);
    assert_eq!(control.diagnostics_snapshot().dropped_full, 0);
}
