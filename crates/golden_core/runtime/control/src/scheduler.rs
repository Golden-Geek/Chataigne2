use std::fmt;
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};

use crate::{KernelId, RuntimeMetrics, WorkUnitId};

/// One compile-assigned unit of semantic work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScheduledWork {
    /// Dense work id and deterministic result position.
    pub id: WorkUnitId,
    /// Shared compiled kernel.
    pub kernel: KernelId,
    /// First multiplex lane.
    pub first_lane: u32,
    /// Number of contiguous lanes.
    pub lane_count: u32,
}

/// Persistent deterministic runtime schedule.
#[derive(Clone, Debug, Default)]
pub struct RuntimeSchedule {
    units: Arc<[ScheduledWork]>,
    dense_threshold: f32,
}

impl RuntimeSchedule {
    /// Creates a schedule with a sparse/dense switch threshold in `0.0..=1.0`.
    pub fn new(units: Vec<ScheduledWork>, dense_threshold: f32) -> Result<Self, SchedulerError> {
        if !(0.0..=1.0).contains(&dense_threshold) {
            return Err(SchedulerError::InvalidDenseThreshold);
        }
        if units.iter().enumerate().any(|(index, unit)| unit.id.index() != index) {
            return Err(SchedulerError::WorkIdsNotDense);
        }
        Ok(Self {
            units: units.into(),
            dense_threshold,
        })
    }

    /// Returns compile-assigned work units in deterministic order.
    pub fn units(&self) -> &[ScheduledWork] {
        &self.units
    }

    /// Returns the dense work-unit count.
    pub fn work_count(&self) -> usize {
        self.units.len()
    }

    fn dense_threshold(&self) -> f32 {
        self.dense_threshold
    }
}

/// Reusable dense dirty bitset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirtySet {
    words: Vec<u64>,
    len: usize,
    count: usize,
}

impl DirtySet {
    /// Allocates a bitset for a stable work layout.
    pub fn new(len: usize) -> Self {
        Self {
            words: vec![0; len.div_ceil(64)],
            len,
            count: 0,
        }
    }

    /// Marks a work unit dirty.
    pub fn mark(&mut self, unit: WorkUnitId) -> Result<(), SchedulerError> {
        let index = unit.index();
        if index >= self.len {
            return Err(SchedulerError::WorkOutOfBounds);
        }
        let bit = 1_u64 << (index % 64);
        let word = &mut self.words[index / 64];
        if *word & bit == 0 {
            *word |= bit;
            self.count += 1;
        }
        Ok(())
    }

    /// Marks every work unit dirty without allocating.
    pub fn mark_all(&mut self) {
        self.words.fill(u64::MAX);
        let trailing = self.words.len() * 64 - self.len;
        if let Some(last) = self.words.last_mut()
            && trailing > 0
        {
            *last &= u64::MAX >> trailing;
        }
        self.count = self.len;
    }

    /// Clears the bitset for reuse.
    pub fn clear(&mut self) {
        self.words.fill(0);
        self.count = 0;
    }

    /// Returns whether one work unit is dirty.
    pub fn contains(&self, unit: WorkUnitId) -> bool {
        let index = unit.index();
        index < self.len && self.words[index / 64] & (1_u64 << (index % 64)) != 0
    }

    /// Returns the number of dirty units.
    pub const fn count(&self) -> usize {
        self.count
    }
}

/// Scheduler path selected for one batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Visit dirty bits only.
    Sparse,
    /// Scan the stable dense schedule.
    Dense,
}

/// Pure executor installed once into a persistent worker pool.
pub trait BatchExecutor: Send + Sync + 'static {
    /// Work result written into a deterministic output position.
    type Output: Send + 'static;

    /// Executes one compile-assigned unit.
    fn execute(&self, work: ScheduledWork) -> Self::Output;
}

impl<O, F> BatchExecutor for F
where
    O: Send + 'static,
    F: Fn(ScheduledWork) -> O + Send + Sync + 'static,
{
    type Output = O;

    fn execute(&self, work: ScheduledWork) -> Self::Output {
        self(work)
    }
}

/// Deterministically positioned results from one scheduler batch.
#[derive(Debug)]
pub struct BatchExecution<O> {
    /// Sparse or dense selection path.
    pub mode: ExecutionMode,
    /// Work results in compile-assigned work-id order.
    pub outputs: Vec<(WorkUnitId, O)>,
}

struct WorkerJob {
    ordinal: usize,
    work: ScheduledWork,
}

struct WorkerResult<O> {
    ordinal: usize,
    work: WorkUnitId,
    output: O,
}

struct BatchScratch<O> {
    selected: Vec<ScheduledWork>,
    positioned: Vec<Option<(WorkUnitId, O)>>,
}

impl<O> Default for BatchScratch<O> {
    fn default() -> Self {
        Self {
            selected: Vec::new(),
            positioned: Vec::new(),
        }
    }
}

enum WorkerMessage {
    Run(WorkerJob),
    Shutdown,
}

/// Persistent CPU worker pool with deterministic result placement.
pub struct PersistentBatchScheduler<E: BatchExecutor> {
    jobs: mpsc::Sender<WorkerMessage>,
    results: Mutex<mpsc::Receiver<WorkerResult<E::Output>>>,
    scratch: Mutex<BatchScratch<E::Output>>,
    workers: Vec<JoinHandle<()>>,
    metrics: Arc<RuntimeMetrics>,
}

