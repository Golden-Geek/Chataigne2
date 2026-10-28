//! SDK receive/lifecycle work owns a separate device/context on each source thread.

mod imports;
mod surface;
mod worker;

use crate::{ProbeResult, d3d::Gpu};
pub use imports::{Frame, ImportReport};
use imports::{Imports, Stage};
use mapgyver_native_probe::{resource_log, statistics::Distribution};
use serde::Serialize;
use std::sync::{
    Arc, Mutex, TryLockError,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::thread::JoinHandle;
use std::time::Instant;

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
    imports: Option<Imports>,
    import_report: Option<ImportReport>,
}

pub struct Finished {
    pub worker: Report,
    pub renderer_imports: ImportReport,
}

impl Source {
    pub fn spawn(gpu: &Gpu, name: &str, sample: std::ops::Range<std::time::Instant>) -> ProbeResult<Self> {
        let imports = Imports::new(gpu, CAPACITY)?;
        let shared = Arc::new(Shared::default());
        let thread_shared = shared.clone();
        let name = name.to_owned();
        let adapter = gpu.adapter.index;
        let luid = gpu.adapter.luid.clone();
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
            imports: Some(imports),
            import_report: None,
        })
    }

    /// Poll only metadata and the zero-timeout bridge mutex, never the source SDK/context.
    pub fn poll(&mut self, collecting: bool) -> ProbeResult<bool> {
        if self.shared.unlock_failures.load(Ordering::Relaxed) != 0 {
            return Err("source reader failed to release keyed mutex".into());
        }
        let imports = self.imports.as_mut().ok_or("source imports already retired")?;
        imports.count(collecting, |c| c.polls += 1);
        let start = Instant::now();
        let snapshot = (|| -> ProbeResult<_> {
            match self.shared.latest.try_lock() {
                Ok(latest) => {
                    if let Some(error) = &latest.error {
                        return Err(error.clone().into());
                    }
                    Ok((latest.publication.clone(), false))
                }
                Err(TryLockError::WouldBlock) => Ok((None, true)),
                Err(TryLockError::Poisoned(_)) => Err("source publication poisoned".into()),
            }
        })();
        // Timing storage may allocate; it must never extend the mailbox critical section.
        imports.record(Stage::Metadata, start, collecting);
        let (publication, busy) = snapshot?;
        if busy {
            imports.count(collecting, |c| c.metadata_busy += 1);
            return Ok(false);
        }
        let Some(publication) = publication else {
            imports.count(collecting, |c| {
                c.no_publication += 1;
                c.reader_clears += u64::from(self.current.is_some());
            });
            imports.release_reader(self.current.take(), collecting);
            imports.clear(collecting);
            return Ok(false);
        };
        if self
            .current
            .as_ref()
            .is_some_and(|frame| Arc::ptr_eq(&frame.surface, &publication.surface))
        {
            imports.count(collecting, |c| c.same_surface += 1);
            return Ok(false);
        }
        if let Some(frame) = imports.open(publication, self.shared.clone(), collecting)? {
            imports.release_reader(self.current.replace(frame), collecting);
            imports.count(collecting, |c| c.reader_changes += 1);
            return Ok(true);
        }
        Ok(false)
    }

    pub fn frame(&self) -> Option<&Frame> {
        self.current.as_ref()
    }

    pub fn finish(mut self) -> ProbeResult<Finished> {
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
        let renderer_imports = self.import_report.take().ok_or("source import report absent")?;
        if renderer_imports.cached_after_clear != 0 || renderer_imports.live_imports_after_clear != 0 {
            return Err("renderer source imports survived retirement".into());
        }
        Ok(Finished {
            worker: report,
            renderer_imports,
        })
    }

    fn stop(&mut self) {
        // Release reader/mailbox ownership before notifying the worker. Its Acquire load
        // may immediately enter final ownership checks, without another pacing interval.
        if let Some(mut imports) = self.imports.take() {
            imports.count(false, |c| c.reader_clears += u64::from(self.current.is_some()));
            imports.release_reader(self.current.take(), false);
            self.import_report = Some(imports.finish());
        } else {
            self.current = None;
        }
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
