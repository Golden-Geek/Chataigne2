//! One bounded IO owner probes authored variants independently of renderer demand.
use super::{VideoControl, registry::Source};
use super::{
    failure::Failure,
    protocol::{VideoSourceError, VideoSourceErrorKind},
};
use crate::{
    app::AppNode,
    media::protocol::{AssetLocator, ChannelId},
};
use golden_core::application::ProductionRuntime;
use golden_io::{BoundedWorker, RetirementPool, WorkerLimits};
use std::{
    collections::{HashMap, HashSet},
    io,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock, Weak},
    time::{Duration, Instant, SystemTime},
};

type Key = (ChannelId, uuid::Uuid);
struct Job {
    control: Weak<VideoControl>,
    path: PathBuf,
    fingerprint: Option<(u64, SystemTime)>,
    retry: Instant,
}
#[derive(Default)]
pub(super) struct Probe {
    jobs: HashMap<Key, Job>,
}
impl Probe {
    pub fn reconcile(&mut self, source: &Source, root: &Path) -> Result<(), String> {
        let mut desired = Vec::<(Key, Arc<VideoControl>, AssetLocator)>::new();
        for control in source.registry.controls() {
            for (channel, generation, locator) in control.metadata_files()? {
                if desired.len() == 4096 {
                    return Err("Video metadata exceeds 4096 configured Files".into());
                }
                desired.push(((channel, generation), control.clone(), locator));
            }
        }
        let wanted = desired.iter().map(|(key, _, _)| *key).collect::<HashSet<_>>();
        self.jobs.retain(|key, _| wanted.contains(key));
        for (key, control, locator) in desired {
            let path = locator.resolve(root);
            if self.jobs.get(&key).is_some_and(|job| job.path == path) {
                continue;
            }
            self.jobs.insert(
                key,
                Job {
                    control: Arc::downgrade(&control),
                    path,
                    fingerprint: None,
                    retry: Instant::now(),
                },
            );
        }
        Ok(())
    }
    pub fn run_due(
        &mut self,
        source: &Source,
        cancelled: impl Fn() -> bool + Clone + Send + 'static,
    ) -> Result<(), String> {
        let now = Instant::now();
        for ((channel, generation), job) in &mut self.jobs {
            if cancelled() || !source.guard.is_current() {
                break;
            }
            if now < job.retry {
                continue;
            }
            job.retry = now + Duration::from_secs(1);
            let fingerprint = std::fs::metadata(&job.path).and_then(|meta| {
                if !meta.is_file() {
                    return Err(io::Error::other("Video source is not a regular file"));
                }
                Ok((meta.len(), meta.modified()?))
            });
            // Endpoint loss retains known duration, but reports its current availability.
            let fingerprint = match fingerprint {
                Ok(fingerprint) => fingerprint,
                Err(error) => {
                    job.fingerprint = None;
                    let kind = if error.kind() == io::ErrorKind::NotFound {
                        VideoSourceErrorKind::Missing
                    } else {
                        VideoSourceErrorKind::Io
                    };
                    if source.guard.is_current()
                        && let Some(control) = job.control.upgrade()
                    {
                        control.metadata_failure(
                            *channel,
                            *generation,
                            VideoSourceError::new(kind, error.to_string()),
                        )?;
                    }
                    continue;
                }
            };
            if job.fingerprint == Some(fingerprint) {
                continue;
            }
            let guard = source.guard.clone();
            let cancel = cancelled.clone();
            let control = job.control.clone();
            let channel_id = *channel;
            let generation_id = *generation;
            let timing = golden_video::decode::probe_timing_with_interrupt(&job.path, move || {
                cancel()
                    || !guard.is_current()
                    || control
                        .upgrade()
                        .is_none_or(|control| !control.metadata_is_current(channel_id, generation_id))
            });
            let duration = timing.map_err(|error| Failure::from(error).0).and_then(|timing| {
                let ticks = timing.duration.filter(|duration| *duration > 0).ok_or_else(|| {
                    VideoSourceError::new(
                        VideoSourceErrorKind::Unsupported,
                        "Video stream duration is unavailable",
                    )
                })?;
                super::reader::duration(ticks, timing.time_base)
                    .map_err(|error| VideoSourceError::new(VideoSourceErrorKind::Unsupported, error))
            });
            let duration = match duration {
                Ok(duration) => duration,
                Err(error) => {
                    job.fingerprint = None;
                    if !cancelled()
                        && source.guard.is_current()
                        && let Some(control) = job.control.upgrade()
                    {
                        control.metadata_failure(*channel, *generation, error)?;
                    }
                    continue;
                }
            };
            if !cancelled()
                && source.guard.is_current()
                && let Some(control) = job.control.upgrade()
                && control.metadata_duration(*channel, *generation, duration)?
            {
                job.fingerprint = Some(fingerprint);
            }
        }
        Ok(())
    }
}

/// Cancellation does not join file IO on the desktop or engine owner.
pub(crate) struct Service {
    _worker: BoundedWorker<(), ()>,
    #[cfg(test)]
    pub(super) pool: RetirementPool,
}
impl Service {
    pub fn start(runtime: ProductionRuntime<AppNode>) -> io::Result<Self> {
        static POOL: OnceLock<RetirementPool> = OnceLock::new();
        let pool = POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(2).unwrap()));
        let worker = BoundedWorker::spawn(
            "mapgyver-video-metadata",
            pool,
            WorkerLimits {
                command_items: 1,
                command_weight: 1,
                event_items: 1,
                event_weight: 1,
            },
            move |context| {
                let mut source: Option<Source> = None;
                let mut probe = Probe::default();
                let mut failure = None;
                while !context.cancellation().is_cancelled() {
                    let checkpoint = context.wake_checkpoint();
                    if source.as_ref().is_none_or(|source| !source.guard.is_current()) {
                        probe = Probe::default();
                        source = runtime.capture_app(Source::capture).ok().and_then(Result::ok);
                    }
                    if let Some(source) = &source {
                        let file = runtime.read_model().current_project_file();
                        let root = file
                            .current_path
                            .as_deref()
                            .and_then(|path| Path::new(path).parent())
                            .map(Path::to_path_buf)
                            .or_else(|| std::env::current_dir().ok());
                        if let Some(root) = root {
                            let cancel = context.cancellation().clone();
                            let result = probe
                                .reconcile(source, &root)
                                .and_then(|()| probe.run_due(source, move || cancel.is_cancelled()));
                            match result {
                                Ok(()) => failure = None,
                                Err(error) => {
                                    if failure.as_ref() != Some(&error) {
                                        eprintln!("[mapgyver-video-metadata] {error}");
                                    }
                                    failure = Some(error);
                                }
                            }
                        }
                    }
                    context.wait_until(checkpoint, Instant::now() + Duration::from_millis(100));
                }
            },
        )
        .map_err(io::Error::other)?;
        Ok(Self {
            _worker: worker,
            #[cfg(test)]
            pool: pool.clone(),
        })
    }
}
