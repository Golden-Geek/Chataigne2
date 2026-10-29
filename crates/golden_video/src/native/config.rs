use std::collections::BTreeSet;
use std::num::NonZeroIsize;
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::runtime::{RuntimeConfig, RuntimeStartError};
use crate::{AlphaMode, ColorEncoding, Extent2D, OutputId, PixelFormat, TextureDescriptor, TextureId};

/// DXGI adapter LUID. Enumeration indices are not persistent adapter identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AdapterId {
    /// Low unsigned LUID word.
    pub low: u32,
    /// High signed LUID word.
    pub high: i32,
}

/// Borrowed host-owned HWND identity, containing no native object ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowTarget(NonZeroIsize);

impl WindowTarget {
    /// Borrows a native window for one renderer lifetime.
    ///
    /// # Safety
    /// The caller must own this live HWND, prevent another renderer from attaching a swapchain,
    /// and keep the HWND alive until `RuntimeRetirement::wait` succeeds. An event or timeout is
    /// not retirement. The shared desktop host's native-presentation task provides this ordering.
    pub unsafe fn from_raw(hwnd: isize) -> Result<Self, NativeConfigError> {
        NonZeroIsize::new(hwnd)
            .map(Self)
            .ok_or(NativeConfigError::InvalidWindow)
    }

    pub(crate) fn raw(self) -> isize {
        self.0.get()
    }
}

/// Immutable logical output to native consumer binding.
#[derive(Clone, Debug)]
pub enum OutputBinding {
    /// Fixed-size budgeted backbuffers, stretched by DXGI when the host window changes size.
    /// Physical output/preview geometry and budgeted buffer resizing are separate contracts.
    Window {
        /// Consumer identity referenced by a render plan.
        id: OutputId,
        /// Window retained by its host through actual renderer retirement.
        window: WindowTarget,
        /// Admitted backing extent; never silently follows desktop/window dimensions.
        extent: Extent2D,
    },
    /// A named GPU-only Spout sender on the renderer's exact caller-owned device.
    Spout {
        /// Consumer identity referenced by a render plan.
        id: OutputId,
        /// Exact sender name. Name collision/renaming is a preparation failure.
        name: String,
        /// Explicit storage, size, transfer and alpha contract supplied to consumers.
        descriptor: TextureDescriptor,
    },
}

impl OutputBinding {
    /// Logical consumer identity.
    pub fn id(&self) -> OutputId {
        match self {
            Self::Window { id, .. } | Self::Spout { id, .. } => *id,
        }
    }

    /// Native output storage/conversion contract. Windows are opaque sRGB over black.
    pub fn descriptor(&self) -> TextureDescriptor {
        match self {
            Self::Window { extent, .. } => TextureDescriptor {
                extent: *extent,
                format: PixelFormat::Bgra8Unorm,
                color_encoding: ColorEncoding::Srgb,
                alpha_mode: AlphaMode::Opaque,
            },
            Self::Spout { descriptor, .. } => *descriptor,
        }
    }

    pub(super) fn persistent_copies(&self) -> u64 {
        match self {
            Self::Window { .. } => 2,
            Self::Spout { .. } => 1,
        }
    }
}

/// Explicit hardware adapter and bounded immutable native consumers.
#[derive(Clone, Debug)]
pub struct D3d11Config {
    /// Hardware adapter selected by observed LUID; no automatic WARP or adapter fallback.
    pub adapter: AdapterId,
    /// Fixed bindings admitted before their native pools are created.
    pub outputs: Vec<OutputBinding>,
    /// At most 32 fixed live bindings, each with its own bounded IO/device worker.
    pub inputs: Vec<SpoutInput>,
}

/// One explicitly selected Spout source and its admitted immutable image contract.
#[derive(Clone, Debug)]
pub struct SpoutInput {
    /// External texture identity referenced by compiled plans.
    pub texture: TextureId,
    /// Exact sender selector; the active-sender UI is never used.
    pub sender: String,
    /// Maximum admitted extent and required storage/transfer/alpha convention.
    /// Actual extent is observed separately; incompatible senders remain unavailable.
    pub descriptor: TextureDescriptor,
    /// Dedicated worker polling interval, from one millisecond through one second.
    pub poll_interval: Duration,
}

impl D3d11Config {
    pub(crate) fn validate(&self, runtime: &RuntimeConfig) -> Result<(), RuntimeStartError> {
        let invalid = RuntimeStartError::InvalidConfig;
        if self.inputs.len() > runtime.plan_limits.max_external_textures || self.inputs.len() > 32 {
            return Err(invalid("native input bindings exceed admitted source-worker limit"));
        }
        let mut inputs = BTreeSet::new();
        for input in &self.inputs {
            if !inputs.insert(input.texture)
                || input.sender.is_empty()
                || input.sender.len() > 255
                || input.sender.contains('\0')
            {
                return Err(invalid("invalid or duplicate native input binding"));
            }
            let extent = input.descriptor.extent;
            let limit = runtime.plan_limits.max_texture_dimension.min(16_384);
            if extent.width() > limit || extent.height() > limit {
                return Err(invalid("native input extent exceeds admitted device/plan limit"));
            }
            if input.poll_interval < Duration::from_millis(1)
                || input.poll_interval > Duration::from_secs(1)
                || Instant::now().checked_add(input.poll_interval).is_none()
            {
                return Err(invalid(
                    "native input polling interval is outside one millisecond to one second",
                ));
            }
        }
        if self.outputs.len() > runtime.plan_limits.max_outputs {
            return Err(invalid("native output bindings exceed plan output limit"));
        }
        let mut ids = BTreeSet::new();
        let mut windows = BTreeSet::new();
        let mut names = BTreeSet::new();
        for output in &self.outputs {
            if !ids.insert(output.id()) {
                return Err(invalid("duplicate native output identity"));
            }
            let extent = output.descriptor().extent;
            let limit = runtime.plan_limits.max_texture_dimension.min(16_384);
            if extent.width() > limit || extent.height() > limit {
                return Err(invalid("native output extent exceeds admitted device/plan limit"));
            }
            match output {
                OutputBinding::Window { window, .. } if !windows.insert(window.raw()) => {
                    return Err(invalid("multiple swapchains cannot attach to one native window"));
                }
                OutputBinding::Spout { name, .. }
                    if name.is_empty() || name.len() > 255 || name.contains('\0') || !names.insert(name) =>
                {
                    return Err(invalid("invalid or duplicate Spout sender name"));
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// Invalid native handle configuration; no native ownership has been acquired.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum NativeConfigError {
    /// Zero is not a window identity.
    #[error("a native presentation target requires a nonzero host-owned HWND")]
    InvalidWindow,
}
