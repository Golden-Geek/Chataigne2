use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use golden_io::{ReconnectBackoff, WorkerContext};
use uuid::Uuid;
use windows::core::{Error, HRESULT, Result};

use super::super::config::INPUT_BRIDGE_SLOTS;
use super::super::timing::{
    CpuScope, DiagnosticContext, DiagnosticEvent, GpuTiming, PublicationTiming, Recorder, SourceCpuTrace,
};
use super::super::{AdapterId, completion::EventCompletion, device::Device};
use super::SourceState;
use super::receiver::{Received, Receiver};
use super::shared::{Publication, Shared};
use super::surface::Slot;
use crate::TextureDescriptor;
use crate::plan::SpoutSource;
use crate::runtime::{CompletionToken, DeviceGeneration};

struct Session {
    timing: Option<GpuTiming>,
    diagnostic_context: DiagnosticContext,
    receiver: Receiver,
    descriptor: Option<TextureDescriptor>,
    slots: Vec<Slot>,
    completion: EventCompletion,
    device: Device,
    generation: DeviceGeneration,
    submitted: u64,
    shared: Arc<Shared>,
}

pub(super) fn run(context: WorkerContext<(), ()>, adapter: AdapterId, config: SpoutSource, shared: Arc<Shared>) {
    let mut retry = ReconnectBackoff::new(Duration::from_millis(100), Duration::from_secs(1));
    let mut sequence = 0_u64;
    while !context.cancellation().is_cancelled() {
        let checkpoint = context.wake_checkpoint();
        if !shared.active.load(Ordering::Acquire) {
            shared.state(SourceState::Idle);
            context.wait_until(checkpoint, Instant::now() + Duration::from_secs(1));
            continue;
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut session = Session::new(adapter, &config, shared.clone())?;
            session.serve(&context, &config, &mut sequence)
        }));
        match result {
            Ok(Ok(())) => break,
            Ok(Err(error)) => shared.update(|status| status.hresult = Some(error.code().0)),
            Err(_) => shared.update(|status| status.hresult = Some(0x8000_4005_u32 as i32)),
        }
        shared.state(SourceState::Recovering);
        let checkpoint = context.wake_checkpoint();
        context.wait_until(checkpoint, retry.schedule(Instant::now()));
    }
    shared.clear();
    shared.state(SourceState::Stopped);
}

impl Session {
    fn new(adapter: AdapterId, config: &SpoutSource, shared: Arc<Shared>) -> Result<Self> {
        let device = Device::new(adapter)?;
        let receiver = Receiver::new(&device.device, &config.sender, config.descriptor, config.frame_count)?;
        let completion = EventCompletion::new(&device)?;
        let slots = Vec::new();
        let generation = DeviceGeneration::new(Uuid::new_v4(), 0);
        let next = shared
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .generation
            .checked_add(1)
            .ok_or_else(|| Error::from_hresult(HRESULT(0x8000_4005_u32 as i32)))?;
        shared.released.store(false, Ordering::Release);
        shared.retiring.store(false, Ordering::Release);
        shared.faulted.store(false, Ordering::Release);
        shared.update(|status| status.generation = next);
        let status = *shared.status.lock().unwrap_or_else(|error| error.into_inner());
        let diagnostic_context = DiagnosticContext::Source {
            binding: status.binding,
            texture: status.texture,
            renderer_device: status.device,
        };
        let timing = shared
            .diagnostics
            .as_ref()
            .map(|recorder| GpuTiming::new(&device.device, recorder.clone(), diagnostic_context));
        Ok(Self {
            timing,
            diagnostic_context,
            receiver,
            descriptor: None,
            slots,
            completion,
            device,
            generation,
            submitted: 0,
            shared,
        })
    }

