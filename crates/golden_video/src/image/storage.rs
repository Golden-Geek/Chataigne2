//! Payload admission shared by decoded planes, conversion scratch and immutable images.
use super::ImageError;
use std::sync::{Arc, Mutex};

/// CPU image payload admission, including idle buffers, retained readers and in-flight
/// allocation reservations. At settled observation points this equals owned payload.
/// Excludes allocator overhead, codec/scaler-private state and compressed packets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ImageStorageAllocation {
    /// Maximum simultaneous admitted payload.
    pub limit_bytes: u64,
    /// Current payload reservations, including buffers idle in a codec pool.
    pub retained_bytes: u64,
    /// Lifetime maximum admitted payload, including in-flight reservations.
    pub peak_bytes: u64,
    /// Number of payload allocations, not reader references.
    pub allocations: u64,
}

/// Read-only lifetime observer; retaining it does not retain any pixel buffer or pool.
#[derive(Clone, Debug)]
pub struct ImageStorageBudget(Arc<Mutex<ImageStorageAllocation>>);

#[derive(Debug)]
pub(crate) struct StorageLease {
    budget: ImageStorageBudget,
    bytes: u64,
}
impl StorageLease {
    #[cfg(feature = "ffmpeg")]
    pub(crate) fn bytes(&self) -> usize {
        // Reservations originate from a representable usize allocation request.
        self.bytes as usize
    }
}
impl ImageStorageBudget {
    /// Create a positive payload ceiling; cloning an observer adds no storage or thread.
    pub fn new(bytes: u64) -> Result<Self, ImageError> {
        if bytes == 0 {
            return Err(ImageError::StorageLimit);
        }
        Ok(Self(Arc::new(Mutex::new(ImageStorageAllocation {
            limit_bytes: bytes,
            retained_bytes: 0,
            peak_bytes: 0,
            allocations: 0,
        }))))
    }
    /// One coherent inventory; zero after decoder/converter/frame drop proves payload retirement.
    pub fn snapshot(&self) -> ImageStorageAllocation {
        *self.0.lock().unwrap_or_else(|error| error.into_inner())
    }
    #[cfg(feature = "ffmpeg")]
    pub(crate) fn tighten(&self, bytes: u64) -> Result<(), ImageError> {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if bytes == 0 || bytes > state.limit_bytes {
            return Err(ImageError::StorageLimit);
        }
        if bytes < state.retained_bytes {
            return Err(ImageError::Capacity);
        }
        state.limit_bytes = bytes;
        Ok(())
    }
    #[cfg(feature = "ffmpeg")]
    pub(crate) fn reserve(&self, bytes: usize) -> Option<StorageLease> {
        let bytes = u64::try_from(bytes).ok().filter(|bytes| *bytes > 0)?;
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        let retained = state.retained_bytes.checked_add(bytes)?;
        if retained > state.limit_bytes {
            return None;
        }
        state.retained_bytes = retained;
        state.peak_bytes = state.peak_bytes.max(retained);
        state.allocations += 1;
        Some(StorageLease {
            budget: self.clone(),
            bytes,
        })
    }
}
impl Drop for StorageLease {
    fn drop(&mut self) {
        let mut state = self.budget.0.lock().unwrap_or_else(|error| error.into_inner());
        state.retained_bytes -= self.bytes;
        state.allocations -= 1;
    }
}
