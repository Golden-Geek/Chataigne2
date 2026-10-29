use std::num::NonZeroUsize;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use windows::core::{Error, HRESULT, Result};

use super::pipeline_helper::Pipeline;
use super::{CardStamp, MarkerError, marker_hlsl};
use crate::native::AdapterId;
use crate::native::completion::EventCompletion;
use crate::native::device::Device;
use crate::native::inputs::receiver::{Received, Receiver};
use crate::native::texture::Texture;
use crate::native::timing::{
    CpuScope, DiagnosticContext, DiagnosticEvent, DiagnosticsConfig, DiagnosticsSnapshot, GpuTiming, Recorder,
    SourceCpuTrace,
};
use crate::runtime::{CompletionToken, DeviceGeneration};
use crate::{ColorEncoding, TextureDescriptor};
use log::GpuLog;

mod log;

const INVALID_ARGUMENT: HRESULT = HRESULT(0x80070057_u32 as i32);

/// Exact independent receiver and its append-only GPU marker-log reservation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FixtureConsumerConfig {
    /// Explicit hardware adapter; no fallback or software device.
    pub adapter: AdapterId,
    /// Exact renderer Spout sender identity.
    pub sender_name: String,
    /// Caller-declared counter contract; owned qualification peers use VersionedV1.
    pub frame_count: crate::plan::SpoutFrameCount,
    /// Immutable expected sRGB UNORM target layout; actual extent must match exactly.
    pub descriptor: TextureDescriptor,
    /// Maximum GPU log rows, from one through 65,536, tiled in pages at most 16,384 high.
    /// Full logs never overwrite rows.
    pub max_frames: u32,
    /// Independent consumer/device lifetime.
    pub instance: Uuid,
    /// Fixed telemetry ring capacity, at most 65,536 rows.
    pub diagnostics: DiagnosticsConfig,
}

/// One receiver tick, without CPU pixel access or a blocking GPU wait.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ConsumerTick {
    /// An actual SDK image copy was submitted and its marker decode queued into one GPU row.
    Logged {
        /// Append-only zero-based log row.
        row: u32,
        /// Consumer GPU batch covering capture and marker decode.
        token: CompletionToken,
    },
    /// Exact named sender is absent.
    Absent,
    /// SDK connection/update handshake has not yielded a frame.
    Waiting,
    /// An explicitly versioned sender counter was proven unchanged, without an image copy.
    Unchanged,
    /// Actual storage/extent differs from the immutable expected descriptor.
    Mismatch,
    /// Previous capture/log work or SDK texture access is busy; no copy is published.
    Busy,
    /// GPU log capacity is exhausted; overflow is counted without overwriting old evidence.
    Full,
}

#[derive(Clone, Copy)]
struct Record {
    token: CompletionToken,
    spout_image: Option<super::super::timing::SpoutImageVersion>,
    receive_begin_qpc: Option<u64>,
    completion_observed_qpc: Option<u64>,
}

#[derive(Default)]
struct Counters {
    busy: u64,
    receive_busy: u64,
    receive_copies: u64,
    last_copied_frame: Option<i32>,
    absent: u64,
    waiting: u64,
    unchanged: u64,
    mismatch: u64,
    overflow: u64,
    failed: u64,
}

/// One raw GPU decode and its independently checked side-by-side source markers. Corrupt
/// rows remain present with their exact raw words and validation errors.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsumerFrame {
    /// Zero-based append-only GPU log row.
    pub row: u32,
    /// Actual capture/decode completion lifetime.
    pub token: CompletionToken,
    /// Exact SDK image version copied before this GPU marker decode; absent when unknown.
    pub spout_image: Option<super::super::timing::SpoutImageVersion>,
    /// Before SDK receive; this is neither producer nor GPU time.
    pub receive_begin_qpc: Option<u64>,
    /// First caller poll that observed the real capture/decode event completed.
    pub completion_observed_qpc: Option<u64>,
    /// Exact twelve integer words read once after the measurement window.
    pub raw_words: [u32; 12],
    /// Left marker checksum result, without clamping or identity/order inference.
    pub left: std::result::Result<CardStamp, MarkerError>,
    /// Right marker checksum result, without clamping or identity/order inference.
    pub right: std::result::Result<CardStamp, MarkerError>,
}

