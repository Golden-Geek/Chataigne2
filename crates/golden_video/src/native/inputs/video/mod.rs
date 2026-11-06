//! Native video storage and plane views are distinct from the sampled RGB image contract.
mod decoder;
pub(super) use decoder::Decoder;

use super::super::device::Device;
use crate::{Extent2D, TextureDescriptor, TextureError};
use windows::Win32::Graphics::{Direct3D::D3D11_SRV_DIMENSION_TEXTURE2D, Direct3D11::*, Dxgi::Common::*};
use windows::core::{Error, HRESULT, Result};

pub(in crate::native) const UNIFORM_BYTES: u64 = 16;

#[derive(Clone, Copy, Debug)]
pub(in crate::native) enum Format {
    Ayuv,
    Y410,
    Y416,
    Nv12,
    P010,
    P016,
    Yuy2,
    Y210,
    Y216,
}
impl Format {
    pub fn from_native(storage: u32) -> Option<Self> {
        match storage {
            100 => Some(Self::Ayuv),
            101 => Some(Self::Y410),
            102 => Some(Self::Y416),
            103 => Some(Self::Nv12),
            104 => Some(Self::P010),
            105 => Some(Self::P016),
            107 => Some(Self::Yuy2),
            108 => Some(Self::Y210),
            109 => Some(Self::Y216),
            _ => None,
        }
    }
    pub fn native(self) -> u32 {
        match self {
            Self::Ayuv => 100,
            Self::Y410 => 101,
            Self::Y416 => 102,
            Self::Nv12 => 103,
            Self::P010 => 104,
            Self::P016 => 105,
            Self::Yuy2 => 107,
            Self::Y210 => 108,
            Self::Y216 => 109,
        }
    }
    fn view(self) -> DXGI_FORMAT {
        match self {
            Self::Ayuv | Self::Yuy2 => DXGI_FORMAT_R8G8B8A8_UINT,
            Self::Y410 => DXGI_FORMAT_R10G10B10A2_UINT,
            Self::Y416 | Self::Y210 | Self::Y216 => DXGI_FORMAT_R16G16B16A16_UINT,
            Self::Nv12 => DXGI_FORMAT_R8_UINT,
            Self::P010 | Self::P016 => DXGI_FORMAT_R16_UINT,
        }
    }
    fn chroma_view(self) -> Option<DXGI_FORMAT> {
        match self {
            Self::Nv12 => Some(DXGI_FORMAT_R8G8_UINT),
            Self::P010 | Self::P016 => Some(DXGI_FORMAT_R16G16_UINT),
            _ => None,
        }
    }
    pub fn has_alpha(self) -> bool {
        matches!(self, Self::Ayuv | Self::Y410 | Self::Y416)
    }
    pub fn accepts_extent(self, extent: Extent2D) -> bool {
        match self {
            Self::Nv12 | Self::P010 | Self::P016 => {
                extent.width().is_multiple_of(2) && extent.height().is_multiple_of(2)
            }
            Self::Yuy2 | Self::Y210 | Self::Y216 => extent.width().is_multiple_of(2),
            _ => true,
        }
    }
    pub fn image_bytes(self, descriptor: TextureDescriptor) -> std::result::Result<u64, TextureError> {
        let width = u64::from(descriptor.extent.width());
        let height = u64::from(descriptor.extent.height());
        // Round invalid odd planar extents conservatively; admission rejects them before allocation.
        let (columns, rows, element_bytes) = match self {
            Self::Nv12 => (width.div_ceil(2) * 2, height + height.div_ceil(2), 1),
            Self::P010 | Self::P016 => (width.div_ceil(2) * 2, height + height.div_ceil(2), 2),
            Self::Ayuv | Self::Y410 => (width, height, 4),
            Self::Y416 => (width, height, 8),
            Self::Yuy2 => (width.div_ceil(2), height, 4),
            Self::Y210 | Self::Y216 => (width.div_ceil(2), height, 8),
        };
        columns
            .checked_mul(rows)
            .and_then(|elements| elements.checked_mul(element_bytes))
            .ok_or_else(|| overflow(descriptor))
    }
    pub fn source_bytes(self, descriptor: TextureDescriptor) -> std::result::Result<u64, TextureError> {
        self.image_bytes(descriptor)?
            .checked_mul(2)
            .and_then(|raw| descriptor.byte_len().ok()?.checked_mul(3)?.checked_add(raw))
            .and_then(|images| images.checked_add(UNIFORM_BYTES))
            .ok_or_else(|| overflow(descriptor))
    }
}
fn overflow(d: TextureDescriptor) -> TextureError {
    TextureError::ByteSizeOverflow {
        width: d.extent.width(),
        height: d.extent.height(),
    }
}

/// Read-only native capture: Y410/Y416 have no RTV support. Only successful SDK copies
/// become observable, and the source's existing completion authority retains this storage.
pub(in crate::native) struct Image {
    pub texture: ID3D11Texture2D,
    pub view: ID3D11ShaderResourceView,
    pub chroma: Option<ID3D11ShaderResourceView>,
    pub descriptor: TextureDescriptor,
}
impl Image {
    pub fn new(device: &Device, descriptor: TextureDescriptor, format: Format) -> Result<Self> {
        if !format.accepts_extent(descriptor.extent) {
            return Err(invalid());
        }
        let mut texture = None;
        unsafe {
            device.device.CreateTexture2D(
                &D3D11_TEXTURE2D_DESC {
                    Width: descriptor.extent.width(),
                    Height: descriptor.extent.height(),
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: DXGI_FORMAT(format.native() as i32),
                    SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                    ..Default::default()
                },
                None,
                Some(&mut texture),
            )?;
            let texture = texture.ok_or_else(invalid)?;
            let view = create_view(&device.device, &texture, format.view())?;
            let chroma = format
                .chroma_view()
                .map(|format| create_view(&device.device, &texture, format))
                .transpose()?;
            Ok(Self {
                texture,
                view,
                chroma,
                descriptor,
            })
        }
    }
}
fn create_view(
    device: &ID3D11Device,
    texture: &ID3D11Texture2D,
    format: DXGI_FORMAT,
) -> Result<ID3D11ShaderResourceView> {
    let mut view = None;
    unsafe {
        device.CreateShaderResourceView(
            texture,
            Some(&D3D11_SHADER_RESOURCE_VIEW_DESC {
                Format: format,
                ViewDimension: D3D11_SRV_DIMENSION_TEXTURE2D,
                Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                    Texture2D: D3D11_TEX2D_SRV {
                        MostDetailedMip: 0,
                        MipLevels: 1,
                    },
                },
            }),
            Some(&mut view),
        )?;
    }
    view.ok_or_else(invalid)
}
fn invalid() -> Error {
    Error::from_hresult(HRESULT(0x8007_0057_u32 as i32))
}

#[cfg(test)]
mod tests;
