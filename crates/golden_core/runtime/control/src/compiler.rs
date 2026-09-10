use std::collections::BTreeSet;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::{self, JoinHandle};

use crate::{ProjectRevision, RuntimeGeneration, RuntimeGenerationId, RuntimeMetrics};

const COMPLETION_CAPACITY: usize = 2;

/// Precise domain keys affected by one authoritative project transaction.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuntimeChangeSet {
    affected: BTreeSet<Arc<str>>,
}

impl RuntimeChangeSet {
    /// Creates an empty change set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks a domain key affected.
    pub fn mark(&mut self, key: impl Into<Arc<str>>) {
        self.affected.insert(key.into());
    }

    /// Returns whether a domain key requires recompilation.
    pub fn affects(&self, key: &str) -> bool {
        self.affected.contains(key)
    }

    /// Iterates affected keys in deterministic order.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.affected.iter().map(AsRef::as_ref)
    }
}

/// Immutable compile request. The previous generation remains runnable while it executes.
pub struct CompileRequest<P> {
    /// Immutable authored project snapshot.
    pub project: Arc<P>,
    /// Source project revision.
    pub revision: ProjectRevision,
    /// Precise affected-domain set.
    pub changes: RuntimeChangeSet,
    /// Previous valid generation available for incremental reuse.
    pub previous: Option<Arc<RuntimeGeneration>>,
}

/// Cooperative staleness token for one compilation generation.
#[derive(Clone)]
pub struct CompilationContext {
    ticket: u64,
    latest_ticket: Arc<AtomicU64>,
    stopping: Arc<AtomicBool>,
}

impl CompilationContext {
    /// Returns the ticket of the compile currently executing.
    pub const fn ticket(&self) -> u64 {
        self.ticket
    }

    /// Returns whether a newer request or service shutdown superseded this compile.
    pub fn is_stale(&self) -> bool {
        self.stopping.load(Ordering::Acquire) || self.latest_ticket.load(Ordering::Acquire) != self.ticket
    }
}

/// App/domain compiler plugged into the reusable asynchronous service.
pub trait GenerationCompiler<P>: Send + Sync + 'static {
    /// Compiler failure.
    type Error: fmt::Display + Send + 'static;

    /// Builds one immutable generation without mutating the live semantic runtime.
    ///
    /// Long-running implementations must check `context.is_stale()` between materialization
    /// stages and return promptly when it becomes true.
    fn compile(
        &self,
        generation_id: RuntimeGenerationId,
        request: CompileRequest<P>,
        context: &CompilationContext,
    ) -> Result<RuntimeGeneration, Self::Error>;
}

struct CompileJob<P> {
    ticket: u64,
    generation_id: RuntimeGenerationId,
    request: CompileRequest<P>,
}

struct RequestState<P> {
    pending: Option<CompileJob<P>>,
    stopping: bool,
}

struct RequestSlot<P> {
    state: Mutex<RequestState<P>>,
    ready: Condvar,
}

/// Result of admitting a replaceable compilation request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompilationAdmission {
    /// Monotonic ticket assigned to the accepted latest request.
    pub ticket: u64,
    /// Pending request replaced before compilation began, if any.
    pub superseded_ticket: Option<u64>,
}

/// Cloneable admission handle for asynchronous generation compilation.
pub struct CompilationHandle<P> {
    requests: Arc<RequestSlot<P>>,
    next_ticket: Arc<AtomicU64>,
    next_generation: Arc<AtomicU64>,
    latest_ticket: Arc<AtomicU64>,
    stopping: Arc<AtomicBool>,
    metrics: Arc<RuntimeMetrics>,
}

impl<P> Clone for CompilationHandle<P> {
    fn clone(&self) -> Self {
        Self {
            requests: self.requests.clone(),
            next_ticket: self.next_ticket.clone(),
            next_generation: self.next_generation.clone(),
            latest_ticket: self.latest_ticket.clone(),
            stopping: self.stopping.clone(),
            metrics: self.metrics.clone(),
        }
    }
}

impl<P: Send + Sync + 'static> CompilationHandle<P> {
    /// Replaces any not-yet-started request and returns the admitted ticket pair.
    pub fn request(&self, request: CompileRequest<P>) -> Result<CompilationAdmission, CompilationError> {
        if self.stopping.load(Ordering::Acquire) {
            return Err(CompilationError::Disconnected);
        }
        let ticket = self.next_ticket.fetch_add(1, Ordering::Relaxed);
        let generation_id = RuntimeGenerationId(self.next_generation.fetch_add(1, Ordering::Relaxed));
        let job = CompileJob {
            ticket,
            generation_id,
            request,
        };
        let mut state = self
            .requests
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.stopping {
            return Err(CompilationError::Disconnected);
        }
        self.latest_ticket.store(ticket, Ordering::Release);
        let superseded_ticket = state.pending.replace(job).map(|pending| pending.ticket);
        self.metrics.compilation_requested(superseded_ticket.is_some());
        drop(state);
        self.requests.ready.notify_one();
        Ok(CompilationAdmission {
            ticket,
            superseded_ticket,
        })
    }
}

/// Asynchronous compiler completion; only successful generations are eligible for semantic swap.
#[derive(Debug)]
pub struct CompilationCompletion {
    /// Compile request ticket.
    pub ticket: u64,
    /// Source revision requested.
    pub revision: ProjectRevision,
    /// New immutable generation, or a diagnostic while the previous generation remains valid.
    pub result: Result<Arc<RuntimeGeneration>, CompilationError>,
}