/// Consumer ownership and admission counters. Texel charges are descriptor payloads;
/// aliases are foreign ownership and must not be double-counted as physical allocations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FixtureConsumerSnapshot {
    /// Consumer/process lifetime.
    pub instance: Uuid,
    /// Selected hardware adapter.
    pub adapter: AdapterId,
    /// Immutable expected received storage.
    pub descriptor: TextureDescriptor,
    /// Reserved append-only row capacity.
    pub max_frames: u32,
    /// Actual marker rows queued on the GPU.
    pub logged_frames: u32,
    /// SDK receive batches admitted, including unchanged/failed receive calls.
    pub submitted_batches: u64,
    /// Current exact GPU submission token, including the separately identified final readback.
    pub last_submitted: CompletionToken,
    /// Latest actually observed completed token.
    pub last_completed: CompletionToken,
    /// Receive ticks skipped because previous work or SDK access was busy.
    pub busy: u64,
    /// Detailed receives that submitted an actual SDK image copy, without a uniqueness claim.
    pub receive_copies: u64,
    /// Detailed receives that were SDK-access Busy, separate from GPU log admission.
    pub receive_busy: u64,
    /// Caller-declared frame-counter contract, never inferred from a semaphore.
    pub frame_count: crate::plan::SpoutFrameCount,
    /// Counter attached to the last actual copy; absent for unversioned/unknown peers.
    pub last_copied_frame: Option<i32>,
    /// Ticks observing no exact named sender.
    pub absent: u64,
    /// SDK connection/update handshakes without a frame.
    pub waiting: u64,
    /// Polls proven unchanged, including CPU preflight without a new GPU batch.
    pub unchanged: u64,
    /// Descriptor/storage mismatches.
    pub mismatch: u64,
    /// Admissions refused because append-only row capacity was exhausted.
    pub overflow: u64,
    /// Native receive/decode errors.
    pub failed_receives: u64,
    /// Owned capture texel payload bytes.
    pub capture_bytes: u64,
    /// Maximum opened foreign sender alias bytes, not an extra owned receiving allocation.
    pub alias_bytes: u64,
    /// Owned integer GPU log texel payload bytes.
    pub gpu_log_bytes: u64,
    /// Reserved final CPU-readable staging texel payload bytes.
    pub staging_bytes: u64,
    /// Owned capture, padded GPU-log and staging texel payload bytes; foreign aliases are excluded.
    pub admitted_texture_bytes: u64,
    /// Final post-window readback attempts, at most one.
    pub final_readbacks: u32,
    /// Exact final staging-copy token, separate from measured receive/decode batches.
    pub readback_token: Option<CompletionToken>,
    /// Whether finish has ended further tick/readback admission.
    pub finished: bool,
    /// Whether the owning device has actually reported removal.
    pub device_removed: bool,
}

/// Worker-local independent GPU consumer. No pixel data is mapped during tick/poll. Drop
/// retains SDK aliases, capture and log through actual completion or confirmed device removal.
pub struct FixtureConsumer {
    receiver: Receiver,
    capture: Texture,
    log: GpuLog,
    pipeline: Pipeline,
    timing: GpuTiming,
    completion: EventCompletion,
    recorder: Recorder,
    device: Device,
    config: FixtureConsumerConfig,
    generation: DeviceGeneration,
    submitted: u64,
    completed: u64,
    receive_batches: u64,
    records: Vec<Record>,
    pending_record: Option<usize>,
    counters: Counters,
    finished: bool,
    readbacks: u32,
    readback_token: Option<CompletionToken>,
    removed: bool,
}

