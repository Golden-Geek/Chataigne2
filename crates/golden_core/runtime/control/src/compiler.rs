use std::collections::BTreeSet;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};

use crate::{ProjectRevision, RuntimeGeneration, RuntimeGenerationId, RuntimeMetrics};

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

/// App/domain compiler plugged into the reusable asynchronous service.
pub trait GenerationCompiler<P>: Send + Sync + 'static {
    /// Compiler failure.
    type Error: fmt::Display + Send + 'static;

    /// Builds one immutable generation without mutating the live semantic runtime.
    fn compile(
        &self,
        generation_id: RuntimeGenerationId,
        request: CompileRequest<P>,
    ) -> Result<RuntimeGeneration, Self::Error>;
}

struct CompileJob<P> {
    ticket: u64,
    generation_id: RuntimeGenerationId,
    request: CompileRequest<P>,
}

enum CompileMessage<P> {
    Compile(CompileJob<P>),
    Shutdown,
}

/// Cloneable admission handle for asynchronous generation compilation.
pub struct CompilationHandle<P> {
    sender: mpsc::Sender<CompileMessage<P>>,
    next_ticket: Arc<AtomicU64>,
    next_generation: Arc<AtomicU64>,
    metrics: Arc<RuntimeMetrics>,
}

impl<P> Clone for CompilationHandle<P> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            next_ticket: self.next_ticket.clone(),
            next_generation: self.next_generation.clone(),
            metrics: self.metrics.clone(),
        }
    }
}

impl<P: Send + Sync + 'static> CompilationHandle<P> {
    /// Admits a lossless compile request and returns its monotonic ticket.
    pub fn request(&self, request: CompileRequest<P>) -> Result<u64, CompilationError> {
        let ticket = self.next_ticket.fetch_add(1, Ordering::Relaxed);
        let generation_id = RuntimeGenerationId(self.next_generation.fetch_add(1, Ordering::Relaxed));
        self.metrics.compilation_requested();
        self.sender
            .send(CompileMessage::Compile(CompileJob {
                ticket,
                generation_id,
                request,
            }))
            .map_err(|_| CompilationError::Disconnected)?;
        Ok(ticket)
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

/// Owner of one compilation thread and its completion queue.
pub struct CompilationService<P, C: GenerationCompiler<P>> {
    handle: CompilationHandle<P>,
    completions: mpsc::Receiver<CompilationCompletion>,
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
        let (request_tx, request_rx) = mpsc::channel();
        let (completion_tx, completion_rx) = mpsc::channel();
        let handle = CompilationHandle {
            sender: request_tx,
            next_ticket: Arc::new(AtomicU64::new(1)),
            next_generation: Arc::new(AtomicU64::new(first_generation_id)),
            metrics: metrics.clone(),
        };
        let thread = thread::Builder::new()
            .name("golden-compiler".to_string())
            .spawn(move || compiler_loop(compiler, request_rx, completion_tx, metrics))?;
        Ok(Self {
            handle,
            completions: completion_rx,
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
        match self.completions.try_recv() {
            Ok(completion) => Ok(Some(completion)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(CompilationError::Disconnected),
        }
    }

    /// Waits for one completion in a host/compiler coordination thread.
    pub fn complete(&self) -> Result<CompilationCompletion, CompilationError> {
        self.completions.recv().map_err(|_| CompilationError::Disconnected)
    }
}

impl<P, C: GenerationCompiler<P>> Drop for CompilationService<P, C> {
    fn drop(&mut self) {
        let _ = self.handle.sender.send(CompileMessage::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn compiler_loop<P, C>(
    compiler: C,
    requests: mpsc::Receiver<CompileMessage<P>>,
    completions: mpsc::Sender<CompilationCompletion>,
    metrics: Arc<RuntimeMetrics>,
) where
    P: Send + Sync + 'static,
    C: GenerationCompiler<P>,
{
    while let Ok(message) = requests.recv() {
        let CompileMessage::Compile(job) = message else { break };
        let revision = job.request.revision;
        let result = compiler
            .compile(job.generation_id, job.request)
            .map(Arc::new)
            .map_err(|error| CompilationError::CompileFailed(Arc::from(error.to_string())));
        metrics.compilation_finished(result.is_ok(), result.as_ref().ok().map(|generation| generation.id.0));
        if completions
            .send(CompilationCompletion {
                ticket: job.ticket,
                revision,
                result,
            })
            .is_err()
        {
            break;
        }
    }
}

/// Compilation-plane failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompilationError {
    /// Compiler or completion channel disconnected.
    Disconnected,
    /// App/domain compiler rejected the project revision.
    CompileFailed(Arc<str>),
}

impl fmt::Display for CompilationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected => formatter.write_str("compilation service is not available"),
            Self::CompileFailed(error) => write!(formatter, "generation compilation failed: {error}"),
        }
    }
}

impl std::error::Error for CompilationError {}
