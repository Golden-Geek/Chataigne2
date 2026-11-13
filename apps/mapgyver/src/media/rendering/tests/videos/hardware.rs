use super::*;
use golden_video::{
    native::{GpuFrameBudget, adapters},
    plan::ExternalSource,
};

#[test]
#[ignore = "requires Intel D3D11 decoding on the local Windows machine"]
fn hardware_file_worker_steps_exactly_and_falls_back_without_losing_its_clock() {
    let intel = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| adapter.vendor_id == 0x8086 && !adapter.software)
        .unwrap();
    for (name, capacity, resident, gpu_publication) in [
        ("h264.mp4", 64 * 1024 * 1024, true, true),
        ("h264.mp4", 0, false, false),
        ("prores.mov", 64 * 1024 * 1024, false, true),
    ] {
        let mut engine = engine();
        let reference = source(&mut engine, name);
        let budget = GpuFrameBudget::new(capacity);
        let mut renderer = renderer();
        renderer
            .enable_hardware_videos(&fixture_root(), None, intel.id, budget.clone())
            .unwrap();
        renderer
            .publish(capture_channels(&engine, &[reference], 8).unwrap(), routes(reference))
            .unwrap();
        let mut current = steps::at(&mut engine, reference, 0.0);
        wait(|| renderer.videos.as_ref().unwrap().source(reference).is_some());
        let (_, published) = renderer.videos.as_ref().unwrap().source(reference).unwrap();
        assert_eq!(
            matches!(published, ExternalSource::GpuFrames(_)),
            gpu_publication,
            "{name}, capacity{capacity}"
        );
        if resident {
            assert!(budget.retained_bytes() > 0);
            let pools = budget.snapshot();
            assert!(pools.decoder_pools.allocations > 0);
            assert!(pools.decoder_pools.bytes > 0);
            assert_eq!(pools.hardware_producers.allocations, 1);
            assert_eq!(pools.hap_producers.bytes, 0);
            assert_eq!(pools.software_producers.bytes, 0);
            assert_eq!(
                pools.retained_bytes,
                pools.decoder_pools.bytes + pools.hardware_producers.bytes
            );
            eprintln!("hardware source accounting: {pools:?}");
        } else {
            let pools = budget.snapshot();
            assert_eq!(pools.decoder_pools.bytes, 0);
            assert_eq!(pools.hardware_producers.bytes, 0);
            assert_eq!(pools.hap_producers.bytes, 0);
            assert_eq!(pools.software_producers.allocations, u64::from(gpu_publication));
            assert_eq!(pools.retained_bytes, pools.software_producers.bytes);
            if !gpu_publication {
                assert_eq!(pools.peak_bytes, 0, "zero budget avoids both source-GPU routes");
            }
            eprintln!("software source accounting: {pools:?}");
        }
        let mut timestamps = vec![current.position_ms];
        while let Some(next) = current.next_ms {
            assert!(timestamps.len() < 10);
            steps::step(
                &mut engine,
                reference,
                crate::media::video::protocol::VideoStepDirection::Next,
            );
            current = steps::at(&mut engine, reference, next);
            timestamps.push(current.position_ms);
        }
        assert_eq!(timestamps.len(), 6);
        for expected in timestamps[..5].iter().rev() {
            steps::step(
                &mut engine,
                reference,
                crate::media::video::protocol::VideoStepDirection::Previous,
            );
            current = steps::at(&mut engine, reference, *expected);
            assert!((current.position_ms - expected).abs() < 0.000001);
        }
        edit(
            &mut engine,
            MediaEdit::Video {
                media: reference.media,
                edit: VideoEdit::Seek { position_ms: 125.0 },
            },
        );
        steps::at(&mut engine, reference, if name == "prores.mov" { 120.0 } else { 125.0 });
        edit(
            &mut engine,
            MediaEdit::Video {
                media: reference.media,
                edit: VideoEdit::Stop,
            },
        );
        steps::at(&mut engine, reference, 0.0);
        let retirement = renderer.video_retirement().unwrap();
        renderer.close();
        drop(renderer);
        drop(published);
        assert!(
            retirement.wait(Duration::from_secs(5)),
            "{name}: complete worker and pool retirement"
        );
        assert_eq!(budget.retained_bytes(), 0);
        eprintln!(
            "file worker verified {name}: resident={resident}, gpu_publication={gpu_publication}, {timestamps:?}, retired=0"
        );
    }
}