impl FixtureConsumer {
    /// Allocates every shader/query/capture/log/staging object before measurement. It does
    /// not receive pixels, clear/map textures, or submit native work during construction.
    pub fn new(config: FixtureConsumerConfig) -> Result<Self> {
        if config.max_frames == 0
            || config.max_frames > 65536
            || config.diagnostics.capacity.get() > 65536
            || config.descriptor.color_encoding != ColorEncoding::Srgb
            || config.descriptor.extent.width() > 16384
            || config.descriptor.extent.height() > 16384
        {
            return Err(Error::from_hresult(INVALID_ARGUMENT));
        }
        let device = Device::new(config.adapter)?;
        let recorder = Recorder::new(config.diagnostics);
        let generation = DeviceGeneration::new(config.instance, 0);
        let context = DiagnosticContext::FixtureConsumer { id: config.instance };
        let pipeline = Pipeline::new(
            &device.device,
            &format!("{}\n{}", marker_hlsl(), include_str!("consumer.hlsl")),
            0,
        )?;
        let capture = Texture::new(&device.device, config.descriptor)?;
        let log = GpuLog::new(&device.device, config.max_frames)?;
        let completion = EventCompletion::new(&device)?;
        let timing = GpuTiming::new(&device.device, recorder.clone(), context);
        let receiver = Receiver::new(
            &device.device,
            &config.sender_name,
            config.descriptor,
            config.frame_count,
        )?;
        let records = Vec::with_capacity(config.max_frames as usize);
        Ok(Self {
            receiver,
            capture,
            log,
            pipeline,
            timing,
            completion,
            recorder,
            device,
            config,
            generation,
            submitted: 0,
            completed: 0,
            receive_batches: 0,
            records,
            pending_record: None,
            counters: Counters::default(),
            finished: false,
            readbacks: 0,
            readback_token: None,
            removed: false,
        })
    }

    /// Attempts one capture/decode. The caller schedules ticks; no preflight, receive or SDK
    /// alias replacement is allowed until the previous actual capture/log batch completes.
    pub fn tick(&mut self) -> Result<ConsumerTick> {
        if self.finished {
            return Err(Error::from_hresult(INVALID_ARGUMENT));
        }
        self.poll()?;
        if self.completed < self.submitted {
            return Ok(self.count(ConsumerTick::Busy));
        }
        if self.records.len() == self.config.max_frames as usize {
            return Ok(self.count(ConsumerTick::Full));
        }
        let preflight = match self.receiver.preflight() {
            Ok(preflight) => preflight,
            Err(error) => {
                self.counters.failed = self.counters.failed.saturating_add(1);
                return Err(error);
            }
        };
        if let Some(result) = preflight {
            return Ok(self.count(classify(result)));
        }
        if self.receiver.descriptor() != Some(self.config.descriptor) {
            self.receiver.disconnect();
            return Ok(self.count(ConsumerTick::Mismatch));
        }
        let token = CompletionToken {
            device: self.generation,
            serial: self
                .submitted
                .checked_add(1)
                .ok_or_else(|| Error::from_hresult(INVALID_ARGUMENT))?,
        };
        self.submitted = token.serial;
        self.receive_batches = self.receive_batches.saturating_add(1);
        let cpu = CpuScope::new(&self.recorder);
        let completion = self.completion.begin(&self.device, token);
        let gpu = self.timing.begin(&self.device.context, token);
        let receive_begin_qpc = self.recorder.stamp();
        let received = self.receiver.receive(&self.capture);
        let receive_api = self.recorder.interval(receive_begin_qpc);
        match &received {
            Ok(Received::Frame) => {
                self.counters.receive_copies = self.counters.receive_copies.saturating_add(1);
                self.counters.last_copied_frame = self.receiver.last_copied_frame();
            }
            Ok(Received::Busy) => self.counters.receive_busy = self.counters.receive_busy.saturating_add(1),
            _ => {}
        }
        let bridge_begin = self.recorder.stamp();
        let result = match received {
            Ok(Received::Frame) => {
                let row = self.records.len() as u32;
                self.pipeline
                    .draw_region(
                        &self.device.context,
                        &self.log.target,
                        self.log.origin(row),
                        [3, 1],
                        &[Some(self.capture.srv.clone())],
                        &[],
                    )
                    .map(|()| {
                        self.records.push(Record {
                            token,
                            spout_image: self.receiver.copied_image_version(),
                            receive_begin_qpc,
                            completion_observed_qpc: None,
                        });
                        self.pending_record = Some(row as usize);
                        ConsumerTick::Logged { row, token }
                    })
            }
            Ok(other) => Ok(classify(other)),
            Err(error) => Err(error),
        };
        let bridge = self.recorder.interval(bridge_begin);
        drop(gpu);
        let completion_begin = self.recorder.stamp();
        drop(completion);
        let completion = self.recorder.interval(completion_begin);
        let measured = cpu.finish();
        self.recorder.push(DiagnosticEvent::SourceCpu(SourceCpuTrace {
            token,
            context: DiagnosticContext::FixtureConsumer {
                id: self.config.instance,
            },
            wall: measured.wall,
            thread_cpu: measured.thread_cpu,
            receive_api,
            bridge,
            completion,
        }));
        match result {
            Ok(tick) => Ok(self.count(tick)),
            Err(error) => {
                self.counters.failed = self.counters.failed.saturating_add(1);
                Err(error)
            }
        }
    }