    fn serve(&mut self, context: &WorkerContext<(), ()>, config: &SpoutSource, sequence: &mut u64) -> Result<()> {
        let mut missing = ReconnectBackoff::new(Duration::from_millis(100), Duration::from_secs(1));
        loop {
            let checkpoint = context.wake_checkpoint();
            if context.cancellation().is_cancelled() {
                return Ok(());
            }
            // SAFETY: this worker retains the exact source device; preserve its real removal code.
            unsafe { self.device.device.GetDeviceRemovedReason()? };
            if self.shared.faulted.load(Ordering::Acquire) {
                let cause = self
                    .shared
                    .status
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .hresult;
                return Err(Error::from_hresult(HRESULT(cause.unwrap_or(0x8000_4005_u32 as i32))));
            }
            let completed = self.completion.completed(&self.device, self.generation)?;
            if let Some(timing) = &mut self.timing {
                timing.poll(&self.device.context, completed);
            }
            if completed.serial < self.submitted {
                context.wait_until(checkpoint, Instant::now() + Duration::from_millis(1));
                continue;
            }
            if !self.shared.active.load(Ordering::Acquire) {
                self.receiver.disconnect();
                self.shared.clear();
                self.shared.state(SourceState::Idle);
                context.wait_until(checkpoint, Instant::now() + Duration::from_secs(1));
                continue;
            }
            let result = self.step(sequence);
            let wait = match result {
                Ok(Received::Frame) => {
                    missing.reset();
                    config.poll_interval
                }
                Ok(Received::Unchanged) => {
                    self.shared
                        .update(|status| status.unchanged = status.unchanged.saturating_add(1));
                    config.poll_interval
                }
                Ok(Received::Busy) => config.poll_interval,
                Ok(Received::Absent) => {
                    self.shared.clear();
                    self.shared.update(|status| status.last_copied_frame = None);
                    self.shared.state(SourceState::WaitingForSender);
                    missing
                        .schedule(Instant::now())
                        .saturating_duration_since(Instant::now())
                }
                Ok(Received::Mismatch) => {
                    self.shared.clear();
                    self.shared.update(|status| status.last_copied_frame = None);
                    self.shared.state(SourceState::DescriptorMismatch);
                    self.drain_source();
                    self.receiver.disconnect();
                    missing
                        .schedule(Instant::now())
                        .saturating_duration_since(Instant::now())
                }
                Ok(Received::Waiting) => {
                    self.shared.clear();
                    self.shared.update(|status| status.last_copied_frame = None);
                    self.shared.state(SourceState::WaitingForSender);
                    config.poll_interval
                }
                Err(error) => {
                    self.shared.update(|status| status.hresult = Some(error.code().0));
                    return Err(error);
                }
            };
            context.wait_until(checkpoint, Instant::now() + wait);
        }
    }

    fn step(&mut self, sequence: &mut u64) -> Result<Received> {
        // The source loop observed completion of every prior command before entering step.
        // A demand 0→1 pulse can clear the mailbox without the worker observing inactivity;
        // publication can also be suppressed when demand falls during receive. Re-arm the
        // SDK comparator only after a copied version loses its retained publication, so a
        // static sender can republish without repeatedly resetting the initial handshake.
        let unpublished_copy = self.receiver.last_copied_frame().is_some()
            && self
                .shared
                .latest
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .is_none();
        if unpublished_copy {
            self.receiver.disconnect();
        }
        if let Some(outcome) = self.receiver.preflight()? {
            return Ok(outcome);
        }
        let descriptor = self.receiver.descriptor().expect("validated receiver layout");
        if self.descriptor != Some(descriptor) && !self.rebuild(descriptor)? {
            return Ok(Received::Waiting);
        }
        if !self.shared.active.load(Ordering::Acquire) {
            return Ok(Received::Waiting);
        }
        let mut selected = None;
        for candidate in &self.slots {
            if candidate.acquire_free()? {
                selected = Some(candidate);
                break;
            }
        }
        let Some(slot) = selected else {
            self.shared
                .update(|status| status.bridge_busy = status.bridge_busy.saturating_add(1));
            return Ok(Received::Busy);
        };
        // Admit bridge capacity before SDK receive/copy, not after queuing wasted GPU work.
        let mut writer = Writer {
            slot,
            key: 0,
            shared: self.shared.clone(),
        };
        let serial = self
            .submitted
            .checked_add(1)
            .ok_or_else(|| Error::from_hresult(HRESULT(0x8000_4005_u32 as i32)))?;
        let token = CompletionToken {
            device: self.generation,
            serial,
        };
        let recorder = self.shared.diagnostics.as_ref();
        let cpu = recorder.map(CpuScope::new);
        let guard = self.completion.begin(&self.device, token);
        let gpu = self
            .timing
            .as_mut()
            .map(|timing| timing.begin(&self.device.context, token));
        self.submitted = serial;
        self.shared
            .update(|status| status.sdk_calls = status.sdk_calls.saturating_add(1));
        let receive_begin_qpc = recorder.and_then(Recorder::stamp);
        // This free bridge slot is already write-locked. The SDK's checked caller-owned
        // destination path copies directly into it, with no private intermediate image.
        let received = self.receiver.receive(&slot.texture);
        let receive_api = recorder.and_then(|recorder| recorder.interval(receive_begin_qpc));
        self.shared.update(|status| match &received {
            Ok(Received::Frame) => {
                status.receive_copies = status.receive_copies.saturating_add(1);
                status.last_copied_frame = self.receiver.last_copied_frame();
            }
            Ok(Received::Busy) => status.receive_busy = status.receive_busy.saturating_add(1),
            _ => {}
        });
        let bridge_begin = recorder.and_then(Recorder::stamp);
        if matches!(&received, Ok(Received::Frame)) && self.shared.active.load(Ordering::Acquire) {
            unsafe {
                self.device.context.Flush();
            }
            writer.key = 1;
            drop(writer);
            *sequence = sequence
                .checked_add(1)
                .ok_or_else(|| Error::from_hresult(HRESULT(0x8000_4005_u32 as i32)))?;
            self.shared.publish(Publication {
                surface: slot.surface.clone(),
                sequence: *sequence,
                timing: recorder.map(|recorder| PublicationTiming {
                    source_token: token,
                    receive_begin_qpc,
                    publication_qpc: recorder.stamp(),
                    spout_image: self.receiver.copied_image_version(),
                }),
            });
        } else {
            drop(writer);
        }
        let bridge = recorder.and_then(|recorder| recorder.interval(bridge_begin));
        drop(gpu);
        let completion_begin = recorder.and_then(Recorder::stamp);
        drop(guard);
        let completion = recorder.and_then(|recorder| recorder.interval(completion_begin));
        if let Some(cpu) = cpu {
            let measured = cpu.finish();
            recorder.unwrap().push(DiagnosticEvent::SourceCpu(SourceCpuTrace {
                token,
                context: self.diagnostic_context,
                wall: measured.wall,
                thread_cpu: measured.thread_cpu,
                receive_api,
                bridge,
                completion,
            }));
        }
        received
    }