impl<E: BatchExecutor> PersistentBatchScheduler<E> {
    /// Initializes workers once. `worker_count` must be nonzero.
    pub fn new(worker_count: usize, executor: E, metrics: Arc<RuntimeMetrics>) -> Result<Self, SchedulerError> {
        if worker_count == 0 {
            return Err(SchedulerError::NoWorkers);
        }
        let executor = Arc::new(executor);
        let (job_tx, job_rx) = mpsc::channel();
        let job_rx = Arc::new(Mutex::new(job_rx));
        let (result_tx, result_rx) = mpsc::channel();
        let mut workers = Vec::with_capacity(worker_count);
        for index in 0..worker_count {
            let executor = executor.clone();
            let job_rx = job_rx.clone();
            let result_tx = result_tx.clone();
            let worker = thread::Builder::new()
                .name(format!("golden-runtime-{index}"))
                .spawn(move || worker_loop(executor, job_rx, result_tx))
                .map_err(|error| SchedulerError::WorkerStart(Arc::from(error.to_string())))?;
            workers.push(worker);
        }
        Ok(Self {
            jobs: job_tx,
            results: Mutex::new(result_rx),
            scratch: Mutex::new(BatchScratch::default()),
            workers,
            metrics,
        })
    }

    /// Executes the selected batch and returns results without completion-order sorting.
    pub fn execute(
        &self,
        schedule: &RuntimeSchedule,
        dirty: &DirtySet,
    ) -> Result<BatchExecution<E::Output>, SchedulerError> {
        let mut outputs = Vec::with_capacity(dirty.count());
        let mode = self.execute_into(schedule, dirty, &mut outputs)?;
        Ok(BatchExecution { mode, outputs })
    }

    /// Executes into caller-owned output storage while reusing internal selection and result
    /// positioning buffers. Production callers keep the output vector across ticks, avoiding
    /// work-proportional vector allocation after the schedule has warmed up.
    pub fn execute_into(
        &self,
        schedule: &RuntimeSchedule,
        dirty: &DirtySet,
        outputs: &mut Vec<(WorkUnitId, E::Output)>,
    ) -> Result<ExecutionMode, SchedulerError> {
        if dirty.len != schedule.work_count() {
            return Err(SchedulerError::DirtyLayoutMismatch);
        }
        outputs.clear();
        let density = if dirty.len == 0 {
            0.0
        } else {
            dirty.count as f32 / dirty.len as f32
        };
        let mode = if density >= schedule.dense_threshold() {
            ExecutionMode::Dense
        } else {
            ExecutionMode::Sparse
        };
        let mut scratch = self.scratch.lock().map_err(|_| SchedulerError::WorkerDisconnected)?;
        scratch.selected.clear();
        for work in schedule.units() {
            if dirty.contains(work.id) {
                scratch.selected.push(*work);
            }
        }
        for (ordinal, work) in scratch.selected.iter().copied().enumerate() {
            self.jobs
                .send(WorkerMessage::Run(WorkerJob { ordinal, work }))
                .map_err(|_| SchedulerError::WorkerDisconnected)?;
        }
        let selected_len = scratch.selected.len();
        scratch.positioned.clear();
        scratch.positioned.resize_with(selected_len, || None);
        let results = self.results.lock().map_err(|_| SchedulerError::WorkerDisconnected)?;
        for _ in 0..selected_len {
            let result = results.recv().map_err(|_| SchedulerError::WorkerDisconnected)?;
            scratch.positioned[result.ordinal] = Some((result.work, result.output));
        }
        outputs.extend(
            scratch
                .positioned
                .drain(..)
                .map(|output| output.expect("every admitted work item returns one result")),
        );
        self.metrics.batch_finished(mode == ExecutionMode::Dense, selected_len);
        Ok(mode)
    }
}

impl<E: BatchExecutor> Drop for PersistentBatchScheduler<E> {
    fn drop(&mut self) {
        for _ in &self.workers {
            let _ = self.jobs.send(WorkerMessage::Shutdown);
        }
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

fn worker_loop<E: BatchExecutor>(
    executor: Arc<E>,
    jobs: Arc<Mutex<mpsc::Receiver<WorkerMessage>>>,
    results: mpsc::Sender<WorkerResult<E::Output>>,
) {
    loop {
        let message = match jobs.lock() {
            Ok(receiver) => receiver.recv(),
            Err(_) => return,
        };
        match message {
            Ok(WorkerMessage::Run(job)) => {
                let output = executor.execute(job.work);
                if results
                    .send(WorkerResult {
                        ordinal: job.ordinal,
                        work: job.work.id,
                        output,
                    })
                    .is_err()
                {
                    return;
                }
            }
            Ok(WorkerMessage::Shutdown) | Err(_) => return,
        }
    }
}

/// Invalid scheduler configuration or execution state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchedulerError {
    /// Dense switch threshold is outside `0.0..=1.0`.
    InvalidDenseThreshold,
    /// Work ids are not dense and ordered.
    WorkIdsNotDense,
    /// Dirty work id is outside the generation layout.
    WorkOutOfBounds,
    /// Dirty bitset belongs to another schedule layout.
    DirtyLayoutMismatch,
    /// Worker count was zero.
    NoWorkers,
    /// A worker thread could not start.
    WorkerStart(Arc<str>),
    /// A worker or result channel disconnected.
    WorkerDisconnected,
}

impl fmt::Display for SchedulerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDenseThreshold => formatter.write_str("dense threshold must be between zero and one"),
            Self::WorkIdsNotDense => formatter.write_str("work ids must be dense and ordered"),
            Self::WorkOutOfBounds => formatter.write_str("work id is out of bounds"),
            Self::DirtyLayoutMismatch => formatter.write_str("dirty set and schedule layouts differ"),
            Self::NoWorkers => formatter.write_str("runtime scheduler requires at least one worker"),
            Self::WorkerStart(error) => write!(formatter, "failed to start runtime worker: {error}"),
            Self::WorkerDisconnected => formatter.write_str("runtime worker disconnected"),
        }
    }
}

impl std::error::Error for SchedulerError {}
