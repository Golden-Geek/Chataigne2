use uuid::Uuid;

/// One runtime's project lifetime. Equal counters from another runtime are never equal epochs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProjectEpoch {
    runtime: Uuid,
    generation: u64,
}

impl ProjectEpoch {
    pub(crate) const fn new(runtime: Uuid, generation: u64) -> Self {
        Self { runtime, generation }
    }

    /// Generation within the owning runtime; identity also includes its runtime lifetime.
    pub const fn generation(self) -> u64 {
        self.generation
    }
}

/// One native device lifetime, fencing completion values that may restart after recreation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DeviceGeneration {
    runtime: Uuid,
    generation: u64,
}

impl DeviceGeneration {
    pub(crate) const fn new(runtime: Uuid, generation: u64) -> Self {
        Self { runtime, generation }
    }

    pub(crate) fn next(self) -> Option<Self> {
        self.generation
            .checked_add(1)
            .map(|generation| Self { generation, ..self })
    }

    /// Generation within the owning runtime; foreign runtime lifetimes remain distinct.
    pub const fn generation(self) -> u64 {
        self.generation
    }
}

/// Monotonic desired-plan revision assigned at publication admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlanRevision(u64);

impl PlanRevision {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Assigned revision, never an array index or native pointer.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Authored plan publication, independent of its GPU resource generations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlanIdentity {
    /// Project lifetime captured before preparation.
    pub project: ProjectEpoch,
    /// Desired-plan publication revision.
    pub revision: PlanRevision,
}

/// One successfully prepared resource set, separate from channel and texture identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GenerationId(u64);

impl GenerationId {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Monotonic resource generation within its runtime.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A completion serial interpreted only within its exact native device lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompletionToken {
    /// Device that owns the submission/fence.
    pub device: DeviceGeneration,
    /// Device-local serial; zero means no submitted use.
    pub serial: u64,
}
