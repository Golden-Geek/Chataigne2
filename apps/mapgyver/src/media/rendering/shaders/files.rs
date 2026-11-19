//! Watch demanded shader files and prepare bounded UTF-8 snapshots outside the renderer.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use golden_assets::{
    AssetBinding, AssetDecoder, AssetId, AssetInput, AssetLocator, AssetPreparer, AssetRecord, AssetRequestId,
    AssetScope, MAX_ASSET_PATH_BYTES, PreparationAdmissionError, PreparationFailure, PreparationLimits,
};
use golden_io::{DelayedNotifier, WorkerNotifier};
use golden_video::shader::MAX_SOURCE_BYTES;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};

const WRITE_SETTLE: Duration = Duration::from_millis(100);
const RETRY: Duration = Duration::from_secs(1);
const MAX_NOTICES: usize = 128;

type Source = Result<Arc<str>, PreparationFailure>;

pub(super) struct Files {
    project: Option<uuid::Uuid>,
    root: PathBuf,
    scope: AssetScope,
    entries: HashMap<AssetLocator, FileEntry>,
    preparer: AssetPreparer<ReadSource>,
    watcher: Option<RecommendedWatcher>,
    watched: HashSet<PathBuf>,
    notices: Arc<Mutex<Notices>>,
    watch_retry: Option<Instant>,
    wake: DelayedNotifier,
}

struct FileEntry {
    path: PathBuf,
    binding: AssetBinding,
    request: Option<AssetRequestId>,
    result: Option<Source>,
    due: Option<Instant>,
}

impl FileEntry {
    fn invalidate(&mut self, at: Instant) {
        self.binding.invalidate();
        self.request = None;
        self.due = Some(at);
    }
}

#[derive(Default)]
struct Notices {
    all: bool,
    paths: HashSet<PathBuf>,
}

impl Notices {
    fn record(&mut self, result: notify::Result<Event>) {
        match result {
            Ok(event) if !event.need_rescan() => {
                for path in event.paths {
                    if self.paths.len() == MAX_NOTICES || path.as_os_str().len() > MAX_ASSET_PATH_BYTES {
                        self.all = true;
                        self.paths.clear();
                        return;
                    }
                    self.paths.insert(path);
                }
            }
            _ => {
                self.all = true;
                self.paths.clear();
            }
        }
    }
}

