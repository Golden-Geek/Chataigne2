//! Metadata for actual native owners. Texel payload is not driver allocation size/residency.

use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::runtime::DeviceGeneration;
use crate::{OutputId, TextureDescriptor};

/// Completion-retained plan texture objects; shared Rc owners and views are charged once.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct OwnedTextureSnapshot {
    /// Successful complete texture/view constructions tracked in this control lifetime.
    pub created: u64,
    /// Objects whose native texture/views have been released.
    pub released: u64,
    /// Current owned texture objects.
    pub live: u64,
    /// Highest simultaneously owned object count.
    pub peak_live: u64,
    /// Sum of validated descriptor payloads; excludes driver alignment/overhead.
    pub texel_bytes: u64,
    /// Highest simultaneously owned descriptor-payload total.
    pub peak_texel_bytes: u64,
}

#[derive(Clone, Default)]
pub(super) struct TextureAllocations(Arc<Mutex<OwnedTextureSnapshot>>);

pub(super) struct TextureAllocation {
    owner: TextureAllocations,
    bytes: u64,
}

impl TextureAllocations {
    pub fn snapshot(&self) -> OwnedTextureSnapshot {
        *self.0.lock().unwrap_or_else(|error| error.into_inner())
    }

    pub fn register(&self, bytes: u64) -> TextureAllocation {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        state.created = state.created.saturating_add(1);
        state.live = state
            .live
            .checked_add(1)
            .expect("native object count is physically bounded");
        state.texel_bytes = state
            .texel_bytes
            .checked_add(bytes)
            .expect("native storage was admitted before allocation");
        state.peak_live = state.peak_live.max(state.live);
        state.peak_texel_bytes = state.peak_texel_bytes.max(state.texel_bytes);
        TextureAllocation {
            owner: self.clone(),
            bytes,
        }
    }
}

impl Drop for TextureAllocation {
    fn drop(&mut self) {
        let mut state = self.owner.0.lock().unwrap_or_else(|error| error.into_inner());
        state.live -= 1;
        state.texel_bytes -= self.bytes;
        state.released = state.released.saturating_add(1);
    }
}

/// Worker-published source-pool owners and renderer aliases. Aliases add no owned storage.
#[derive(Clone, Debug, Serialize)]
pub struct SourceTextureSnapshot {
    /// Exact source registration, distinct from a reused slot index or shared handle.
    pub binding: uuid::Uuid,
    /// Owning renderer device lifetime.
    pub device: DeviceGeneration,
    /// Current source bridge-pool incarnation.
    pub pool_generation: u64,
    /// Actual committed owned bridge objects.
    pub bridge_slots: usize,
    /// Actual pool descriptor; absent with no committed pool.
    pub descriptor: Option<TextureDescriptor>,
    /// Validated bridge payload; absent if metadata is incomplete.
    pub owned_texel_bytes: Option<u64>,
    /// Renderer COM import aliases, not new texture allocations.
    pub import_aliases: usize,
    /// Completion-retained readers; these do not add texture storage.
    pub reader_leases: usize,
}

/// Storage retained by a live or retiring native endpoint, separate from plan textures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum OutputTextureAllocation {
    /// Actual checked DXGI backing layout; includes every buffer once.
    Window {
        /// Checked native BGRA backing layout.
        descriptor: TextureDescriptor,
        /// Actual swapchain buffer count.
        buffers: u32,
    },
    /// Accepted SDK shared image with a validated immutable layout.
    Spout {
        /// Layout validated after an actual SDK send.
        descriptor: TextureDescriptor,
        /// Normalized legacy shared-image identity, never opened by observers.
        shared_handle: u32,
    },
    /// SDK owner exists but no successful send established its complete allocation metadata.
    /// Failed/partial SDK initialization must never be reported as zero allocated bytes.
    UnknownSdk,
}

impl OutputTextureAllocation {
    /// Known descriptor payload, excluding driver padding and opaque partial SDK state.
    pub fn texel_bytes(self) -> Option<u64> {
        match self {
            Self::Window { descriptor, buffers } => descriptor.byte_len().ok()?.checked_mul(u64::from(buffers)),
            Self::Spout { descriptor, .. } => descriptor.byte_len().ok(),
            Self::UnknownSdk => None,
        }
    }
}

/// One endpoint's current incarnation and allocation; None means no endpoint object exists.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct OutputTextureSnapshot {
    /// Configured consumer identity.
    pub output: OutputId,
    /// Successful native endpoint incarnation.
    pub generation: u64,
    /// Live/retiring storage, including unknown partial SDK state.
    pub allocation: Option<OutputTextureAllocation>,
}

/// Bounded metadata inventory. Sources publish independently; construction transients and
/// opaque driver allocations remain unmeasured. No resource interfaces leave their workers.
#[derive(Clone, Debug, Serialize)]
pub struct NativeAllocationSnapshot {
    /// Actual owned plan textures, deduplicated across reused plan generations.
    pub plan: OwnedTextureSnapshot,
    /// Source-owned bridge storage and uncharged renderer aliases.
    pub sources: Vec<SourceTextureSnapshot>,
    /// Current native output owners.
    pub outputs: Vec<OutputTextureSnapshot>,
    /// Complete known owner payload total, or absent for unknown metadata/partial SDK owners.
    pub owned_texel_bytes: Option<u64>,
    /// Physical driver allocation sizes/identity are not established by these descriptors.
    pub driver_physical_bytes: Option<u64>,
}

#[cfg(test)]
mod tests;
