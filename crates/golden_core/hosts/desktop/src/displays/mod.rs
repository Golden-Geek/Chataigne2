//! Bounded display enumeration on a host/preparation worker, never the engine or WebView thread.
use crate::NativeOutputBounds;
use std::io;

/// An active desktop target with an OS device-interface identity rather than an enumeration index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeDisplay {
    /// Opaque OS monitor identity. X11 combines its connector with available monitor evidence.
    pub id: String,
    /// Human-readable monitor label; never used to resolve identity.
    pub name: String,
    /// Canonical lowercase hexadecimal of a validated 128-byte base EDID block, when available.
    /// This preserves manufacturer, product and supplied serial data without inventing uniqueness.
    pub edid: Option<String>,
    /// Display order exposed by the host, or zero when unavailable.
    /// This is routing metadata and can change when the desktop topology changes.
    pub index: u32,
    /// OS primary-desktop flag, falling back to a physical origin of (0, 0) when unavailable.
    pub primary: bool,
    /// Physical connection classification. None means unknown or virtual; primary is unrelated.
    pub internal: Option<bool>,
    /// OS desktop refresh frequency as numerator/denominator Hz; None when unreported.
    pub refresh_rate: Option<(u32, u32)>,
    /// Current physical desktop client rectangle, independent of caller DPI awareness.
    pub bounds: NativeOutputBounds,
}

/// Enumerate at most 32 active desktop targets, sorted by device path. A topology race retries at
/// most three times; capacity/query/identity errors are explicit, never a primary-display fallback.
/// Paths can change after OS device reinstallation; unmatched persisted paths remain missing.
/// Optional EDID/index metadata never removes an otherwise valid active target from the catalog.
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

#[cfg(target_os = "macos")]
pub(crate) mod macos;
#[cfg(target_os = "macos")]
pub use macos::macos_desktop_displays;

#[cfg(target_os = "linux")]
pub(crate) mod wayland;
#[cfg(target_os = "linux")]
pub(crate) mod x11;

#[cfg(any(windows, target_os = "linux", test))]
mod metadata;
#[cfg(windows)]
mod monitor_registry;
#[cfg(windows)]
mod platform;
#[cfg(test)]
mod tests;
