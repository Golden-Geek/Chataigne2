use std::num::NonZeroUsize;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use windows::core::{Error, HRESULT, Result};

use super::pipeline_helper::Pipeline;
use super::{CardStamp, marker_hlsl};
use crate::native::AdapterId;
use crate::native::completion::EventCompletion;
use crate::native::device::Device;
use crate::native::outputs::Delivery;
use crate::native::outputs::spout::SpoutOutput;
use crate::native::texture::Texture;
use crate::native::timing::{
    CpuScope, DiagnosticContext, DiagnosticDelivery, DiagnosticEvent, DiagnosticsConfig, DiagnosticsSnapshot,
    GpuTiming, OutputTrace, Recorder, SourceCpuTrace,
};
use crate::runtime::{CompletionToken, DeviceGeneration};
use crate::{AlphaMode, ColorEncoding, OutputId, PixelFormat, TextureDescriptor};

const INVALID_ARGUMENT: HRESULT = HRESULT(0x8007_0057_u32 as i32);
const CONSTANT_BYTES: u64 = 32;

/// Explicit producer configuration. Run this worker-local fixture in its own owned process;
/// it is a GPU card generator and does not submit per-frame render plans or image uploads.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FixtureSourceConfig {
    /// Exact selected hardware adapter; no software or alternate-adapter fallback.
    pub adapter: AdapterId,
    /// Required BGRA8 UNORM, sRGB, straight-alpha source layout.
    pub descriptor: TextureDescriptor,
    /// Source identity encoded in every card and checked on each submission.
    pub source_id: u32,
    /// Unique producer lifetime, also fencing its completion and diagnostic tokens.
    pub instance: Uuid,
    /// Exact externally visible Spout sender identity; collisions fail explicitly.
    pub sender_name: String,
    /// Explicit bounded metadata capacity, at most 65,536 rows.
    pub diagnostics: DiagnosticsConfig,
}

/// One attempted native send, after its GPU-use completion query was established. API
/// acceptance alone does not establish receiver delivery, frame freshness or scanout.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct FixtureSubmission {
    /// Fixture device token; its serial is the complete positive `stamp.frame` value.
    pub token: CompletionToken,
    /// Exact marker included in the submitted card.
    pub stamp: CardStamp,
    /// Native SDK disposition, retained even when the send failed after queuing GPU work.
    pub outcome: DiagnosticDelivery,
    /// Actual accepted shared-image version, distinct from attempted marker/frame serial.
    pub spout_image: Option<super::super::timing::SpoutImageVersion>,
}

/// Sparse producer metadata; descriptor payload charges are separate from measured DXGI
/// residency. SDK storage remains admitted until this producer is actually retired.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FixtureSourceSnapshot {
    /// Whether finish has closed admission before the final telemetry report.
    pub finished: bool,
    /// Exact producer lifetime.
    pub instance: Uuid,
    /// Logical card source identity.
    pub source_id: u32,
    /// Exact selected hardware adapter.
    pub adapter: AdapterId,
    /// Immutable card storage/color/alpha layout.
    pub descriptor: TextureDescriptor,
    /// One owned card texture plus the SDK's one admitted shared sender texture.
    pub admitted_texture_bytes: u64,
    /// The one fixed uniform buffer; uniform updates are not CPU pixel uploads.
    pub constant_buffer_bytes: u64,
    /// Actual batches admitted to the producer GPU context, including failed sends.
    pub submitted_batches: u64,
    /// Actual SDK copy submissions, without claiming receiver delivery or unique consumption.
    pub accepted_sends: u64,
    /// SDK access was busy, with no shared-image copy or frame-counter advance.
    pub busy_sends: u64,
    /// Native failed sends, including confirmed producer-device removal.
    pub failed_sends: u64,
    /// Attempts skipped before native work because both real completion slots were retained.
    pub admission_busy: u64,
    /// Last proposed positive-frame token whose event was submitted.
    pub last_submitted: CompletionToken,
    /// Latest actually observed completed producer token.
    pub last_completed: CompletionToken,
    /// Last card admitted to native work, whether or not the SDK accepted it.
    pub last_stamp: Option<CardStamp>,
    /// Whether the producer device itself has reported removal.
    pub device_removed: bool,
}

/// Qualification-only GPU producer. All native interfaces remain on its creating worker;
/// Drop retains them until real completion or confirmed removal, without a false timeout fence.
pub struct FixtureSource {
    finished: bool,
    output: SpoutOutput,
    texture: Texture,
    pipeline: Pipeline,
    timing: GpuTiming,
    completion: EventCompletion,
    recorder: Recorder,
    device: Device,
    config: FixtureSourceConfig,
    generation: DeviceGeneration,
    submitted: u64,
    completed: u64,
    submitted_batches: u64,
    accepted_sends: u64,
    busy_sends: u64,
    failed_sends: u64,
    admission_busy: u64,
    last_stamp: Option<CardStamp>,
    admitted_texture_bytes: u64,
    removed: bool,
}