/// Owner of one compiler thread, one replaceable pending slot, and a bounded completion queue.
pub struct CompilationService<P, C: GenerationCompiler<P>> {
    handle: CompilationHandle<P>,
    completions: Option<mpsc::Receiver<CompilationCompletion>>,
    thread: Option<JoinHandle<()>>,
    _compiler: std::marker::PhantomData<C>,
}

impl<P, C> CompilationService<P, C>
where
    P: Send + Sync + 'static,
    C: GenerationCompiler<P>,
{
    /// Starts the compilation plane with a dedicated worker.
    pub fn spawn(compiler: C, first_generation_id: u64, metrics: Arc<RuntimeMetrics>) -> std::io::Result<Self> {
        let requests = Arc::new(RequestSlot {
            state: Mutex::new(RequestState {
                pending: None,
                stopping: false,
            }),
            ready: Condvar::new(),
        });
        let (completion_tx, completion_rx) = mpsc::sync_channel(COMPLETION_CAPACITY);
        let stopping = Arc::new(AtomicBool::new(false));
        let latest_ticket = Arc::new(AtomicU64::new(0));
        let handle = CompilationHandle {
            requests: requests.clone(),
            next_ticket: Arc::new(AtomicU64::new(1)),
            next_generation: Arc::new(AtomicU64::new(first_generation_id)),
            latest_ticket: latest_ticket.clone(),
            stopping: stopping.clone(),
            metrics: metrics.clone(),
        };
        let thread = thread::Builder::new()
            .name("golden-compiler".to_string())
            .spawn(move || compiler_loop(compiler, requests, completion_tx, latest_ticket, stopping, metrics))?;
        Ok(Self {
            handle,
            completions: Some(completion_rx),
            thread: Some(thread),
            _compiler: std::marker::PhantomData,
        })
    }

    /// Returns a cloneable compile admission handle.
    pub fn handle(&self) -> CompilationHandle<P> {
        self.handle.clone()
    }

    /// Polls one completion without blocking the control or semantic planes.
    pub fn try_complete(&self) -> Result<Option<CompilationCompletion>, CompilationError> {
        let Some(completions) = self.completions.as_ref() else {
            return Err(CompilationError::Disconnected);
        };
        match completions.try_recv() {
            Ok(completion) => Ok(Some(completion)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(CompilationError::Disconnected),
        }
    }

    /// Waits for one completion in a host/compiler coordination thread.
    pub fn complete(&self) -> Result<CompilationCompletion, CompilationError> {
        self.completions
            .as_ref()
            .ok_or(CompilationError::Disconnected)?
            .recv()
            .map_err(|_| CompilationError::Disconnected)
    }
}

impl<P, C: GenerationCompiler<P>> Drop for CompilationService<P, C> {
    fn drop(&mut self) {
        self.handle.stopping.store(true, Ordering::Release);
        let mut state = self
            .handle
            .requests
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.stopping = true;
        let discarded_pending = state.pending.take().is_some();
        self.handle.metrics.compilation_discard_pending(discarded_pending);
        drop(state);
        self.handle.requests.ready.notify_all();
        self.completions.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn compiler_loop<P, C>(
    compiler: C,
    requests: Arc<RequestSlot<P>>,
    completions: mpsc::SyncSender<CompilationCompletion>,
    latest_ticket: Arc<AtomicU64>,
    stopping: Arc<AtomicBool>,
    metrics: Arc<RuntimeMetrics>,
) where
    P: Send + Sync + 'static,
    C: GenerationCompiler<P>,
{
    loop {
        let job = {
            let mut state = requests.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            while state.pending.is_none() && !state.stopping {
                state = requests
                    .ready
                    .wait(state)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
            if state.stopping {
                return;
            }
            state.pending.take().expect("pending compile exists after wait")
        };
        metrics.compilation_started();
        let revision = job.request.revision;
        let context = CompilationContext {
            ticket: job.ticket,
            latest_ticket: latest_ticket.clone(),
            stopping: stopping.clone(),
        };
        let result = if context.is_stale() {
            Err(CompilationError::Superseded {
                by_ticket: latest_ticket.load(Ordering::Acquire),
            })
        } else {
            match compiler.compile(job.generation_id, job.request, &context) {
                _ if context.is_stale() => Err(CompilationError::Superseded {
                    by_ticket: latest_ticket.load(Ordering::Acquire),
                }),
                Ok(generation) => Ok(Arc::new(generation)),
                Err(error) => Err(CompilationError::CompileFailed(Arc::from(error.to_string()))),
            }
        };
        match &result {
            Ok(generation) => metrics.compilation_finished(true, Some(generation.id.0)),
            Err(CompilationError::Superseded { .. }) => metrics.compilation_finished_superseded(),
            Err(_) => metrics.compilation_finished(false, None),
        }
        if completions
            .send(CompilationCompletion {
                ticket: job.ticket,
                revision,
                result,
            })
            .is_err()
        {
            return;
        }
    }
}

/// Compilation-plane failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompilationError {
    /// Compiler or completion channel disconnected.
    Disconnected,
    /// A newer replaceable request made this generation ineligible for publication.
    Superseded {
        /// Newest ticket observed by the compiler.
        by_ticket: u64,
    },
    /// App/domain compiler rejected the project revision.
    CompileFailed(Arc<str>),
}

impl fmt::Display for CompilationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected => formatter.write_str("compilation service is not available"),
            Self::Superseded { by_ticket } => write!(formatter, "compilation was superseded by ticket {by_ticket}"),
            Self::CompileFailed(error) => write!(formatter, "generation compilation failed: {error}"),
        }
    }
}

impl std::error::Error for CompilationError {}