impl Files {
    pub(super) fn new(root: &Path, notify: WorkerNotifier) -> Result<Self, String> {
        let notices = Arc::new(Mutex::new(Notices::default()));
        let watcher = notify::recommended_watcher({
            let notices = notices.clone();
            let notify = notify.clone();
            move |event: notify::Result<Event>| {
                if event
                    .as_ref()
                    .is_ok_and(|event| event.kind.is_access() && !event.need_rescan())
                {
                    return;
                }
                notices.lock().unwrap_or_else(|error| error.into_inner()).record(event);
                notify.notify();
            }
        });
        let watcher = match watcher {
            Ok(watcher) => Some(watcher),
            Err(error) => {
                eprintln!("[mapgyver-shader] File watch unavailable. Using periodic reload checks. {error}");
                None
            }
        };
        let preparer = AssetPreparer::new_notified(
            ReadSource,
            PreparationLimits {
                max_source_bytes: MAX_SOURCE_BYTES as u64,
                max_decode_bytes: NonZeroUsize::new(MAX_SOURCE_BYTES * 3).unwrap(),
                ..Default::default()
            },
            notify.clone(),
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            project: None,
            root: root.to_path_buf(),
            scope: AssetScope::new(root).map_err(|error| error.to_string())?,
            entries: HashMap::new(),
            preparer,
            watcher,
            watched: HashSet::new(),
            notices,
            watch_retry: None,
            wake: DelayedNotifier::new(notify)?,
        })
    }

    pub(super) fn set_root(&mut self, root: &Path) -> Result<bool, String> {
        if root == self.root {
            return Ok(false);
        }
        self.scope.relocate(root).map_err(|error| error.to_string())?;
        self.root = root.to_path_buf();
        self.entries.clear();
        self.sync_watches(Instant::now());
        Ok(true)
    }

    pub(super) fn sync(&mut self, project: uuid::Uuid, wanted: HashSet<AssetLocator>) {
        let mut changed = self.project != Some(project);
        if changed {
            self.entries.clear();
            self.project = Some(project);
        }
        let count = self.entries.len();
        self.entries.retain(|file, _| wanted.contains(file));
        changed |= self.entries.len() != count;
        for locator in wanted {
            if let std::collections::hash_map::Entry::Vacant(entry) = self.entries.entry(locator.clone()) {
                entry.insert(FileEntry {
                    path: locator.resolve(&self.root),
                    binding: AssetBinding::new(AssetRecord {
                        id: AssetId::new(),
                        locator,
                    }),
                    request: None,
                    result: None,
                    due: Some(Instant::now()),
                });
                changed = true;
            }
        }
        if changed {
            self.sync_watches(Instant::now());
        }
    }

    pub(super) fn get(&self, file: &AssetLocator) -> Option<&Source> {
        self.entries.get(file)?.result.as_ref()
    }

    pub(super) fn service(&mut self) -> Result<bool, String> {
        let now = Instant::now();
        let notices = std::mem::take(&mut *self.notices.lock().unwrap_or_else(|error| error.into_inner()));
        let watch_retry = self.watch_retry.is_some_and(|at| at <= now);
        let mut renew_watches = watch_retry || notices.all;
        for entry in self.entries.values_mut() {
            let directory = entry.path.parent();
            let changed_directory = directory.is_some_and(|dir| notices.paths.iter().any(|path| same_path(path, dir)));
            let unwatched = watch_retry && directory.is_none_or(|dir| !self.watched.contains(dir));
            let changed_file = notices.all
                || changed_directory
                || unwatched
                || notices.paths.iter().any(|path| same_path(path, &entry.path));
            if (changed_directory || notices.all)
                && let Some(dir) = directory
            {
                if self.watched.remove(dir)
                    && let Some(watcher) = &mut self.watcher
                {
                    let _ = watcher.unwatch(dir);
                }
                renew_watches = true;
            }
            if changed_file {
                entry.invalidate(now + WRITE_SETTLE);
            }
        }
        if renew_watches {
            self.sync_watches(now);
        }

        let mut outcomes = Vec::new();
        let mut changed = false;
        self.preparer.drain(&mut outcomes, NonZeroUsize::new(8).unwrap());
        for outcome in outcomes {
            let Some(entry) = self
                .entries
                .values_mut()
                .find(|entry| entry.binding.record().id == outcome.asset && entry.request == Some(outcome.request))
            else {
                continue;
            };
            entry.request = None;
            let result = outcome.result.and_then(|value| {
                value
                    .is_current()
                    .then(|| value.value().clone())
                    .ok_or(PreparationFailure::Stale)
            });
            changed |= entry.result.as_ref() != Some(&result);
            entry.due = match result {
                Err(
                    PreparationFailure::Missing
                    | PreparationFailure::Io(_)
                    | PreparationFailure::SourceChanged
                    | PreparationFailure::ResourcePressure
                    | PreparationFailure::Stale,
                ) => Some(now + RETRY),
                _ => None,
            };
            entry.result = Some(result);
        }
        for entry in self.entries.values_mut() {
            if entry.request.is_some() || entry.due.is_none_or(|at| at > now) {
                continue;
            }
            match self.preparer.submit(self.scope.capture(&entry.binding)) {
                Ok(request) => {
                    entry.request = Some(request);
                    entry.due = None;
                }
                Err(PreparationAdmissionError::Full) => break,
                Err(error) => return Err(error.to_string()),
            }
        }
        // A full command queue is driven by its next outcome, never a zero-delay retry loop.
        if let Some(at) = self
            .entries
            .values()
            .filter_map(|entry| entry.due)
            .chain(self.watch_retry)
            .filter(|at| *at > now)
            .min()
        {
            self.wake.wake_after(at - now);
        }
        Ok(changed)
    }

    fn sync_watches(&mut self, now: Instant) {
        let directories = self
            .entries
            .values()
            .filter_map(|entry| entry.path.parent())
            .collect::<HashSet<_>>();
        if let Some(watcher) = &mut self.watcher {
            self.watched.retain(|path| {
                if directories.contains(path.as_path()) {
                    true
                } else {
                    let _ = watcher.unwatch(path);
                    false
                }
            });
            for directory in &directories {
                if !self.watched.contains(*directory) && watcher.watch(directory, RecursiveMode::NonRecursive).is_ok() {
                    self.watched.insert(directory.to_path_buf());
                }
            }
        }
        self.watch_retry = (directories.len() != self.watched.len()).then_some(now + RETRY);
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    left == right
        || left
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(&right.as_os_str().to_string_lossy())
}

struct ReadSource;

impl AssetDecoder for ReadSource {
    type Output = Arc<str>;

    fn decode(&mut self, input: &mut AssetInput, maximum_bytes: usize) -> Result<Self::Output, PreparationFailure> {
        let length = input.length() as usize;
        if length.saturating_mul(2) > maximum_bytes {
            return Err(PreparationFailure::SourceTooLarge);
        }
        let mut bytes = vec![0; length];
        input.read_exact(&mut bytes).map_err(PreparationFailure::from)?;
        let text = String::from_utf8(bytes).map_err(|_| PreparationFailure::Decode)?;
        crate::media::shader::validate_code(&text).map_err(|_| PreparationFailure::Decode)?;
        Ok(Arc::from(text))
    }

    fn retained_bytes(&self, value: &Self::Output) -> usize {
        value.len()
    }
}
