//! Stable application-facing contracts used while Golden runtime implementations migrate.
//!
//! The traits in this crate describe the seams an application host may depend on. They do not
//! expose an engine, project tree, transport, device API, or app-specific node policy.

#![warn(missing_docs)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Applies project-level transactions and history operations.
pub trait ProjectTransactions {
    /// Transaction accepted by the authoritative project implementation.
    type Transaction;
    /// Receipt returned after applying a transaction or history operation.
    type Receipt;
    /// Transaction failure.
    type Error;

    /// Applies one project transaction atomically.
    fn apply_transaction(&self, transaction: Self::Transaction) -> Result<Self::Receipt, Self::Error>;

    /// Reverts the latest committed transaction.
    fn undo(&self) -> Result<Self::Receipt, Self::Error>;

    /// Reapplies the latest reverted transaction.
    fn redo(&self) -> Result<Self::Receipt, Self::Error>;
}

/// Applies graph-domain edits without exposing implementation-owned graph storage.
pub trait GraphEditing {
    /// Graph edit request.
    type Edit;
    /// Stable graph revision returned by the implementation.
    type Revision;
    /// Graph edit failure.
    type Error;

    /// Applies one graph edit and returns the resulting revision.
    fn apply_graph_edit(&self, edit: Self::Edit) -> Result<Self::Revision, Self::Error>;
}

/// Reads runtime values and publishes timestamped inputs through a typed boundary.
pub trait RuntimeValues {
    /// Stable runtime value key.
    type Key;
    /// Canonical value representation used by this adapter.
    type Value;
    /// Runtime value failure.
    type Error;

    /// Reads the latest committed value for `key`.
    fn read_value(&self, key: &Self::Key) -> Result<Option<Self::Value>, Self::Error>;

    /// Publishes one external input value with its source timestamp.
    fn publish_input(&self, key: Self::Key, value: Self::Value, source_time_ns: u64) -> Result<(), Self::Error>;
}

/// Provides immutable snapshots and bounded observation deltas.
pub trait Observation {
    /// Snapshot selection request.
    type SnapshotRequest;
    /// Immutable snapshot returned to a caller.
    type Snapshot;
    /// Delta/replay selection request.
    type DeltaRequest;
    /// Bounded delta returned to a caller.
    type Delta;
    /// Observation failure.
    type Error;

    /// Returns one immutable snapshot.
    fn snapshot(&self, request: Self::SnapshotRequest) -> Result<Self::Snapshot, Self::Error>;

    /// Returns bounded changes selected by `request`.
    fn changes(&self, request: Self::DeltaRequest) -> Result<Self::Delta, Self::Error>;
}

/// Loads and saves project documents without owning desktop or browser workflow.
pub trait Persistence {
    /// Load request such as bytes plus a migration policy.
    type LoadRequest;
    /// Loaded document or replacement receipt.
    type LoadResult;
    /// Save request such as a document selection.
    type SaveRequest;
    /// Encoded document or persistence receipt.
    type SaveResult;
    /// Persistence failure.
    type Error;

    /// Loads a project document through the authoritative codec.
    fn load(&self, request: Self::LoadRequest) -> Result<Self::LoadResult, Self::Error>;

    /// Saves a project document through the authoritative codec.
    fn save(&self, request: Self::SaveRequest) -> Result<Self::SaveResult, Self::Error>;
}

/// Starts and stops an application runtime without coupling callers to a desktop host.
pub trait HostLifecycle {
    /// Startup request.
    type StartRequest;
    /// Startup result.
    type StartResult;
    /// Shutdown request.
    type StopRequest;
    /// Shutdown result.
    type StopResult;
    /// Lifecycle failure.
    type Error;

    /// Starts or prepares the runtime.
    fn start(&self, request: Self::StartRequest) -> Result<Self::StartResult, Self::Error>;

    /// Stops the runtime and releases implementation-owned resources.
    fn stop(&self, request: Self::StopRequest) -> Result<Self::StopResult, Self::Error>;
}

/// Capability required to construct an output accepted by an authoritative I/O dispatcher.
///
/// A shadow evaluator is never given this authority. Shadow APIs in this crate accept only pure
/// evaluators, so they cannot dispatch an output through [`ModuleIo`] by construction.
#[derive(Clone, Debug)]
pub struct EffectAuthority {
    _private: Arc<()>,
}

