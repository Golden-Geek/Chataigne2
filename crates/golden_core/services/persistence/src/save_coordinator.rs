use std::collections::{BTreeMap, HashMap, HashSet};
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::{Duration, Instant};

use crate::{RecoveryPaths, write_file_atomically_with_recovery};

type PersistenceWriter = dyn Fn(&Path, &[u8]) -> io::Result<RecoveryPaths> + Send + Sync + 'static;

/// Stable, platform-aware identity for one persistence destination.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DestinationIdentity(PathBuf);

impl DestinationIdentity {
    /// Returns the normalized identity path used for transaction coordination.
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// Immutable identity and ordering metadata assigned before document encoding begins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveTicketInfo {
    /// Monotonic request identity across this coordinator.
    pub request_id: u64,
    /// Authoritative project generation captured for this save.
    pub project_generation: u64,
    /// Authored document revision captured for this save.
    pub document_revision: u64,
    /// Normalized destination used for alias-safe coordination.
    pub destination: DestinationIdentity,
    /// Absolute lexical path used for the physical transaction.
    pub target: PathBuf,
}

/// Accepted save reservation. Dropping it before commit unblocks later saves.
pub struct SaveTicket {
    inner: Weak<CoordinatorInner>,
    info: SaveTicketInfo,
    pending: bool,
}

impl SaveTicket {
    /// Returns the immutable acceptance metadata.
    pub fn info(&self) -> &SaveTicketInfo {
        &self.info
    }

    fn disarm(&mut self) {
        self.pending = false;
    }
}

impl std::fmt::Debug for SaveTicket {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SaveTicket")
            .field("info", &self.info)
            .field("pending", &self.pending)
            .finish()
    }
}

impl Drop for SaveTicket {
    fn drop(&mut self) {
        if !self.pending {
            return;
        }
        if let Some(inner) = self.inner.upgrade() {
            inner.cancel_pending(&self.info);
        }
    }
}

/// Successful durable transaction and its host-publication result.
#[derive(Debug)]
pub struct CoordinatedSave<R> {
    /// Acceptance metadata that won this physical transaction.
    pub ticket: SaveTicketInfo,
    /// Durable primary, backup, and journal paths.
    pub paths: RecoveryPaths,
    /// Time spent waiting for destination order, capacity, or a replacement fence.
    pub coordination_wait: Duration,
    /// Time spent in backup, journal, temporary-file, target replacement, and cleanup work.
    pub write: Duration,
    /// Result returned by the post-commit metadata publication callback.
    pub publication: R,
}

/// Persistence ordering or generation-fence failure.
#[derive(Debug)]
pub enum PersistenceCoordinatorError {
    /// The destination could not be normalized or written.
    Io(io::Error),
    /// A save or replacement refers to a project generation that is no longer authoritative.
    StaleGeneration {
        /// Generation supplied by the caller.
        requested: u64,
        /// Generation currently accepted by the coordinator.
        current: u64,
    },
    /// A ticket was canceled or otherwise no longer belongs to the pending queue.
    TicketNotPending {
        /// Monotonic ticket identity.
        request_id: u64,
    },
    /// A committed replacement generation did not advance monotonically.
    InvalidReplacementGeneration {
        /// Generation guarded by the replacement fence.
        previous: u64,
        /// Proposed replacement generation.
        next: u64,
    },
    /// The process exhausted the monotonic save-request identity space.
    RequestIdExhausted,
}

impl std::fmt::Display for PersistenceCoordinatorError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "persistence transaction failed: {error}"),
            Self::StaleGeneration { requested, current } => write!(
                formatter,
                "project generation {requested} is stale; current persistence generation is {current}"
            ),
            Self::TicketNotPending { request_id } => {
                write!(formatter, "save request {request_id} is no longer pending")
            }
            Self::InvalidReplacementGeneration { previous, next } => write!(
                formatter,
                "replacement generation must advance beyond {previous}, got {next}"
            ),
            Self::RequestIdExhausted => formatter.write_str("persistence save request identity space is exhausted"),
        }
    }
}

impl std::error::Error for PersistenceCoordinatorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for PersistenceCoordinatorError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Debug)]
struct CoordinatorState {
    active_generation: u64,
    next_request_id: u64,
    replacement_pending: bool,
    active_commits: usize,
    pending_by_destination: HashMap<DestinationIdentity, BTreeMap<u64, u64>>,
    active_destinations: HashSet<DestinationIdentity>,
}

struct CoordinatorInner {
    state: Mutex<CoordinatorState>,
    changed: Condvar,
    max_concurrent_commits: usize,
    writer: Arc<PersistenceWriter>,
}

impl CoordinatorInner {
    fn lock_state(&self) -> std::sync::MutexGuard<'_, CoordinatorState> {
        self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn wait<'a>(
        &self,
        state: std::sync::MutexGuard<'a, CoordinatorState>,
    ) -> std::sync::MutexGuard<'a, CoordinatorState> {
        self.changed
            .wait(state)
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn cancel_pending(&self, info: &SaveTicketInfo) {
        let mut state = self.lock_state();
        remove_pending(&mut state, info);
        self.changed.notify_all();
    }

    fn finish_commit(&self, destination: &DestinationIdentity) {
        let mut state = self.lock_state();
        state.active_destinations.remove(destination);
        state.active_commits = state.active_commits.saturating_sub(1);
        self.changed.notify_all();
    }
}