impl FixtureSource {
    /// Allocates all producer shaders, views, completion queries and bounded telemetry before
    /// frame work. The SDK's one lazy shared texture is included in its fixed admitted charge.
    pub fn new(config: FixtureSourceConfig) -> Result<Self> {
        let descriptor = config.descriptor;
        if descriptor.format != PixelFormat::Bgra8Unorm
            || descriptor.color_encoding != ColorEncoding::Srgb
            || descriptor.alpha_mode != AlphaMode::Straight
            || descriptor.extent.width() > 16_384
            || descriptor.extent.height() > 16_384
            || config.diagnostics.capacity.get() > 65_536
        {
            return Err(Error::from_hresult(INVALID_ARGUMENT));
        }
        let admitted_texture_bytes = descriptor
            .byte_len()
            .map_err(|_| Error::from_hresult(INVALID_ARGUMENT))?
            .checked_mul(2)
            .ok_or_else(|| Error::from_hresult(INVALID_ARGUMENT))?;
        let device = Device::new(config.adapter)?;
        let recorder = Recorder::new(config.diagnostics);
        let generation = DeviceGeneration::new(config.instance, 0);
        let context = DiagnosticContext::FixtureProducer { id: config.instance };
        let shader = format!("{}\n{}", marker_hlsl(), include_str!("card.hlsl"));
        let pipeline = Pipeline::new(&device.device, &shader, CONSTANT_BYTES as u32)?;
        let texture = Texture::new(&device.device, descriptor)?;
        let completion = EventCompletion::new(&device)?;
        let timing = GpuTiming::new(&device.device, recorder.clone(), context);
        let output = SpoutOutput::new(&device.device, config.sender_name.clone(), descriptor)?;
        Ok(Self {
            finished: false,
            output,
            texture,
            pipeline,
            timing,
            completion,
            recorder,
            device,
            config,
            generation,
            submitted: 0,
            completed: 0,
            submitted_batches: 0,
            accepted_sends: 0,
            busy_sends: 0,
            failed_sends: 0,
            admission_busy: 0,
            last_stamp: None,
            admitted_texture_bytes,
            removed: false,
        })
    }

    /// Attempts one caller-paced frame. Frame IDs start at one and increase strictly; gaps
    /// after busy attempts are permitted. None means no native work was admitted. The caller
    /// schedules 60 Hz without catch-up bursts and preserves these skipped attempts in evidence.
    pub fn submit(&mut self, stamp: CardStamp) -> Result<Option<FixtureSubmission>> {
        if self.finished
            || stamp.source != self.config.source_id
            || stamp.frame <= self.submitted
            || stamp.qpc == 0
            || self.last_stamp.is_some_and(|previous| stamp.qpc < previous.qpc)
        {
            return Err(Error::from_hresult(INVALID_ARGUMENT));
        }
        self.poll()?;
        if !self.completion.can_submit() {
            self.admission_busy = self.admission_busy.saturating_add(1);
            return Ok(None);
        }
        let token = CompletionToken {
            device: self.generation,
            serial: stamp.frame,
        };
        let scope = CpuScope::new(&self.recorder);
        self.submitted = stamp.frame;
        self.submitted_batches = self.submitted_batches.saturating_add(1);
        self.last_stamp = Some(stamp);
        // Claim completion before any diagnostic or image command, including unwind paths.
        let submission = self.completion.begin(&self.device, token);
        let gpu = self.timing.begin(&self.device.context, token);
        let words = stamp.words();
        let constants = [words[0], words[1], words[2], words[3], words[4], words[5], 0, 0];
        let draw = self.pipeline.draw_region(
            &self.device.context,
            &self.texture.rtv,
            [0, 0],
            [
                self.config.descriptor.extent.width(),
                self.config.descriptor.extent.height(),
            ],
            &[],
            &constants,
        );
        let begin_delivery = self.recorder.stamp();
        let mut consumer_api = None;
        let mut delivery = match draw {
            Ok(()) => self
                .output
                .deliver(&self.texture, Some(&self.recorder), &mut consumer_api),
            Err(error) => Delivery::Failed(error.code()),
        };
        let delivery_wall = self.recorder.interval(begin_delivery);
        // SAFETY: only this producer owns the exact device whose commands were just queued.
        if let Err(error) = unsafe { self.device.device.GetDeviceRemovedReason() } {
            self.removed = true;
            delivery = Delivery::DeviceLost(error.code());
        }
        drop(gpu);
        let completion_begin = self.recorder.stamp();
        drop(submission);
        let completion = self.recorder.interval(completion_begin);
        let cpu = scope.finish();
        self.recorder.push(DiagnosticEvent::SourceCpu(SourceCpuTrace {
            token,
            context: DiagnosticContext::FixtureProducer {
                id: self.config.instance,
            },
            wall: cpu.wall,
            thread_cpu: cpu.thread_cpu,
            receive_api: None,
            bridge: None,
            completion,
        }));
        let outcome = DiagnosticDelivery::from(delivery);
        let spout_image = (delivery == Delivery::Accepted)
            .then(|| self.output.copied_image_version())
            .flatten();
        self.recorder.push(DiagnosticEvent::Output(OutputTrace {
            token,
            output: OutputId::new(1).expect("positive fixture-local output identity"),
            conversion: None,
            delivery: delivery_wall,
            consumer_api,
            outcome,
            spout_image,
        }));
        match outcome {
            DiagnosticDelivery::Accepted => self.accepted_sends = self.accepted_sends.saturating_add(1),
            DiagnosticDelivery::Busy => self.busy_sends = self.busy_sends.saturating_add(1),
            DiagnosticDelivery::Failed { .. } | DiagnosticDelivery::DeviceLost { .. } => {
                self.failed_sends = self.failed_sends.saturating_add(1);
            }
            _ => {}
        }
        Ok(Some(FixtureSubmission {
            token,
            stamp,
            outcome,
            spout_image,
        }))
    }

