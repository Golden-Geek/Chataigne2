//! SDK registry values and native DXGI storage are distinct namespaces for legacy senders.
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM};

use crate::plan::{SpoutMetadata, SpoutSource};
use crate::{AlphaMode, Extent2D, TextureDescriptor};

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
    match binding.metadata {
        SpoutMetadata::Declared => binding.observed_descriptor(extent, descriptor.format),
        SpoutMetadata::Discover => binding.accepts(descriptor).then_some(descriptor),
    }
}
