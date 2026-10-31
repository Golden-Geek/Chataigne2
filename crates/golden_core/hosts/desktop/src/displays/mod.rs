//! Bounded display enumeration on a host/preparation worker, never the engine or WebView thread.
use crate::NativeOutputBounds;
use std::io;

/// An active desktop target with an OS device-interface identity rather than an enumeration index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeDisplay {
    /// Opaque Windows monitor device path. Persist as an exact intended identity, not a position.
    pub id: String,
    /// Human-readable monitor label; never used to resolve identity.
    pub name: String,
    /// Current physical desktop client rectangle, independent of caller DPI awareness.
    pub bounds: NativeOutputBounds,
}

/// Enumerate at most 32 active desktop targets, sorted by device path. A topology race retries at
/// most three times; capacity/query/identity errors are explicit, never a primary-display fallback.
/// Paths can change after OS device reinstallation; unmatched persisted paths remain missing.
pub fn native_displays() -> io::Result<Vec<NativeDisplay>> {
    #[cfg(windows)]
    {
        platform::enumerate()
    }
    #[cfg(not(windows))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "native display catalog requires Windows",
        ))
    }
}

#[cfg(windows)]
mod platform;
