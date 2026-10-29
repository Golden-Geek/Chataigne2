use std::path::{Path, PathBuf};
use std::sync::Arc;

use golden_io::{RevisionGuard, RevisionSource};

use crate::{AssetId, AssetLocator, AssetRecord, LocatorError, MAX_ASSET_PATH_BYTES};

fn root(path: &Path) -> Result<Arc<Path>, LocatorError> {
    if !path.is_absolute() {
        return Err(LocatorError::NotAbsolute);
    }
    if path.as_os_str().len() > MAX_ASSET_PATH_BYTES {
        return Err(LocatorError::TooLong);
    }
    if path.as_os_str().as_encoded_bytes().contains(&0) {
        return Err(LocatorError::Nul);
    }
    Ok(Arc::from(path))
}

fn guard(source: &RevisionSource) -> RevisionGuard {
    let reader = source.reader();
    reader.guard_at(reader.current())
}

/// Runtime project lifetime and directory, separate from the app-owned authored catalog.
/// Construction, relocation and capture perform no filesystem access.
pub struct AssetScope {
    root: Arc<Path>,
    revision: RevisionSource,
}

impl AssetScope {
    pub fn new(project_directory: impl AsRef<Path>) -> Result<Self, LocatorError> {
        Ok(Self {
            root: root(project_directory.as_ref())?,
            revision: RevisionSource::default(),
        })
    }

    /// Invalidate prior preparation before changing the resolution root. A project replacement
    /// should create a fresh scope even if its directory is unchanged.
    pub fn relocate(&mut self, project_directory: impl AsRef<Path>) -> Result<(), LocatorError> {
        let next = root(project_directory.as_ref())?;
        if self.root != next {
            self.revision.advance();
            self.root = next;
        }
        Ok(())
    }

    pub fn capture(&self, binding: &AssetBinding) -> AssetCapture {
        self.capture_record(binding.record.clone(), guard(&binding.revision))
    }

    /// A consumer with an existing authored lifetime can reuse its revision fence rather than
    /// maintaining a second catalog/binding clock. The record and guard must be captured together.
    pub fn capture_record(&self, record: AssetRecord, source: RevisionGuard) -> AssetCapture {
        AssetCapture {
            record,
            root: self.root.clone(),
            guards: [guard(&self.revision), source],
        }
    }
}

/// Runtime binding for one authored record. Relink/history restoration keeps the authored ID;
/// replacing/dropping this owner invalidates outstanding preparation even if history keeps JSON.
pub struct AssetBinding {
    record: AssetRecord,
    revision: RevisionSource,
}

impl AssetBinding {
    pub fn new(record: AssetRecord) -> Self {
        Self {
            record,
            revision: RevisionSource::default(),
        }
    }

    pub fn record(&self) -> &AssetRecord {
        &self.record
    }

    pub fn relink(&mut self, locator: AssetLocator) {
        if self.record.locator != locator {
            self.revision.advance();
            self.record.locator = locator;
        }
    }

    /// Filesystem change notifications or an explicit retry invalidate derived work without
    /// changing project truth. Watching/retry policy belongs to the consumer.
    pub fn invalidate(&self) {
        self.revision.advance();
    }
}

/// Immutable bounded CPU input, suitable for capture on a control owner and worker-side use.
#[derive(Clone, Debug)]
pub struct AssetCapture {
    pub(crate) record: AssetRecord,
    root: Arc<Path>,
    pub(crate) guards: [RevisionGuard; 2],
}

impl AssetCapture {
    pub fn id(&self) -> AssetId {
        self.record.id
    }
    pub fn is_current(&self) -> bool {
        self.guards.iter().all(RevisionGuard::is_current)
    }
    pub(crate) fn resolve(&self) -> PathBuf {
        self.record.locator.resolve(&self.root)
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        let path = match &self.record.locator {
            AssetLocator::ProjectRelative { path } => path.as_str(),
            AssetLocator::Absolute { path } => path.as_str(),
        };
        // Charge every shared allocation conservatively, including wide native path storage.
        size_of::<Self>()
            + path.len()
            + self.root.as_os_str().len() * 2
            + 2 * size_of::<usize>()
            + self
                .guards
                .iter()
                .map(RevisionGuard::retained_heap_bytes)
                .sum::<usize>()
    }
}
