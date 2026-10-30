use super::PictureDecoder;
use crate::media::{
    capture::{CapturedAsset, CapturedChannels},
    protocol::ChannelSource,
};
use golden_assets::{
    AssetId, AssetPreparer, AssetRequestId, AssetRetirement, AssetScope, PreparationAdmissionError, PreparationFailure,
    PreparationLimits, PreparedAsset,
};
use golden_io::{DelayedNotifier, WorkerNotifier};
use golden_video::image::ImageData;
use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

struct Entry {
    scope: AssetScope,
    authored: CapturedAsset,
    request: Option<AssetRequestId>,
    value: Option<PreparedAsset<ImageData>>,
    failure: Option<PreparationFailure>,
    retry_at: Option<Instant>,
}

pub(crate) struct Pictures {
    root: PathBuf,
    project: Option<uuid::Uuid>,
    service: AssetPreparer<PictureDecoder>,
    entries: HashMap<AssetId, Entry>,
    retry_wake: Option<DelayedNotifier>,
}

impl Pictures {
    pub fn set_root(&mut self, root: &Path) -> Result<bool, String> {
        AssetScope::new(root).map_err(|error| error.to_string())?;
        if self.root == root {
            return Ok(false);
        }
        self.entries.clear();
        self.project = None;
        self.root = root.to_path_buf();
        Ok(true)
    }
    pub fn new(root: &Path, limits: PreparationLimits, notifier: Option<WorkerNotifier>) -> Result<Self, String> {
        AssetScope::new(root).map_err(|error| error.to_string())?;
        let retry_wake = notifier.clone().map(DelayedNotifier::new).transpose()?;
        let service = match notifier {
            Some(notifier) => AssetPreparer::new_notified(PictureDecoder, limits, notifier),
            None => AssetPreparer::new(PictureDecoder, limits),
        }
        .map_err(|error| error.to_string())?;
        Ok(Self {
            root: root.to_path_buf(),
            project: None,
            service,
            entries: HashMap::new(),
            retry_wake,
        })
    }

    pub fn sync(
        &mut self,
        capture: &CapturedChannels,
        references: &[crate::media::protocol::ChannelReference],
        limit: usize,
    ) -> Result<bool, String> {
        let wanted_routes = references.iter().copied().collect::<HashSet<_>>();
        let demanded = capture
            .channels
            .iter()
            .filter(|channel| channel.enabled && wanted_routes.contains(&channel.reference))
            .filter_map(
                |channel| match channel.definition.as_ref().map(|definition| &definition.source) {
                    Some(ChannelSource::Picture { asset: Some(asset) }) => Some(*asset),
                    _ => None,
                },
            )
            .collect::<HashSet<_>>();
        if demanded.len() > limit {
            return Err("picture demand exceeds admitted source count".into());
        }
        let mut changed = false;
        if self.project != Some(capture.project) {
            self.entries.clear();
            self.project = Some(capture.project);
            changed = true;
        }
        let present = capture
            .assets
            .iter()
            .map(|asset| asset.record.id)
            .collect::<HashSet<_>>();
        self.entries
            .retain(|id, _| demanded.contains(id) && present.contains(id));
        for asset in &capture.assets {
            if !demanded.contains(&asset.record.id) {
                continue;
            }
            let replace = self
                .entries
                .get(&asset.record.id)
                .is_none_or(|entry| !entry.authored.guard.is_current() || entry.authored.record != asset.record);
            if replace {
                self.entries.insert(
                    asset.record.id,
                    Entry {
                        scope: AssetScope::new(&self.root).map_err(|error| error.to_string())?,
                        authored: asset.clone(),
                        request: None,
                        value: None,
                        failure: None,
                        retry_at: None,
                    },
                );
                changed = true;
            }
        }
        changed |= self.service()?;
        Ok(changed)
    }

