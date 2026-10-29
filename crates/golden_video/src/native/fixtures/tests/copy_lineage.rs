use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use uuid::Uuid;

use super::super::{CardStamp, ConsumerFrame, ConsumerTick, FixtureConsumer, FixtureConsumerConfig};
use super::super::{FixtureSource, FixtureSourceConfig, FixtureSubmission};
use crate::native::timing::{DiagnosticDelivery, DiagnosticsConfig, Recorder, SpoutImageVersion};
use crate::native::{AdapterId, adapters};
use crate::plan::SpoutFrameCount;
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor};

const WAIT: Duration = Duration::from_secs(6);

mod bridge;
mod keyed_access;
mod runtime_capture;

struct Captures {
    frames: Vec<ConsumerFrame>,
    source_images: Option<BTreeMap<(u32, i32), Option<SpoutImageVersion>>>,
    stages: Option<BTreeMap<(u32, i32), [u32; 12]>>,
}

impl Captures {
    fn direct(frames: Vec<ConsumerFrame>) -> Self {
        Self {
            frames,
            source_images: None,
            stages: None,
        }
    }
}

enum Boundary {
    Sdk,
    Bridge,
    Runtime,
    RuntimeTapped,
}

enum Capture {
    Sdk(Box<FixtureConsumer>),
    Bridge(Box<bridge::BridgeCapture>),
    Runtime(Box<runtime_capture::RuntimeCapture>),
}

impl Capture {
    fn tick(&mut self) {
        match self {
            Self::Sdk(consumer) => {
                consumer.tick().unwrap();
            }
            Self::Bridge(consumer) => consumer.tick(),
            Self::Runtime(consumer) => consumer.tick(),
        }
    }
    fn len(&self) -> usize {
        match self {
            Self::Sdk(consumer) => consumer.snapshot().logged_frames as usize,
            Self::Bridge(consumer) => consumer.len(),
            Self::Runtime(consumer) => consumer.len(),
        }
    }
    fn finish(&mut self) -> Captures {
        match self {
            Self::Sdk(consumer) => Captures::direct(consumer.finish().unwrap()),
            Self::Bridge(consumer) => Captures::direct(consumer.finish()),
            Self::Runtime(consumer) => consumer.finish(),
        }
    }
}

