//! SDK receive/lifecycle work owns a separate device/context on each source thread.

mod surface;
mod worker;

use crate::{ProbeResult, d3d::Gpu};
use mapgyver_native_probe::{resource_log, statistics::Distribution};
use serde::Serialize;
use std::sync::{
    Arc, Mutex, TryLockError,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::thread::JoinHandle;
pub use surface::Frame;

const CAPACITY: usize = 3;

#[derive(Default)]
struct Latest {
    publication: Option<surface::Publication>,
    error: Option<String>,
}

#[derive(Default)]
struct Shared {
    stop: AtomicBool,
    unlock_failures: AtomicU64,
    latest: Mutex<Latest>,
}

#[derive(Serialize)]
pub struct Report {
    pub adapter_luid: String,
    pub maximum_surfaces: usize,
    pub peak_surfaces: usize,
    pub receive_calls: u64,
    pub publications: u64,
    pub pool_busy: u64,
    pub replaced_publications: u64,
    pub truncated_receive_samples: u32,
    pub receive_wall_time_all_windows: Option<Distribution>,
    pub gpu_admission: crate::frame_admission::FinalReport,
    pub timing: crate::timing::TimingReport,
    pub resources: resource_log::Report,
}

pub struct Source {
    shared: Arc<Shared>,
    join: Option<JoinHandle<Result<Report, String>>>,
    current: Option<Frame>,
}

impl Source {
    pub fn spawn(
        adapter: u32,
        luid: &str,
        name: &str,
        sample: std::ops::Range<std::time::Instant>,
    ) -> ProbeResult<Self> {
        let shared = Arc::new(Shared::default());
        let thread_shared = shared.clone();
        let name = name.to_owned();
        let luid = luid.to_owned();
        let join = std::thread::Builder::new()
            .name("probe-source-ingest".into())
            .spawn(move || {
                let result = worker::run(adapter, &luid, &name, sample, &thread_shared).map_err(|e| e.to_string());
                if let Err(error) = &result
                    && let Ok(mut latest) = thread_shared.latest.lock()
                {
                    latest.error = Some(error.clone());
                }
                result
            })?;
        Ok(Self {
            shared,
            join: Some(join),
            current: None,
        })
    }

    /// Poll only metadata and the zero-timeout bridge mutex, never the source SDK/context.
    pub fn poll(&mut self, gpu: &Gpu) -> ProbeResult<bool> {
        if self.shared.unlock_failures.load(Ordering::Relaxed) != 0 {
            return Err("source reader failed to release keyed mutex".into());
        }
        let publication = match self.shared.latest.try_lock() {
            Ok(latest) => {
                if let Some(error) = &latest.error {
                    return Err(error.clone().into());
                }
                latest.publication.clone()
            }
            Err(TryLockError::WouldBlock) => return Ok(false),
            Err(TryLockError::Poisoned(_)) => return Err("source publication poisoned".into()),
        };
        let Some(publication) = publication else {
            self.current = None;
            return Ok(false);
        };
        if self
            .current
            .as_ref()
            .is_some_and(|frame| Arc::ptr_eq(&frame.surface, &publication.surface))
        {
            return Ok(false);
        }
        if let Some(frame) = Frame::open(gpu, publication, self.shared.clone())? {
            self.current = Some(frame);
            return Ok(true);
        }
        Ok(false)
    }

    pub fn frame(&self) -> Option<&Frame> {
        self.current.as_ref()
    }

    pub fn finish(mut self) -> ProbeResult<Report> {
        self.stop();
        let report = self
            .join
            .take()
            .ok_or("source worker already joined")?
            .join()
            .map_err(|_| "source worker panicked")??;
        if self.shared.unlock_failures.load(Ordering::Relaxed) != 0 {
            return Err("source bridge retained failed unlocks".into());
        }
        Ok(report)
    }

    fn stop(&mut self) {
        // Release reader/mailbox ownership before notifying the worker. Its Acquire load
        // may immediately enter final ownership checks, without another pacing interval.
        self.current = None;
        if let Ok(mut latest) = self.shared.latest.lock() {
            latest.publication = None;
        }
        self.shared.stop.store(true, Ordering::Release);
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        self.stop();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

#[cfg(test)]
mod tests;