struct ActiveCommit {
    inner: Arc<CoordinatorInner>,
    destination: DestinationIdentity,
}

impl Drop for ActiveCommit {
    fn drop(&mut self) {
        self.inner.finish_commit(&self.destination);
    }
}

/// Coordinates save ordering, destination aliases, bounded concurrency, and project replacement.
#[derive(Clone)]
pub struct PersistenceCoordinator {
    inner: Arc<CoordinatorInner>,
}

impl std::fmt::Debug for PersistenceCoordinator {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.inner.lock_state();
        formatter
            .debug_struct("PersistenceCoordinator")
            .field("active_generation", &state.active_generation)
            .field("next_request_id", &state.next_request_id)
            .field("replacement_pending", &state.replacement_pending)
            .field("active_commits", &state.active_commits)
            .field("max_concurrent_commits", &self.inner.max_concurrent_commits)
            .finish()
    }
}

impl PersistenceCoordinator {
    /// Creates a coordinator for an already-authoritative initial project generation.
    pub fn new(initial_generation: u64, max_concurrent_commits: usize) -> Self {
        Self::with_writer(
            initial_generation,
            max_concurrent_commits,
            Arc::new(|path, contents| write_file_atomically_with_recovery(path, contents)),
        )
    }

    fn with_writer(initial_generation: u64, max_concurrent_commits: usize, writer: Arc<PersistenceWriter>) -> Self {
        Self {
            inner: Arc::new(CoordinatorInner {
                state: Mutex::new(CoordinatorState {
                    active_generation: initial_generation,
                    next_request_id: 1,
                    replacement_pending: false,
                    active_commits: 0,
                    pending_by_destination: HashMap::new(),
                    active_destinations: HashSet::new(),
                }),
                changed: Condvar::new(),
                max_concurrent_commits: max_concurrent_commits.max(1),
                writer,
            }),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_writer_for_tests<F>(initial_generation: u64, max_concurrent_commits: usize, writer: F) -> Self
    where
        F: Fn(&Path, &[u8]) -> io::Result<RecoveryPaths> + Send + Sync + 'static,
    {
        Self::with_writer(initial_generation, max_concurrent_commits, Arc::new(writer))
    }

    /// Accepts one save after capture and before independently scheduled encoding.
    ///
    /// Acceptance waits for an in-progress replacement fence, then rejects stale captures. Dropping
    /// the returned ticket cancels its queue position.
    pub fn accept_save(
        &self,
        path: impl AsRef<Path>,
        project_generation: u64,
        document_revision: u64,
    ) -> Result<SaveTicket, PersistenceCoordinatorError> {
        let (destination, target) = normalize_destination(path.as_ref())?;
        let mut state = self.inner.lock_state();
        while state.replacement_pending {
            state = self.inner.wait(state);
        }
        if project_generation != state.active_generation {
            return Err(PersistenceCoordinatorError::StaleGeneration {
                requested: project_generation,
                current: state.active_generation,
            });
        }
        let request_id = state.next_request_id;
        state.next_request_id = state
            .next_request_id
            .checked_add(1)
            .ok_or(PersistenceCoordinatorError::RequestIdExhausted)?;
        state
            .pending_by_destination
            .entry(destination.clone())
            .or_default()
            .insert(request_id, project_generation);
        Ok(SaveTicket {
            inner: Arc::downgrade(&self.inner),
            info: SaveTicketInfo {
                request_id,
                project_generation,
                document_revision,
                destination,
                target,
            },
            pending: true,
        })
    }

    /// Commits an accepted save in destination order and publishes metadata before releasing its
    /// replacement lease.
    pub fn commit_save<R>(
        &self,
        mut ticket: SaveTicket,
        contents: &[u8],
        publish: impl FnOnce(&SaveTicketInfo) -> R,
    ) -> Result<CoordinatedSave<R>, PersistenceCoordinatorError> {
        let wait_started = Instant::now();
        let active = self.admit_commit(&ticket.info)?;
        let coordination_wait = wait_started.elapsed();
        ticket.disarm();
        let info = ticket.info.clone();
        let write_started = Instant::now();
        let paths = (self.inner.writer)(&info.target, contents)?;
        let write = write_started.elapsed();
        let publication = publish(&info);
        drop(active);
        Ok(CoordinatedSave {
            ticket: info,
            paths,
            coordination_wait,
            write,
            publication,
        })
    }

    /// Fences project cutover against complete physical save transactions.
    ///
    /// Already-admitted commits and their metadata publication finish first. Accepted saves that
    /// have not started writing are invalidated only when the fence commits a new generation.
    pub fn begin_generation_replacement(
        &self,
        expected_generation: u64,
    ) -> Result<GenerationReplacementFence, PersistenceCoordinatorError> {
        let mut state = self.inner.lock_state();
        while state.replacement_pending {
            state = self.inner.wait(state);
        }
        if state.active_generation != expected_generation {
            return Err(PersistenceCoordinatorError::StaleGeneration {
                requested: expected_generation,
                current: state.active_generation,
            });
        }
        state.replacement_pending = true;
        while state.active_commits != 0 {
            state = self.inner.wait(state);
        }
        Ok(GenerationReplacementFence {
            inner: Arc::clone(&self.inner),
            previous_generation: expected_generation,
            resolved: false,
        })
    }

    fn admit_commit(&self, info: &SaveTicketInfo) -> Result<ActiveCommit, PersistenceCoordinatorError> {
        let mut state = self.inner.lock_state();
        loop {
            if info.project_generation != state.active_generation {
                remove_pending(&mut state, info);
                self.inner.changed.notify_all();
                return Err(PersistenceCoordinatorError::StaleGeneration {
                    requested: info.project_generation,
                    current: state.active_generation,
                });
            }
            let is_pending = state
                .pending_by_destination
                .get(&info.destination)
                .is_some_and(|pending| pending.contains_key(&info.request_id));
            if !is_pending {
                return Err(PersistenceCoordinatorError::TicketNotPending {
                    request_id: info.request_id,
                });
            }
            let is_next = state
                .pending_by_destination
                .get(&info.destination)
                .and_then(|pending| pending.first_key_value())
                .is_some_and(|(request_id, _)| *request_id == info.request_id);
            if !state.replacement_pending
                && is_next
                && !state.active_destinations.contains(&info.destination)
                && state.active_commits < self.inner.max_concurrent_commits
            {
                remove_pending(&mut state, info);
                state.active_destinations.insert(info.destination.clone());
                state.active_commits += 1;
                return Ok(ActiveCommit {
                    inner: Arc::clone(&self.inner),
                    destination: info.destination.clone(),
                });
            }
            state = self.inner.wait(state);
        }
    }
}

/// Exclusive persistence fence held across one project-generation cutover.
pub struct GenerationReplacementFence {
    inner: Arc<CoordinatorInner>,
    previous_generation: u64,
    resolved: bool,
}

impl GenerationReplacementFence {
    /// Publishes the new generation and invalidates all unstarted saves from older generations.
    pub fn commit(mut self, next_generation: u64) -> Result<(), PersistenceCoordinatorError> {
        if next_generation <= self.previous_generation {
            return Err(PersistenceCoordinatorError::InvalidReplacementGeneration {
                previous: self.previous_generation,
                next: next_generation,
            });
        }
        let mut state = self.inner.lock_state();
        state.active_generation = next_generation;
        for pending in state.pending_by_destination.values_mut() {
            pending.retain(|_, generation| *generation == next_generation);
        }
        state.pending_by_destination.retain(|_, pending| !pending.is_empty());
        state.replacement_pending = false;
        self.resolved = true;
        self.inner.changed.notify_all();
        Ok(())
    }
}

impl std::fmt::Debug for GenerationReplacementFence {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GenerationReplacementFence")
            .field("previous_generation", &self.previous_generation)
            .field("resolved", &self.resolved)
            .finish()
    }
}

impl Drop for GenerationReplacementFence {
    fn drop(&mut self) {
        if self.resolved {
            return;
        }
        let mut state = self.inner.lock_state();
        state.replacement_pending = false;
        self.inner.changed.notify_all();
    }
}

/// Resolves a destination through existing ancestors and platform path semantics.
pub fn normalize_destination_identity(path: impl AsRef<Path>) -> io::Result<DestinationIdentity> {
    normalize_destination(path.as_ref()).map(|(identity, _)| identity)
}

fn normalize_destination(path: &Path) -> io::Result<(DestinationIdentity, PathBuf)> {
    if path.as_os_str().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "persistence destination is empty",
        ));
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let target = lexical_normalize(&absolute);
    let mut probe = target.clone();
    let mut missing = Vec::new();
    let canonical_base = loop {
        match std::fs::canonicalize(&probe) {
            Ok(canonical) => break canonical,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let Some(name) = probe.file_name().map(|name| name.to_os_string()) else {
                    return Err(error);
                };
                missing.push(name);
                if !probe.pop() {
                    return Err(error);
                }
            }
            Err(error) => return Err(error),
        }
    };
    let mut identity_path = canonical_base;
    for component in missing.into_iter().rev() {
        identity_path.push(component);
    }
    #[cfg(windows)]
    let identity_path = PathBuf::from(identity_path.to_string_lossy().to_lowercase());
    Ok((DestinationIdentity(identity_path), target))
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

fn remove_pending(state: &mut CoordinatorState, info: &SaveTicketInfo) {
    let remove_destination = if let Some(pending) = state.pending_by_destination.get_mut(&info.destination) {
        pending.remove(&info.request_id);
        pending.is_empty()
    } else {
        false
    };
    if remove_destination {
        state.pending_by_destination.remove(&info.destination);
    }
}
