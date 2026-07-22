use std::fmt;
use std::sync::Arc;

macro_rules! integer_id {
    ($name:ident, $inner:ty, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub $inner);

        impl $name {
            /// Returns the dense integer representation.
            pub const fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

integer_id!(
    RuntimeGenerationId,
    u64,
    "Monotonic immutable runtime-generation identifier."
);
integer_id!(
    ProjectRevision,
    u64,
    "Authoritative project revision compiled into a generation."
);
integer_id!(ArtifactId, u32, "Dense compiled-artifact identifier.");
integer_id!(KernelId, u32, "Dense shared processor-kernel identifier.");
integer_id!(ProcessorInstanceId, u32, "Dense processor-instance identifier.");
integer_id!(WorkUnitId, u32, "Dense scheduled work-unit identifier.");
integer_id!(InputSlot, u32, "Dense external-input slot.");
integer_id!(StateSlot, u32, "Dense persistent-state slot.");
integer_id!(ValueSlot, u32, "Dense semantic value/output slot.");
integer_id!(EffectSlot, u32, "Dense staged-effect slot.");
integer_id!(LaneIndex, u32, "Dense multiplex lane index.");

/// Stable typed state key used only while swapping runtime generations.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StableStateKey(Arc<str>);

impl StableStateKey {
    /// Creates a stable state key.
    pub fn new(value: impl Into<Arc<str>>) -> Self {
        Self(value.into())
    }

    /// Returns the stable textual key.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StableStateKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