fn wait(label: &str, mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !ready() {
        assert!(Instant::now() < deadline, "timed out: {label}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn diagnostics() -> DiagnosticsConfig {
    DiagnosticsConfig {
        capacity: NonZeroUsize::new(4096).unwrap(),
    }
}

fn descriptor() -> TextureDescriptor {
    TextureDescriptor {
        extent: Extent2D::new(3840, 2160).unwrap(),
        format: PixelFormat::Bgra8Unorm,
        color_encoding: ColorEncoding::Srgb,
        alpha_mode: AlphaMode::Straight,
    }
}

fn source(adapter: AdapterId, name: &str) -> FixtureSource {
    FixtureSource::new(FixtureSourceConfig {
        adapter,
        descriptor: descriptor(),
        source_id: 1,
        instance: Uuid::new_v4(),
        sender_name: name.into(),
        diagnostics: diagnostics(),
    })
    .unwrap()
}

fn consumer(adapter: AdapterId, name: &str) -> FixtureConsumer {
    FixtureConsumer::with_marker_shader(
        FixtureConsumerConfig {
            adapter,
            sender_name: name.into(),
            frame_count: SpoutFrameCount::VersionedV1,
            descriptor: descriptor(),
            max_frames: 256,
            instance: Uuid::new_v4(),
            diagnostics: diagnostics(),
        },
        include_str!("single_source.hlsl"),
    )
    .unwrap()
}

fn stamp(clock: &Recorder, frame: u64) -> CardStamp {
    CardStamp {
        source: 1,
        frame,
        qpc: clock.stamp().unwrap(),
    }
}

fn collect(captured: Captures, submissions: &[FixtureSubmission]) -> serde_json::Value {
    let sent = submissions
        .iter()
        .filter_map(|submission| {
            submission
                .spout_image
                .map(|image| ((image.shared_handle, image.frame), submission))
        })
        .collect::<BTreeMap<_, _>>();
    let mut mismatches = Vec::new();
    let mut unavailable = 0;
    let mut input_mismatches = 0;
    let mut converted_mismatches = 0;
    let mut output_mismatches = 0;
    for frame in &captured.frames {
        let image = frame.spout_image.unwrap();
        let source_image = match &captured.source_images {
            Some(images) => *images
                .get(&(image.shared_handle, image.frame))
                .expect("captured output has a native record"),
            None => Some(image),
        };
        let Some(source_image) = source_image else {
            unavailable += 1;
            continue;
        };
        let expected = sent
            .get(&(source_image.shared_handle, source_image.frame))
            .expect("copied source version has a sender record");
        assert_eq!(expected.outcome, DiagnosticDelivery::Accepted);
        let words = expected.stamp.words();
        if let Some(stages) = &captured.stages {
            let stages = stages.get(&(image.shared_handle, image.frame)).unwrap();
            input_mismatches += usize::from(stages[..6] != words);
            converted_mismatches += usize::from(stages[6..] != stages[..6]);
            output_mismatches +=
                usize::from(frame.raw_words[..6] != stages[6..] || frame.raw_words[6..] != stages[6..]);
        }
        if frame.left != Ok(expected.stamp)
            || frame.right != Ok(expected.stamp)
            || frame.raw_words[..6] != words
            || frame.raw_words[6..] != words
        {
            mismatches.push(serde_json::json!({
                "copy": image, "row": frame.row, "expected": expected.stamp,
                "left": frame.left, "right": frame.right, "raw_words": frame.raw_words,
            }));
        }
    }
    serde_json::json!({
        "submitted": submissions.len(), "accepted": sent.len(), "captured": captured.frames.len(),
        "unavailable_source": unavailable, "compared": captured.frames.len() - unavailable,
        "input_stage_mismatches": captured.stages.as_ref().map(|_| input_mismatches),
        "conversion_stage_mismatches": captured.stages.as_ref().map(|_| converted_mismatches),
        "output_stage_mismatches": captured.stages.as_ref().map(|_| output_mismatches),
        "mismatches": mismatches,
    })
}

fn fenced(adapter: AdapterId) -> serde_json::Value {
    let name = format!("GoldenCopyFenced-{}", Uuid::new_v4());
    let mut source = source(adapter, &name);
    let mut consumer = consumer(adapter, &name);
    let clock = Recorder::new(diagnostics());
    let mut sent = Vec::new();
    for frame in 1..=24 {
        let submission = source.submit(stamp(&clock, frame)).unwrap().unwrap();
        assert_eq!(submission.outcome, DiagnosticDelivery::Accepted);
        wait("producer copy completion", || {
            source.poll().unwrap().serial >= submission.token.serial
        });
        sent.push(submission);
        let mut captured = None;
        wait("capture after producer completion", || {
            if let ConsumerTick::Logged { token, .. } = consumer.tick().unwrap() {
                captured = Some(token);
                true
            } else {
                false
            }
        });
        wait("consumer copy/decode completion", || {
            consumer.poll().unwrap();
            consumer.snapshot().last_completed.serial >= captured.unwrap().serial
        });
    }
    source.finish().unwrap();
    let frames = consumer.finish().unwrap();
    assert_eq!(frames.len(), 24);
    collect(Captures::direct(frames), &sent)
}

fn concurrent(adapter: AdapterId, boundary: Boundary) -> serde_json::Value {
    std::thread::scope(|scope| {
        let name = format!("GoldenCopyConcurrent-{}", Uuid::new_v4());
        let (ready_tx, ready_rx) = mpsc::channel();
        let (start_tx, start_rx) = mpsc::channel();
        let (report_tx, report_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let producer_name = name.clone();
        scope.spawn(move || {
            let mut source = source(adapter, &producer_name);
            let clock = Recorder::new(diagnostics());
            let first = source.submit(stamp(&clock, 1)).unwrap().unwrap();
            wait("initial producer completion", || {
                source.poll().unwrap().serial >= first.token.serial
            });
            let mut sent = vec![first];
            ready_tx.send(()).unwrap();
            if start_rx.recv().is_ok() {
                let anchor = Instant::now();
                for frame in 2..=121 {
                    let due = anchor + Duration::from_nanos((frame - 2) * 1_000_000_000 / 60);
                    std::thread::sleep(due.saturating_duration_since(Instant::now()));
                    if let Some(submission) = source.submit(stamp(&clock, frame)).unwrap() {
                        sent.push(submission);
                    }
                }
            }
            source.finish().unwrap();
            let _ = report_tx.send(sent);
            // Retain the exact sender while the independent consumer drains and maps its log.
            let _ = release_rx.recv();
        });
        ready_rx.recv_timeout(WAIT).unwrap();
        let mut consumer = match boundary {
            Boundary::Bridge => Capture::Bridge(Box::new(bridge::BridgeCapture::new(adapter, &name))),
            Boundary::Sdk => Capture::Sdk(Box::new(consumer(adapter, &name))),
            Boundary::Runtime => {
                Capture::Runtime(Box::new(runtime_capture::RuntimeCapture::new(adapter, &name, false)))
            }
            Boundary::RuntimeTapped => {
                Capture::Runtime(Box::new(runtime_capture::RuntimeCapture::new(adapter, &name, true)))
            }
        };
        wait("initial static capture", || {
            consumer.tick();
            consumer.len() != 0
        });
        start_tx.send(()).unwrap();
        let mut sent = None;
        let mut end = None;
        let deadline = Instant::now() + WAIT;
        loop {
            consumer.tick();
            if sent.is_none()
                && let Ok(report) = report_rx.try_recv()
            {
                sent = Some(report);
                end = Some(Instant::now() + Duration::from_millis(100));
            }
            if end.is_some_and(|end| Instant::now() >= end) {
                break;
            }
            assert!(Instant::now() < deadline, "finite producer did not complete");
            std::thread::sleep(Duration::from_millis(1));
        }
        let captured = consumer.finish();
        let report = collect(captured, &sent.unwrap());
        assert!(
            report["compared"].as_u64().unwrap() >= 20,
            "insufficient actual dynamic captures"
        );
        drop(consumer);
        release_tx.send(()).unwrap();
        report
    })
}

#[test]
#[ignore = "requires hardware D3D11 and two caller-owned native Spout devices"]
fn hardware_native_copy_lineage_preserves_completed_gpu_markers() {
    let (done, stop) = mpsc::channel::<()>();
    std::thread::spawn(move || {
        if matches!(
            stop.recv_timeout(Duration::from_secs(60)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("direct copy-lineage watchdog expired before actual native retirement");
            std::process::exit(1);
        }
    });
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| !adapter.software)
        .unwrap();
    let baseline = fenced(adapter.id);
    let keyed_access = keyed_access::verify(adapter.id);
    let dynamic = concurrent(adapter.id, Boundary::Sdk);
    let bridge = concurrent(adapter.id, Boundary::Bridge);
    let runtime = concurrent(adapter.id, Boundary::Runtime);
    let tapped = concurrent(adapter.id, Boundary::RuntimeTapped);
    println!(
        "COPY_LINEAGE {}",
        serde_json::json!({"adapter": adapter.id, "fenced": baseline, "keyed_access": keyed_access, "concurrent": dynamic, "bridge": bridge, "runtime": runtime, "tapped": tapped})
    );
    assert!(
        baseline["mismatches"].as_array().unwrap().is_empty(),
        "completed producer baseline is incoherent"
    );
    assert!(
        dynamic["mismatches"].as_array().unwrap().is_empty(),
        "concurrent SDK copy labels differ from GPU pixels"
    );
    assert!(
        bridge["mismatches"].as_array().unwrap().is_empty(),
        "source bridge labels differ from GPU pixels"
    );
    assert!(
        runtime["mismatches"].as_array().unwrap().is_empty(),
        "production runtime labels differ from GPU pixels"
    );
    assert!(
        tapped["mismatches"].as_array().unwrap().is_empty(),
        "tapped production runtime labels differ from GPU pixels"
    );
    done.send(()).unwrap();
}
