use std::{fmt, num::NonZeroU64, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

macro_rules! uuid_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
        #[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
        #[cfg_attr(feature = "codegen", ts(export))]
        pub struct $name(Uuid);

        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            #[must_use]
            pub const fn from_uuid(value: Uuid) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self::from_uuid(value)
            }
        }

        impl From<$name> for Uuid {
            fn from(value: $name) -> Self {
                value.as_uuid()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(value).map(Self::from_uuid)
            }
        }
    };
}

macro_rules! string_id {
    ($name:ident, $kind:literal) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
        #[cfg_attr(feature = "codegen", ts(export))]
        pub struct $name(String);

        impl $name {
            pub const MAX_BYTES: usize = 64 * 1024;

            pub fn new(value: impl Into<String>) -> Result<Self, InvalidIdentifier> {
                let value = value.into();
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err(InvalidIdentifier::Empty { kind: $kind });
                }
                if trimmed != value {
                    return Err(InvalidIdentifier::SurroundingWhitespace { kind: $kind });
                }
                if value.len() > Self::MAX_BYTES {
                    return Err(InvalidIdentifier::TooLong {
                        kind: $kind,
                        maximum: Self::MAX_BYTES,
                    });
                }
                Ok(Self(value.into_boxed_str().into_string()))
            }

            #[must_use]
            pub fn from_static(value: &'static str) -> Self {
                Self::new(value).expect("static audio identifier must satisfy its bounds")
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }

            /// Heap storage retained by this immutable identity, excluding its inline value.
            #[must_use]
            pub fn retained_heap_bytes(&self) -> usize {
                self.0.capacity()
            }

            #[must_use]
            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = InvalidIdentifier;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }
    };
}

uuid_id!(AudioChannelId);
uuid_id!(AudioRouteId);
uuid_id!(AnalysisTapId);

string_id!(BackendId, "backend");
string_id!(AudioDeviceId, "audio device");
string_id!(AudioDeviceProfileKey, "audio device profile");
string_id!(PhysicalChannelKey, "physical channel");
string_id!(PlaybackId, "playback");

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
#[cfg_attr(feature = "codegen", ts(type = "number"))]
pub struct ConfigGeneration(u64);

impl ConfigGeneration {
    pub const INITIAL: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl fmt::Display for ConfigGeneration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommandSequence(NonZeroU64);

impl CommandSequence {
    pub const FIRST: Self = Self(NonZeroU64::MIN);

    pub fn new(value: u64) -> Result<Self, InvalidIdentifier> {
        NonZeroU64::new(value).map(Self).ok_or(InvalidIdentifier::ZeroSequence)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match NonZeroU64::new(self.0.get().saturating_add(1)) {
            Some(value) => Self(value),
            None => Self::FIRST,
        }
    }
}

impl fmt::Display for CommandSequence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct VoiceId {
    slot: u16,
    generation: u32,
}

impl VoiceId {
    #[must_use]
    pub const fn new(slot: u16, generation: u32) -> Self {
        Self { slot, generation }
    }

    #[must_use]
    pub const fn slot(self) -> u16 {
        self.slot
    }

    #[must_use]
    pub const fn generation(self) -> u32 {
        self.generation
    }
}

impl fmt::Display for VoiceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.slot, self.generation)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum InvalidIdentifier {
    #[error("{kind} identifier must not be empty")]
    Empty { kind: &'static str },
    #[error("{kind} identifier must not contain surrounding whitespace")]
    SurroundingWhitespace { kind: &'static str },
    #[error("{kind} identifier exceeds {maximum} bytes")]
    TooLong { kind: &'static str, maximum: usize },
    #[error("command sequence must be greater than zero")]
    ZeroSequence,
}
