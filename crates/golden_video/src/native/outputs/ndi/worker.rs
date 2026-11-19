//! The NDI SDK can compress or wait inside a send. Keep it off the GPU worker.
use std::num::NonZeroUsize;
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use golden_io::{
    BoundedWorker, PayloadPool, PayloadReservation, PendingSendError, ReconnectBackoff, RetirementPool, WorkerLimits,
};
use windows::core::{Error, HRESULT, Result};

use crate::ndi::{NdiSender, NdiVideoFormat};

pub(super) const CPU_BUFFERS: usize = 2;
const CAPACITY: HRESULT = HRESULT(0x8007000e_u32 as i32);
const FAILURE: HRESULT = HRESULT(0x80004005_u32 as i32);

struct Frame {
    pixels: Vec<u8>,
    timecode: i64,
}

pub(super) struct Publisher {
    worker: BoundedWorker<Frame, Vec<u8>>,
    free: Vec<Vec<u8>>,
    ready: Arc<AtomicBool>,
    _reservation: Arc<PayloadReservation>,
}

impl Publisher {
    pub fn start(name: String, format: NdiVideoFormat) -> Result<Self> {
        static LIFETIMES: OnceLock<RetirementPool> = OnceLock::new();
        static PAYLOAD: OnceLock<PayloadPool> = OnceLock::new();
        let lifetimes = LIFETIMES.get_or_init(|| RetirementPool::new(NonZeroUsize::new(16).unwrap()));
        let payload = PAYLOAD.get_or_init(|| {
            PayloadPool::new("NDI output frames", 16, 512 * 1024 * 1024).expect("fixed positive NDI budget")
        });
        let bytes = format.bytes();
        let total = bytes
            .checked_mul(CPU_BUFFERS)
            .ok_or_else(|| Error::from_hresult(CAPACITY))?;
        let reservation = Arc::new(
            payload
                .try_reserve(total)
                .map_err(|_| Error::new(CAPACITY, "NDI output CPU image pool is full"))?,
        );
        let mut free = Vec::with_capacity(CPU_BUFFERS);
        for _ in 0..CPU_BUFFERS {
            let mut pixels = Vec::new();
            pixels
                .try_reserve_exact(bytes)
                .map_err(|_| Error::from_hresult(CAPACITY))?;
            pixels.resize(bytes, 0);
            free.push(pixels);
        }
        let ready = Arc::new(AtomicBool::new(false));
        let worker_ready = ready.clone();
        let worker_reservation = reservation.clone();
        let worker = BoundedWorker::<Frame, Vec<u8>>::spawn(
            "golden-ndi-publish",
            lifetimes,
            WorkerLimits {
                command_items: 1,
                command_weight: bytes,
                event_items: CPU_BUFFERS,
                event_weight: total,
            },
            move |context| {
                let _reservation = worker_reservation;
                let cancellation = context.cancellation();
                let mut retry = ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(2));
                let mut previous_error = None;
                let mut sender = loop {
                    if cancellation.is_cancelled() {
                        return;
                    }
                    match NdiSender::new(&name) {
                        Ok(sender) => break sender,
                        Err(error) => {
                            let message = error.to_string();
                            if previous_error.as_ref() != Some(&message) {
                                eprintln!("[golden-ndi-publish] {name}: {message}");
                                previous_error = Some(message);
                            }
                            let now = Instant::now();
                            if cancellation.wait(retry.schedule(now).saturating_duration_since(now)) {
                                return;
                            }
                        }
                    }
                };
                worker_ready.store(true, Ordering::Release);
                while let Some(mut frame) = context.receive() {
                    if sender
                        .send_video(format, &mut frame.pixels, Some(frame.timecode))
                        .is_err()
                        || context.publish_weighted(frame.pixels, bytes).is_err()
                    {
                        break;
                    }
                }
                worker_ready.store(false, Ordering::Release);
            },
        )
        .map_err(|error| Error::new(FAILURE, error.to_string()))?;
        Ok(Self {
            worker,
            free,
            ready,
            _reservation: reservation,
        })
    }

    pub fn is_open(&self) -> bool {
        self.worker.is_accepting_commands()
    }

    pub fn stop(&self) {
        self.worker.cancel();
    }

    pub fn ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    pub fn take_buffer(&mut self) -> Option<Vec<u8>> {
        self.worker
            .drain_events(&mut self.free, NonZeroUsize::new(CPU_BUFFERS).unwrap());
        self.free.pop()
    }

    pub fn recycle(&mut self, pixels: Vec<u8>) {
        self.free.push(pixels);
    }

    pub fn send(&mut self, pixels: Vec<u8>, timecode: i64) -> bool {
        let weight = pixels.len();
        match self.worker.send_weighted(Frame { pixels, timecode }, weight) {
            Ok(()) => true,
            Err(PendingSendError::Full(frame) | PendingSendError::Disconnected(frame)) => {
                self.recycle(frame.pixels);
                false
            }
        }
    }
}
