use std::fmt;
use std::num::NonZeroU64;

macro_rules! identifier {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
        #[serde(transparent)]
        pub struct $name(NonZeroU64);

        impl $name {
            /// Creates an identity; zero is reserved for absence and is never a valid ID.
            pub const fn new(value: u64) -> Option<Self> {
                match NonZeroU64::new(value) {
                    Some(value) => Some(Self(value)),
                    None => None,
                }
            }

            /// Returns the nonzero value without exposing a platform resource handle.
            pub const fn get(self) -> u64 {
                self.0.get()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

identifier!(
    TextureId,
    "Logical texture identity within a render plan; not a native resource or ownership lease."
);
identifier!(
    PassId,
    "Logical pass identity used for stable scheduling and validation diagnostics."
);
identifier!(
    OutputId,
    "Runtime output binding identity; endpoint selection and authored identity remain app-owned."
);

#[cfg(test)]
mod tests;