impl EffectAuthority {
    fn new() -> Self {
        Self { _private: Arc::new(()) }
    }

    /// Wraps one output with the authority required by [`ModuleIo::dispatch_output`].
    pub fn authorize<T>(&self, value: T) -> AuthoritativeOutput<T> {
        AuthoritativeOutput { value }
    }
}

/// An output explicitly authorized for external dispatch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoritativeOutput<T> {
    value: T,
}

impl<T> AuthoritativeOutput<T> {
    /// Returns the authorized output payload.
    pub fn into_inner(self) -> T {
        self.value
    }

    /// Borrows the authorized output payload.
    pub fn value(&self) -> &T {
        &self.value
    }
}

/// Injectable module/device boundary with a single authoritative output path.
pub trait ModuleIo {
    /// Parsed input frame.
    type Input;
    /// Output command.
    type Output;
    /// I/O failure.
    type Error;

    /// Receives the next input frame, if one is available.
    fn next_input(&self) -> Result<Option<Self::Input>, Self::Error>;

    /// Dispatches one externally visible output command.
    fn dispatch_output(&self, output: AuthoritativeOutput<Self::Output>) -> Result<(), Self::Error>;
}

/// Complete set of independently replaceable application-facing facades.
#[derive(Clone, Debug)]
pub struct ApplicationFacades<P, G, V, O, M, S, H> {
    project_transactions: P,
    graph_editing: G,
    runtime_values: V,
    observation: O,
    module_io: M,
    persistence: S,
    host_lifecycle: H,
    effect_authority: EffectAuthority,
}

impl<P, G, V, O, M, S, H> ApplicationFacades<P, G, V, O, M, S, H> {
    /// Composes a complete application boundary from independently replaceable implementations.
    pub fn new(
        project_transactions: P,
        graph_editing: G,
        runtime_values: V,
        observation: O,
        module_io: M,
        persistence: S,
        host_lifecycle: H,
    ) -> Self {
        Self {
            project_transactions,
            graph_editing,
            runtime_values,
            observation,
            module_io,
            persistence,
            host_lifecycle,
            effect_authority: EffectAuthority::new(),
        }
    }

    /// Returns the project-transaction facade.
    pub fn project_transactions(&self) -> &P {
        &self.project_transactions
    }

    /// Returns the graph-editing facade.
    pub fn graph_editing(&self) -> &G {
        &self.graph_editing
    }

    /// Returns the runtime-value facade.
    pub fn runtime_values(&self) -> &V {
        &self.runtime_values
    }

    /// Returns the observation facade.
    pub fn observation(&self) -> &O {
        &self.observation
    }

    /// Returns the module-I/O facade.
    pub fn module_io(&self) -> &M {
        &self.module_io
    }

    /// Returns the persistence facade.
    pub fn persistence(&self) -> &S {
        &self.persistence
    }

    /// Returns the host-lifecycle facade.
    pub fn host_lifecycle(&self) -> &H {
        &self.host_lifecycle
    }

    /// Authorizes one command for the bundle's single external effect path.
    pub fn authorize_output<T>(&self, value: T) -> AuthoritativeOutput<T> {
        self.effect_authority.authorize(value)
    }
}

/// Source of monotonic timestamps for deterministic I/O capture.
pub trait IoClock {
    /// Returns the next timestamp in nanoseconds.
    fn now_ns(&self) -> u64;
}

/// Deterministic clock that advances by a fixed amount on every read.
#[derive(Debug)]
pub struct DeterministicIoClock {
    next_ns: AtomicU64,
    step_ns: u64,
}

impl DeterministicIoClock {
    /// Creates a clock whose first timestamp is `start_ns`.
    pub fn new(start_ns: u64, step_ns: u64) -> Self {
        Self {
            next_ns: AtomicU64::new(start_ns),
            step_ns,
        }
    }
}

impl IoClock for DeterministicIoClock {
    fn now_ns(&self) -> u64 {
        self.next_ns.fetch_add(self.step_ns, Ordering::Relaxed)
    }
}

/// One deterministically ordered input recording.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedInput<I> {
    /// Monotonic sequence across both input and output records.
    pub sequence: u64,
    /// Timestamp supplied by the injected clock.
    pub timestamp_ns: u64,
    /// Parsed input payload.
    pub payload: I,
}

