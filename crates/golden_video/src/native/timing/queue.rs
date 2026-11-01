use serde::{Deserialize, Serialize};

use super::SpoutImageVersion;

/// Verified storage/admission bounds at an accepted native publication. These describe
/// app-owned publication capacity, not exact instantaneous occupancy, GPU time or scanout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputQueueBound {
    /// Actual DXGI descriptor/latency readbacks. The readiness permit was retained before
    /// this Present call; flip-discard delivery does not add an app-side frame FIFO.
    Window {
        /// Total native back buffers, a conservative published-storage ceiling.
        buffer_count: u32,
        /// DXGI's checked maximum queued frames, separate from total buffer storage.
        maximum_frame_latency: u32,
        /// Actual fixed Present interval; zero allows newer queued images to replace stale ones.
        sync_interval: u32,
        /// Actual worker-local permit state immediately before Present.
        admission_permit: bool,
    },
    /// The pinned SDK copies into one shared image. New accepted versions overwrite that
    /// image under its ownership lock; receivers do not accumulate an app-owned FIFO.
    SpoutLatestImage {
        /// The actual shared image/version also attached to the accepted output trace.
        image: SpoutImageVersion,
    },
}