    /// Polls only existing nonblocking event/timestamp queries. It never receives another
    /// frame, maps pixels, flushes for query readiness, or certifies retirement by timeout.
    pub fn poll(&mut self) -> Result<()> {
        // SAFETY: this caller retains the exact owning device on its worker.
        if let Err(error) = unsafe { self.device.device.GetDeviceRemovedReason() } {
            self.removed = true;
            return Err(error);
        }
        let done = self.completion.completed(&self.device, self.generation)?;
        self.completed = done.serial;
        self.timing.poll(&self.device.context, done);
        if let Some(index) = self.pending_record
            && self.records[index].token.serial <= done.serial
        {
            self.records[index].completion_observed_qpc = self.recorder.stamp();
            self.pending_record = None;
        }
        Ok(())
    }

    /// Drains bounded CPU/GPU telemetry for an offline diagnostics consumer.
    pub fn drain_diagnostics(&self, output: &mut Vec<DiagnosticEvent>, budget: NonZeroUsize) -> usize {
        self.recorder.drain(output, budget)
    }

    /// Reads explicit metadata-ring coverage/loss counters.
    pub fn diagnostics_snapshot(&self) -> DiagnosticsSnapshot {
        self.recorder.snapshot()
    }

    /// Captures ownership/admission metadata without reading image data.
    pub fn snapshot(&self) -> FixtureConsumerSnapshot {
        let bytes = self.config.descriptor.byte_len().expect("validated UNORM descriptor");
        let log_bytes = self.log.bytes();
        FixtureConsumerSnapshot {
            instance: self.config.instance,
            adapter: self.config.adapter,
            descriptor: self.config.descriptor,
            max_frames: self.config.max_frames,
            logged_frames: self.records.len() as u32,
            submitted_batches: self.receive_batches,
            last_submitted: CompletionToken {
                device: self.generation,
                serial: self.submitted,
            },
            last_completed: CompletionToken {
                device: self.generation,
                serial: self.completed,
            },
            busy: self.counters.busy,
            receive_copies: self.counters.receive_copies,
            receive_busy: self.counters.receive_busy,
            frame_count: self.config.frame_count,
            last_copied_frame: self.counters.last_copied_frame,
            absent: self.counters.absent,
            waiting: self.counters.waiting,
            unchanged: self.counters.unchanged,
            mismatch: self.counters.mismatch,
            overflow: self.counters.overflow,
            failed_receives: self.counters.failed,
            capture_bytes: bytes,
            alias_bytes: bytes,
            gpu_log_bytes: log_bytes,
            staging_bytes: log_bytes,
            admitted_texture_bytes: bytes + 2 * log_bytes,
            final_readbacks: self.readbacks,
            readback_token: self.readback_token,
            finished: self.finished,
            device_removed: self.removed,
        }
    }

