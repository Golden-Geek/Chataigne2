//! SDK registry values and native DXGI storage are distinct namespaces for legacy senders.
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_B8G8R8X8_UNORM};

use crate::plan::{SpoutMetadata, SpoutSource};
use crate::{AlphaMode, Extent2D, TextureDescriptor};

#[cfg(test)]
mod tests;

pub(super) fn native_format(registered: u32) -> DXGI_FORMAT {
    match registered {
        // D3DFMT_A8R8G8B8 and D3DFMT_X8R8G8B8, as mapped by the pinned SDK.
        21 | 22 => DXGI_FORMAT_B8G8R8A8_UNORM,
        _ => DXGI_FORMAT(registered as i32),
    }
}

pub(super) fn descriptor(extent: Extent2D, registered: u32) -> Option<TextureDescriptor> {
    let mut descriptor = native_descriptor(extent, native_format(registered))?;
    if registered == 22 {
        descriptor.alpha_mode = AlphaMode::Opaque;
    }
    Some(descriptor)
}

fn native_descriptor(extent: Extent2D, native: DXGI_FORMAT) -> Option<TextureDescriptor> {
    let format = super::texture::source_format(native.0 as u32)?;
    let mut descriptor = SpoutSource::discovered_descriptor(extent, format);
    if let Some(video) = super::inputs::video::Format::from_native(native.0 as u32) {
        if !video.accepts_extent(extent) {
            return None;
        }
        if !video.has_alpha() {
            descriptor.alpha_mode = AlphaMode::Opaque;
        }
    }
    Some(descriptor)
}

pub(super) fn observed(binding: &SpoutSource, extent: Extent2D, registered: u32) -> Option<TextureDescriptor> {
    let descriptor = descriptor(extent, registered)?;
    interpreted(binding, descriptor, native_format(registered))
}

/// Zero is unspecified metadata, not a native pixel layout. Only an opened/authenticated
/// alias can supply its format. Legacy XRGB authenticates actual BGRA/BGRX storage;
/// other nonzero registrations retain their exact native contract.
pub(super) fn descriptor_from_alias(
    extent: Extent2D,
    registered: u32,
    native: DXGI_FORMAT,
) -> Option<TextureDescriptor> {
    if registered == 0 || (registered == 22 && native == DXGI_FORMAT_B8G8R8X8_UNORM) {
        native_descriptor(extent, native)
    } else if native == native_format(registered) {
        descriptor(extent, registered)
    } else {
        None
    }
}

pub(super) fn observed_alias(
    binding: &SpoutSource,
    extent: Extent2D,
    registered: u32,
    native: DXGI_FORMAT,
) -> Option<TextureDescriptor> {
    interpreted(binding, descriptor_from_alias(extent, registered, native)?, native)
}

fn interpreted(binding: &SpoutSource, descriptor: TextureDescriptor, native: DXGI_FORMAT) -> Option<TextureDescriptor> {
    match binding.metadata {
        SpoutMetadata::Declared => {
            let format = if super::texture::typeless_accepts(native, binding.descriptor.format) {
                binding.descriptor.format
            } else {
                descriptor.format
            };
            binding.observed_descriptor(descriptor.extent, format)
        }
        SpoutMetadata::Discover => binding.accepts(descriptor).then_some(descriptor),
    }
}