    pub fn service(&mut self) -> Result<bool, String> {
        let mut outcomes = Vec::new();
        self.service.drain(&mut outcomes, NonZeroUsize::new(32).unwrap());
        let mut changed = false;
        for outcome in outcomes {
            let Some(entry) = self.entries.get_mut(&outcome.asset) else {
                continue;
            };
            if entry.request != Some(outcome.request) {
                continue;
            }
            entry.request = None;
            let result = match outcome.result {
                Ok(value) if value.is_current() => Ok(value),
                Ok(_) => Err(PreparationFailure::Stale),
                Err(error) => Err(error),
            };
            match result {
                Ok(value) => {
                    entry.value = Some(value);
                    entry.failure = None;
                    entry.retry_at = None;
                    changed = true;
                }
                Err(error) => {
                    changed |= entry.failure != Some(error);
                    entry.failure = Some(error);
                    if matches!(
                        error,
                        PreparationFailure::Missing
                            | PreparationFailure::SourceChanged
                            | PreparationFailure::ResourcePressure
                            | PreparationFailure::Io(_)
                    ) {
                        entry.retry_at = Some(Instant::now() + Duration::from_secs(1));
                    }
                }
            }
        }
        for entry in self.entries.values_mut() {
            if entry.request.is_some() || entry.value.is_some() || !entry.authored.guard.is_current() {
                continue;
            }
            if entry.failure.is_some() && entry.retry_at.is_none_or(|at| at > Instant::now()) {
                continue;
            }
            let capture = entry
                .scope
                .capture_record(entry.authored.record.clone(), entry.authored.guard.clone());
            match self.service.submit(capture) {
                Ok(request) => {
                    entry.request = Some(request);
                    entry.retry_at = None;
                }
                Err(PreparationAdmissionError::Full) => break,
                Err(error) => return Err(error.to_string()),
            }
        }
        if let Some(deadline) = self.next_retry()
            && let Some(wake) = &self.retry_wake
        {
            wake.wake_after(deadline.saturating_duration_since(Instant::now()));
        }
        Ok(changed)
    }

    pub fn get(&self, id: AssetId) -> Option<&PreparedAsset<ImageData>> {
        self.entries
            .get(&id)
            .and_then(|entry| entry.value.as_ref())
            .filter(|value| value.is_current())
    }
    pub fn failure(&self, id: AssetId) -> Option<PreparationFailure> {
        self.entries.get(&id).and_then(|entry| entry.failure)
    }

    /// Invoke after a filesystem change/explicit retry. Decoding stays on the worker. Prepared
    /// consumers retain their existing leases; later installation rechecks the scope fence.
    pub fn retry(&mut self) -> Result<(), String> {
        for entry in self.entries.values_mut() {
            entry.scope = AssetScope::new(&self.root).map_err(|error| error.to_string())?;
            entry.request = None;
            entry.value = None;
            entry.failure = None;
            entry.retry_at = None;
        }
        self.service()?;
        Ok(())
    }
    pub fn retry_asset(&mut self, asset: AssetId) -> Result<(), String> {
        if let Some(entry) = self.entries.get_mut(&asset) {
            entry.scope = AssetScope::new(&self.root).map_err(|error| error.to_string())?;
            entry.request = None;
            entry.value = None;
            entry.failure = None;
            entry.retry_at = None;
        }
        self.service()?;
        Ok(())
    }
    pub fn relocate(&mut self, root: &Path) -> Result<(), String> {
        AssetScope::new(root).map_err(|error| error.to_string())?;
        self.root = root.to_path_buf();
        self.retry()
    }
    pub fn retirement(&self) -> AssetRetirement {
        self.service.retirement()
    }
    pub fn next_retry(&self) -> Option<Instant> {
        self.entries.values().filter_map(|entry| entry.retry_at).min()
    }
    pub fn close(&self) {
        if let Some(wake) = &self.retry_wake {
            wake.cancel_deadline();
        }
        self.service.cancel();
    }
}