    /// Ends measurement/tick admission, drains actual GPU work, then performs the one final
    /// staging copy and Map. Call once, after the measurement window; invalid rows are kept.
    pub fn finish(&mut self) -> Result<Vec<ConsumerFrame>> {
        if self.finished {
            return Err(Error::from_hresult(INVALID_ARGUMENT));
        }
        self.finished = true;
        self.drain()?;
        self.readbacks = 1;
        let token = CompletionToken {
            device: self.generation,
            serial: self
                .submitted
                .checked_add(1)
                .ok_or_else(|| Error::from_hresult(INVALID_ARGUMENT))?,
        };
        self.submitted = token.serial;
        self.readback_token = Some(token);
        {
            let _completion = self.completion.begin(&self.device, token);
            // SAFETY: same-layout owned log/staging resources on this independent context.
            unsafe {
                self.device.context.CopyResource(&self.log.staging, &self.log.texture);
            }
        }
        self.drain()?;
        let mut frames = Vec::with_capacity(self.config.max_frames as usize);
        self.timing.finish(
            &self.device.context,
            CompletionToken {
                device: self.generation,
                serial: self.completed,
            },
        );
        self.log
            .read(&self.device.context, self.records.len() as u32, |row, raw_words| {
                let record = self.records[row as usize];
                frames.push(ConsumerFrame {
                    row,
                    token: record.token,
                    spout_image: record.spout_image,
                    receive_begin_qpc: record.receive_begin_qpc,
                    completion_observed_qpc: record.completion_observed_qpc,
                    raw_words,
                    left: CardStamp::from_words(raw_words[..6].try_into().unwrap()),
                    right: CardStamp::from_words(raw_words[6..].try_into().unwrap()),
                });
            })?;
        Ok(frames)
    }

    fn count(&mut self, tick: ConsumerTick) -> ConsumerTick {
        let counter = match tick {
            ConsumerTick::Busy => Some(&mut self.counters.busy),
            ConsumerTick::Absent => Some(&mut self.counters.absent),
            ConsumerTick::Waiting => Some(&mut self.counters.waiting),
            ConsumerTick::Unchanged => Some(&mut self.counters.unchanged),
            ConsumerTick::Mismatch => Some(&mut self.counters.mismatch),
            ConsumerTick::Full => Some(&mut self.counters.overflow),
            ConsumerTick::Logged { .. } => None,
        };
        if let Some(counter) = counter {
            *counter = counter.saturating_add(1);
        }
        tick
    }

    fn drain(&mut self) -> Result<()> {
        while self.completed < self.submitted {
            self.poll()?;
            if self.completed < self.submitted {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        Ok(())
    }
}

fn classify(received: Received) -> ConsumerTick {
    match received {
        Received::Absent => ConsumerTick::Absent,
        Received::Waiting => ConsumerTick::Waiting,
        Received::Unchanged => ConsumerTick::Unchanged,
        Received::Busy => ConsumerTick::Busy,
        Received::Mismatch => ConsumerTick::Mismatch,
        Received::Frame => unreachable!("only a completed GPU log draw constructs Logged"),
    }
}

impl Drop for FixtureConsumer {
    fn drop(&mut self) {
        while self.completed < self.submitted && !self.device.removed() {
            let _ = self.poll();
            if self.completed < self.submitted {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        if self.device.removed() {
            self.removed = true;
            self.timing.abandon();
        } else {
            self.timing.finish(
                &self.device.context,
                CompletionToken {
                    device: self.generation,
                    serial: self.completed,
                },
            );
        }
        // SAFETY: all own uses completed or actual removal was confirmed. Release bindings
        // before SDK/log/capture fields, which remain owned by this worker throughout drain.
        unsafe {
            self.device.context.ClearState();
            self.device.context.Flush();
        }
    }
}