/// One deterministically ordered output recording.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedOutput<O> {
    /// Monotonic sequence across both input and output records.
    pub sequence: u64,
    /// Timestamp supplied by the injected clock.
    pub timestamp_ns: u64,
    /// Dispatched output payload.
    pub payload: O,
}

/// Versioned deterministic protocol or hardware recording.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IoRecording<I, O> {
    /// Recording schema version.
    pub schema_version: u32,
    /// Captured input frames.
    pub inputs: Vec<RecordedInput<I>>,
    /// Captured authoritative output commands.
    pub outputs: Vec<RecordedOutput<O>>,
}

impl<I, O> Default for IoRecording<I, O> {
    fn default() -> Self {
        Self {
            schema_version: 1,
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }
}

/// I/O decorator that captures deterministic recordings without changing the wrapped boundary.
#[derive(Debug)]
pub struct RecordingModuleIo<B, C, I, O> {
    inner: B,
    clock: C,
    next_sequence: AtomicU64,
    recording: Mutex<IoRecording<I, O>>,
}

impl<B, C, I, O> RecordingModuleIo<B, C, I, O> {
    /// Wraps an I/O boundary with an injected clock.
    pub fn new(inner: B, clock: C) -> Self {
        Self {
            inner,
            clock,
            next_sequence: AtomicU64::new(0),
            recording: Mutex::new(IoRecording::default()),
        }
    }