    /// Nonblocking event/query polling on this producer worker. A query error alone never
    /// certifies completion or device removal, and therefore cannot release retained resources.
    pub fn poll(&mut self) -> Result<CompletionToken> {
        // SAFETY: the retained producer device is queried on its owning worker.
        if let Err(error) = unsafe { self.device.device.GetDeviceRemovedReason() } {
            self.removed = true;
            return Err(error);
        }
        let completed = self.completion.completed(&self.device, self.generation)?;
        self.completed = completed.serial;
        self.timing.poll(&self.device.context, completed);
        Ok(completed)
    }

    /// Closes new submissions, drains actual completion and finalizes timestamp coverage
    /// before the final report. S_FALSE becomes explicit Uncollected, without a query wait or
    /// extra flush; ordinary query errors never certify resource retirement.
    pub fn finish(&mut self) -> Result<()> {
        self.finished = true;
        loop {
            if let Err(error) = self.poll() {
                if self.removed {
                    self.timing.abandon();
                }
                return Err(error);
            }
            if self.completed >= self.submitted {
                break;
            }
            if self.completed < self.submitted {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        self.timing.finish(
            &self.device.context,
            CompletionToken {
                device: self.generation,
                serial: self.completed,
            },
        );
        Ok(())
    }

    /// Drains bounded metadata without transferring native resources.
    pub fn drain_diagnostics(&self, output: &mut Vec<DiagnosticEvent>, budget: NonZeroUsize) -> usize {
        self.recorder.drain(output, budget)
    }

    /// Reports dropped/contended/queued diagnostic rows; lost coverage never becomes a pass.
    pub fn diagnostics_snapshot(&self) -> DiagnosticsSnapshot {
        self.recorder.snapshot()
    }

    /// Copies producer counters without polling GPU work or making a delivery claim.
    pub fn snapshot(&self) -> FixtureSourceSnapshot {
        FixtureSourceSnapshot {
            finished: self.finished,
            instance: self.config.instance,
            source_id: self.config.source_id,
            adapter: self.config.adapter,
            descriptor: self.config.descriptor,
            admitted_texture_bytes: self.admitted_texture_bytes,
            constant_buffer_bytes: CONSTANT_BYTES,
            submitted_batches: self.submitted_batches,
            accepted_sends: self.accepted_sends,
            busy_sends: self.busy_sends,
            failed_sends: self.failed_sends,
            admission_busy: self.admission_busy,
            last_submitted: CompletionToken {
                device: self.generation,
                serial: self.submitted,
            },
            last_completed: CompletionToken {
                device: self.generation,
                serial: self.completed,
            },
            last_stamp: self.last_stamp,
            device_removed: self.removed,
        }
    }
}

impl Drop for FixtureSource {
    fn drop(&mut self) {
        while self.completed < self.submitted && !self.removed {
            let _ = self.poll();
            if self.completed < self.submitted && !self.removed {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        if self.removed {
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
        // SAFETY: every admitted use completed or the exact producer device confirmed removal.
        // Native fields drop afterward, releasing the SDK before textures/pipeline/device.
        unsafe {
            self.device.context.ClearState();
            self.device.context.Flush();
        }
    }
}