    fn rebuild(&mut self, descriptor: TextureDescriptor) -> Result<bool> {
        // Called only after the previous source query completed, before any new commands.
        // Renderer aliases/leases must retire before a replacement pool can consume its charge.
        if !self.slots.is_empty() {
            self.retire_imports();
        }
        self.slots.clear();
        self.descriptor = None;
        self.shared.update(|status| {
            status.bridge_slots = 0;
            status.observed_descriptor = Some(descriptor);
        });
        if !self.shared.active.load(Ordering::Acquire) || self.shared.shutdown.load(Ordering::Acquire) {
            self.shared.retiring.store(false, Ordering::Release);
            return Ok(false);
        }
        let slots = (0..INPUT_BRIDGE_SLOTS)
            .map(|index| Slot::new(&self.device, descriptor, index))
            .collect::<Result<Vec<_>>>()?;
        let next = self
            .shared
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .pool_generation
            .checked_add(1)
            .ok_or_else(|| Error::from_hresult(HRESULT(0x8000_4005_u32 as i32)))?;
        self.descriptor = Some(descriptor);
        self.slots = slots;
        self.shared.update(|status| {
            status.pool_generation = next;
            status.bridge_slots = INPUT_BRIDGE_SLOTS;
        });
        self.shared.released.store(false, Ordering::Release);
        self.shared.retiring.store(false, Ordering::Release);
        Ok(true)
    }

    fn drain_source(&mut self) {
        drain_source(
            &self.shared,
            || self.device.removed(),
            || {
                let done = self.completion.completed(&self.device, self.generation)?;
                if let Some(timing) = &mut self.timing {
                    timing.poll(&self.device.context, done);
                }
                Ok(done.serial >= self.submitted)
            },
        );
    }

    fn retire_imports(&self) {
        self.shared.withdraw();
        while !self.shared.shutdown.load(Ordering::Acquire) && !self.shared.released.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(1));
        }
        while self.slots.iter().any(|slot| Arc::strong_count(&slot.surface) != 1) {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

pub(super) fn retire_source(shared: &Shared, removed: impl FnMut() -> bool, completed: impl FnMut() -> Result<bool>) {
    // Withdrawal precedes a potentially blocked/erroring query. This changes visibility,
    // not ownership: cancellation cannot certify completion of queued source-device work.
    shared.withdraw();
    drain_source(shared, removed, completed);
}

fn drain_source(shared: &Shared, mut removed: impl FnMut() -> bool, mut completed: impl FnMut() -> Result<bool>) {
    while !removed() {
        match completed() {
            Ok(true) => break,
            Err(error) => shared.update(|status| status.hresult = Some(error.code().0)),
            Ok(false) => {}
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

struct Writer<'a> {
    slot: &'a Slot,
    key: u64,
    shared: Arc<Shared>,
}

impl Drop for Writer<'_> {
    fn drop(&mut self) {
        // SAFETY: construction follows only a successful zero-timeout keyed-mutex acquisition.
        if let Err(error) = unsafe { self.slot.mutex.ReleaseSync(self.key) } {
            self.shared.record_fault(error.code().0);
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // The source has its own device/context completion authority. Renderer completion alone
        // cannot retire the SDK alias or bridge writer's GPU work.
        retire_source(
            &self.shared,
            || self.device.removed(),
            || {
                let done = self.completion.completed(&self.device, self.generation)?;
                if let Some(timing) = &mut self.timing {
                    timing.poll(&self.device.context, done);
                }
                Ok(done.serial >= self.submitted)
            },
        );
        self.retire_imports();
        self.shared.update(|status| status.bridge_slots = 0);
        if let Some(timing) = &mut self.timing {
            if self.device.removed() {
                timing.abandon();
            } else if let Ok(done) = self.completion.completed(&self.device, self.generation) {
                timing.finish(&self.device.context, done);
            } else {
                timing.abandon();
            }
        }
        // SDK/native fields drop on this source worker only, while its pool lifetime stays held.
        unsafe {
            self.device.context.ClearState();
            self.device.context.Flush();
        }
    }
}
