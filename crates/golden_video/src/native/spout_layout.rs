//! SDK registry values and native DXGI storage are distinct namespaces for legacy senders.
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM};

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
    let format = super::texture::source_format(native_format(registered).0 as u32)?;
    let mut descriptor = SpoutSource::discovered_descriptor(extent, format);
    if registered == 22 {
        descriptor.alpha_mode = AlphaMode::Opaque;
    }
    Some(descriptor)
}

pub(super) fn observed(binding: &SpoutSource, extent: Extent2D, registered: u32) -> Option<TextureDescriptor> {
    let descriptor = descriptor(extent, registered)?;
    interpreted(binding, descriptor, native_format(registered))
}

/// Zero is unspecified metadata, not a native pixel layout. Only an opened/authenticated
/// alias can supply its format; nonzero registrations retain their exact native contract.
pub(super) fn descriptor_from_alias(
    extent: Extent2D,
    registered: u32,
    native: DXGI_FORMAT,
) -> Option<TextureDescriptor> {
    if registered == 0 {
        let format = super::texture::source_format(native.0 as u32)?;
        Some(SpoutSource::discovered_descriptor(extent, format))
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