#[test]
#[ignore = "requires both local GPUs and MAPGYVER_HAP_EXTERNAL with official 4K FFmpeg samples"]
fn hardware_hap_three_4k_files_share_six_consumers_without_duplicate_workers_and_retire() {
    let root = std::path::PathBuf::from(std::env::var_os("MAPGYVER_HAP_EXTERNAL").unwrap());
    for adapter in adapters().unwrap().into_iter().filter(|adapter| !adapter.software) {
        let mut engine = engine();
        let mut originals = Vec::new();
        let mut pairs = Vec::new();
        for name in ["Hap_ffmpeg_64.mov", "Hap_Alpha_ffmpeg_6.mov", "Hap_Q_ffmpeg_2.mov"] {
            let reference = source(&mut engine, &root.join(name).to_string_lossy());
            resize(&mut engine, reference, 64);
            let MediaEditResult::ReferenceCreated { media } = edit(
                &mut engine,
                MediaEdit::CreateReference {
                    target: Some(reference.media),
                    label: None,
                },
            ) else {
                panic!()
            };
            let alias = crate::media::resolve_source_node(&engine, media.0).unwrap();
            originals.push(reference);
            pairs.push((reference, alias));
        }
        let references: Vec<_> = pairs.iter().flat_map(|(source, alias)| [*source, *alias]).collect();
        let routes: Vec<_> = references
            .iter()
            .enumerate()
            .map(|(index, channel)| ChannelRoute {
                output: OutputId::new(index as u64 + 1).unwrap(),
                channel: *channel,
            })
            .collect();
        let mut config = config();
        // Three 4096x2160 external RGBA descriptors exceed the small ordinary unit fixture.
        // This is below the existing production source cap; no production limit is changed.
        config.plan_limits.max_texture_bytes = 256 * 1024 * 1024;
        let mut renderer = ChannelRenderer::new(
            VideoRuntime::spawn_null(config.clone(), NullConfig::default()).unwrap(),
            config.plan_limits,
        );
        let budget = GpuFrameBudget::new(512 * 1024 * 1024);
        renderer
            .enable_hardware_videos(&root, None, adapter.id, budget.clone())
            .unwrap();
        renderer
            .publish(capture_channels(&engine, &references, 16).unwrap(), routes)
            .unwrap();
        let retirement = renderer.video_retirement().unwrap();
        for reference in &originals {
            steps::at(&mut engine, *reference, 0.0);
            wait(|| renderer.videos.as_ref().unwrap().source(*reference).is_some());
            let (descriptor, source) = renderer.videos.as_ref().unwrap().source(*reference).unwrap();
            assert_eq!(descriptor.extent, Extent2D::new(4096, 2160).unwrap());
            assert!(matches!(source, ExternalSource::GpuFrames(_)));
        }
        wait(|| {
            renderer.refresh().unwrap();
            renderer.channels().len() == 6
                && renderer.channels().iter().all(|channel| {
                    channel.resolution.availability == crate::media::resolution::ChannelAvailability::Ready
                })
        });
        for (source, alias) in &pairs {
            let target = |reference| {
                renderer
                    .channels()
                    .iter()
                    .find(|channel| channel.reference == reference)
                    .unwrap()
                    .target
            };
            assert_eq!(
                target(*source),
                target(*alias),
                "shared consumers have one render target"
            );
        }
        assert_eq!(
            retirement.active_workers(),
            3,
            "six consumers own exactly three decoders"
        );
        let admitted = budget.retained_bytes();
        assert!(admitted > 300 * 1024 * 1024 && admitted < 512 * 1024 * 1024);
        let pools = retirement.gpu_allocations().unwrap();
        assert_eq!(pools.hap_producers.allocations, 3);
        assert_eq!(pools.hap_producers.bytes, admitted);
        assert_eq!(pools.decoder_pools.bytes, 0);
        assert_eq!(pools.hardware_producers.bytes, 0);
        assert_eq!(pools.software_producers.bytes, 0);
        eprintln!("HAP source accounting: {pools:?}");
        let request = renderer.latest_request();
        for reference in &originals {
            edit(
                &mut engine,
                MediaEdit::Video {
                    media: reference.media,
                    edit: VideoEdit::Loop { looping: true },
                },
            );
            edit(
                &mut engine,
                MediaEdit::Video {
                    media: reference.media,
                    edit: VideoEdit::Play,
                },
            );
        }
        let mut positions = vec![std::collections::HashSet::new(); 3];
        wait(|| {
            for (index, reference) in originals.iter().enumerate() {
                let MediaEditResult::VideoFrame { frame } =
                    edit(&mut engine, MediaEdit::InspectVideoFrame { reference: *reference })
                else {
                    panic!()
                };
                if let Some(frame) = frame {
                    positions[index].insert((frame.position_ms * 1000.0).round() as u64);
                }
            }
            positions.iter().all(|values| values.len() >= 12)
        });
        assert_eq!(retirement.active_workers(), 3);
        assert_eq!(
            budget.retained_bytes(),
            admitted,
            "playing does not grow the fixed native pools"
        );
        assert!(renderer.refresh().unwrap().is_none(), "new frames do not rebuild plans");
        assert_eq!(renderer.latest_request(), request);
        renderer.close();
        drop(renderer);
        assert!(
            retirement.wait(Duration::from_secs(10)),
            "all three GPU file workers retire"
        );
        assert_eq!(budget.retained_bytes(), 0);
        assert_eq!(retirement.retained_bytes(), 0);
        assert_eq!(retirement.gpu_allocations().unwrap().hap_producers.allocations, 0);
        eprintln!(
            "HAP {}: three concurrent 4096x2160 files, six consumers, three owners, stable {admitted} GPU bytes, 12+ distinct frames each, complete retirement",
            adapter.description
        );
    }
}