    /// Returns a stable snapshot of all captured records.
    pub fn recording(&self) -> IoRecording<I, O>
    where
        I: Clone,
        O: Clone,
    {
        self.recording
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

impl<B, C, I, O> ModuleIo for RecordingModuleIo<B, C, I, O>
where
    B: ModuleIo<Input = I, Output = O>,
    C: IoClock,
    I: Clone,
    O: Clone,
{
    type Input = I;
    type Output = O;
    type Error = B::Error;

    fn next_input(&self) -> Result<Option<Self::Input>, Self::Error> {
        let input = self.inner.next_input()?;
        if let Some(payload) = input.as_ref() {
            let record = RecordedInput {
                sequence: self.next_sequence.fetch_add(1, Ordering::Relaxed),
                timestamp_ns: self.clock.now_ns(),
                payload: payload.clone(),
            };
            self.recording
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .inputs
                .push(record);
        }
        Ok(input)
    }

    fn dispatch_output(&self, output: AuthoritativeOutput<Self::Output>) -> Result<(), Self::Error> {
        let record = RecordedOutput {
            sequence: self.next_sequence.fetch_add(1, Ordering::Relaxed),
            timestamp_ns: self.clock.now_ns(),
            payload: output.value().clone(),
        };
        self.inner.dispatch_output(output)?;
        self.recording
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .outputs
            .push(record);
        Ok(())
    }
}

/// Stable digest of deterministic semantic output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SemanticDigest(pub [u8; 32]);

impl SemanticDigest {
    /// Hashes an already canonical byte representation.
    pub fn from_canonical_bytes(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    /// Returns a lowercase hexadecimal digest.
    pub fn to_hex(self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

/// Converts one semantic output into a stable digest.
pub trait SemanticDigester<T> {
    /// Digest construction failure.
    type Error;

    /// Digests `value` after semantic normalization.
    fn digest(&self, value: &T) -> Result<SemanticDigest, Self::Error>;
}

/// Digests values through deterministic `serde_json` encoding.
#[derive(Clone, Copy, Debug, Default)]
pub struct JsonSemanticDigester;

impl<T> SemanticDigester<T> for JsonSemanticDigester
where
    T: Serialize,
{
    type Error = serde_json::Error;

    fn digest(&self, value: &T) -> Result<SemanticDigest, Self::Error> {
        serde_json::to_vec(value).map(|bytes| SemanticDigest::from_canonical_bytes(&bytes))
    }
}

/// Pure deterministic computation eligible for shadow execution.
pub trait PureEvaluator<I> {
    /// Semantic output.
    type Output;
    /// Evaluation failure.
    type Error;

    /// Evaluates one immutable input without an effect dispatcher.
    fn evaluate(&self, input: &I) -> Result<Self::Output, Self::Error>;
}

impl<I, O, E, F> PureEvaluator<I> for F
where
    F: Fn(&I) -> Result<O, E>,
{
    type Output = O;
    type Error = E;

    fn evaluate(&self, input: &I) -> Result<Self::Output, Self::Error> {
        self(input)
    }
}

/// Outcome of comparing authoritative and shadow semantic digests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadowStatus {
    /// No shadow implementation was selected.
    Disabled,
    /// Both semantic digests were identical.
    Match,
    /// Semantic digests differed.
    Mismatch,
    /// The shadow implementation failed.
    ShadowFailed,
    /// One of the semantic outputs could not be digested.
    DigestFailed,
}

/// One side-effect-free shadow comparison record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShadowComparison {
    /// Comparison outcome.
    pub status: ShadowStatus,
    /// Digest of the authoritative semantic output, when available.
    pub authoritative_digest: Option<SemanticDigest>,
    /// Digest of the shadow semantic output, when available.
    pub shadow_digest: Option<SemanticDigest>,
    /// Diagnostic for shadow or digest failure.
    pub diagnostic: Option<String>,
}

/// Authoritative output plus its observational shadow comparison.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShadowRun<O> {
    /// Output from the only authoritative computation.
    pub output: O,
    /// Observational comparison that cannot alter the authoritative output.
    pub comparison: ShadowComparison,
}

/// Runs an authoritative pure computation and optionally compares a shadow implementation.
#[derive(Clone, Debug)]
pub struct ShadowExecutor<A, S, D> {
    authoritative: A,
    shadow: Option<S>,
    digester: D,
}

impl<A, S, D> ShadowExecutor<A, S, D> {
    /// Creates a shadow executor. `None` keeps only the authoritative path active.
    pub fn new(authoritative: A, shadow: Option<S>, digester: D) -> Self {
        Self {
            authoritative,
            shadow,
            digester,
        }
    }

    /// Evaluates the authoritative implementation once and compares a pure shadow result.
    pub fn run<I, O, AE, SE, DE>(&self, input: &I) -> Result<ShadowRun<O>, AE>
    where
        A: PureEvaluator<I, Output = O, Error = AE>,
        S: PureEvaluator<I, Output = O, Error = SE>,
        D: SemanticDigester<O, Error = DE>,
        SE: std::fmt::Display,
        DE: std::fmt::Display,
    {
        let output = self.authoritative.evaluate(input)?;
        let Some(shadow) = self.shadow.as_ref() else {
            return Ok(ShadowRun {
                output,
                comparison: ShadowComparison {
                    status: ShadowStatus::Disabled,
                    authoritative_digest: None,
                    shadow_digest: None,
                    diagnostic: None,
                },
            });
        };

        let shadow_output = match shadow.evaluate(input) {
            Ok(output) => output,
            Err(error) => {
                return Ok(ShadowRun {
                    output,
                    comparison: ShadowComparison {
                        status: ShadowStatus::ShadowFailed,
                        authoritative_digest: None,
                        shadow_digest: None,
                        diagnostic: Some(error.to_string()),
                    },
                });
            }
        };

        let authoritative_digest = match self.digester.digest(&output) {
            Ok(digest) => digest,
            Err(error) => {
                return Ok(ShadowRun {
                    output,
                    comparison: ShadowComparison {
                        status: ShadowStatus::DigestFailed,
                        authoritative_digest: None,
                        shadow_digest: None,
                        diagnostic: Some(error.to_string()),
                    },
                });
            }
        };
        let shadow_digest = match self.digester.digest(&shadow_output) {
            Ok(digest) => digest,
            Err(error) => {
                return Ok(ShadowRun {
                    output,
                    comparison: ShadowComparison {
                        status: ShadowStatus::DigestFailed,
                        authoritative_digest: Some(authoritative_digest),
                        shadow_digest: None,
                        diagnostic: Some(error.to_string()),
                    },
                });
            }
        };

        Ok(ShadowRun {
            output,
            comparison: ShadowComparison {
                status: if authoritative_digest == shadow_digest {
                    ShadowStatus::Match
                } else {
                    ShadowStatus::Mismatch
                },
                authoritative_digest: Some(authoritative_digest),
                shadow_digest: Some(shadow_digest),
                diagnostic: None,
            },
        })
    }
}

#[cfg(test)]
mod tests;
