use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use golden_io::{ReconnectBackoff, WorkerContext};
use uuid::Uuid;
use windows::core::{Error, HRESULT, Result};

use super::super::{AdapterId, SpoutInput, completion::EventCompletion, device::Device, texture::Texture};
use super::SourceState;
use super::receiver::{Received, Receiver};
use super::shared::{Publication, Shared};
use super::surface::Slot;
use crate::TextureDescriptor;
use crate::runtime::{CompletionToken, DeviceGeneration};

struct Session {
    receiver: Receiver,
    capture: Option<Texture>,
    slots: Vec<Slot>,
    completion: EventCompletion,
    device: Device,
    generation: DeviceGeneration,
    submitted: u64,
    shared: Arc<Shared>,
}

pub(super) fn run(context: WorkerContext<(), ()>, adapter: AdapterId, config: SpoutInput, shared: Arc<Shared>) {
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
    fn new(adapter: AdapterId, config: &SpoutInput, shared: Arc<Shared>) -> Result<Self> {
        let device = Device::new(adapter)?;
        let receiver = Receiver::new(&device.device, &config.sender, config.descriptor)?;
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
        shared.unlock_failed.store(false, Ordering::Release);
        shared.update(|status| status.generation = next);
        Ok(Self {
            receiver,
            capture: None,
            slots,
            completion,
            device,
            generation,
            submitted: 0,
            shared,
        })
    }

    fn serve(&mut self, context: &WorkerContext<(), ()>, config: &SpoutInput, sequence: &mut u64) -> Result<()> {
        let mut missing = ReconnectBackoff::new(Duration::from_millis(100), Duration::from_secs(1));
        loop {
            let checkpoint = context.wake_checkpoint();
            if context.cancellation().is_cancelled() {
                return Ok(());
            }
            if self.device.removed() || self.shared.unlock_failed.load(Ordering::Acquire) {
                return Err(Error::from_hresult(HRESULT(0x887a_0005_u32 as i32)));
            }
            if self.completion.completed(&self.device, self.generation)?.serial < self.submitted {
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
                Ok(Received::Unchanged) => config.poll_interval,
                Ok(Received::Absent) => {
                    self.shared.clear();
                    self.shared.state(SourceState::WaitingForSender);
                    missing
                        .schedule(Instant::now())
                        .saturating_duration_since(Instant::now())
                }
                Ok(Received::Mismatch) => {
                    self.shared.clear();
                    self.shared.state(SourceState::DescriptorMismatch);
                    self.drain_source();
                    self.receiver.disconnect();
                    missing
                        .schedule(Instant::now())
                        .saturating_duration_since(Instant::now())
                }
                Ok(Received::Waiting) => {
                    self.shared.clear();
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
        if let Some(outcome) = self.receiver.preflight()? {
            return Ok(outcome);
        }
        let descriptor = self.receiver.descriptor().expect("validated receiver layout");
        if self
            .capture
            .as_ref()
            .is_none_or(|capture| capture.descriptor != descriptor)
            && !self.rebuild(descriptor)?
        {
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
            return Ok(Received::Unchanged);
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
        let guard = self.completion.begin(
            &self.device,
            CompletionToken {
                device: self.generation,
                serial,
            },
        );
        self.submitted = serial;
        self.shared
            .update(|status| status.sdk_calls = status.sdk_calls.saturating_add(1));
        let capture = self.capture.as_ref().expect("prepared capture");
        let received = self.receiver.receive(capture)?;
        if matches!(received, Received::Frame) && self.shared.active.load(Ordering::Acquire) {
            unsafe {
                self.device
                    .context
                    .CopyResource(&slot.texture.texture, &capture.texture);
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
            });
        } else {
            drop(writer);
        }
        drop(guard);
        Ok(received)
    }

    fn rebuild(&mut self, descriptor: TextureDescriptor) -> Result<bool> {
        // Called only after the previous source query completed, before any new commands.
        // Renderer aliases/leases must retire before a replacement pool can consume its charge.
        if !self.slots.is_empty() {
            self.retire_imports();
        }
        self.slots.clear();
        self.capture = None;
        self.shared.update(|status| {
            status.bridge_slots = 0;
            status.observed_descriptor = Some(descriptor);
        });
        if !self.shared.active.load(Ordering::Acquire) || self.shared.shutdown.load(Ordering::Acquire) {
            self.shared.retiring.store(false, Ordering::Release);
            return Ok(false);
        }
        let capture = Texture::new(&self.device.device, descriptor)?;
        let slots = (0..3)
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
        self.capture = Some(capture);
        self.slots = slots;
        self.shared.update(|status| {
            status.pool_generation = next;
            status.bridge_slots = 3;
        });
        self.shared.released.store(false, Ordering::Release);
        self.shared.retiring.store(false, Ordering::Release);
        Ok(true)
    }

    fn drain_source(&mut self) {
        while !self.device.removed() {
            match self.completion.completed(&self.device, self.generation) {
                Ok(done) if done.serial >= self.submitted => break,
                _ => std::thread::sleep(Duration::from_millis(1)),
            }
        }
    }

    fn retire_imports(&self) {
        self.shared.clear();
        self.shared.released.store(false, Ordering::Release);
        self.shared.retiring.store(true, Ordering::Release);
        self.shared.state(SourceState::Recovering);
        self.shared.wake_renderer();
        while !self.shared.shutdown.load(Ordering::Acquire) && !self.shared.released.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(1));
        }
        while self.slots.iter().any(|slot| Arc::strong_count(&slot.surface) != 1) {
            std::thread::sleep(Duration::from_millis(1));
        }
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
            self.shared.unlock_failed.store(true, Ordering::Release);
            self.shared.update(|status| status.hresult = Some(error.code().0));
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // The source has its own device/context completion authority. Renderer completion alone
        // cannot retire the SDK alias, capture texture or bridge writer's GPU work.
        self.drain_source();
        self.retire_imports();
        self.shared.update(|status| status.bridge_slots = 0);
        // SDK/native fields drop on this source worker only, while its pool lifetime stays held.
        unsafe {
            self.device.context.ClearState();
            self.device.context.Flush();
        }
    }
}
