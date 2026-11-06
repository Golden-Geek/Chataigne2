//! Native packed video storage is distinct from the sampled RGB image contract.
mod decoder;
pub(super) use decoder::Decoder;

use super::super::device::Device;
use crate::{TextureDescriptor, TextureError};
use windows::Win32::Graphics::{Direct3D::D3D11_SRV_DIMENSION_TEXTURE2D, Direct3D11::*, Dxgi::Common::*};
use windows::core::{Error, HRESULT, Result};

pub(in crate::native) const UNIFORM_BYTES: u64 = 16;

#[derive(Clone, Copy, Debug)]
pub(in crate::native) enum Format {
    Ayuv,
    Y410,
    Y416,
}
impl Format {
    pub fn from_native(storage: u32) -> Option<Self> {
        match storage {
            100 => Some(Self::Ayuv),
            101 => Some(Self::Y410),
            102 => Some(Self::Y416),
            _ => None,
        }
    }
    pub fn native(self) -> u32 {
        match self {
            Self::Ayuv => 100,
            Self::Y410 => 101,
            Self::Y416 => 102,
        }
    }
    fn view(self) -> DXGI_FORMAT {
        match self {
            Self::Ayuv => DXGI_FORMAT_R8G8B8A8_UINT,
            Self::Y410 => DXGI_FORMAT_R10G10B10A2_UINT,
            Self::Y416 => DXGI_FORMAT_R16G16B16A16_UINT,
        }
    }
    pub fn image_bytes(self, descriptor: TextureDescriptor) -> std::result::Result<u64, TextureError> {
        let pixels = u64::from(descriptor.extent.width()) * u64::from(descriptor.extent.height());
        pixels
            .checked_mul(if matches!(self, Self::Y416) { 8 } else { 4 })
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
    pub descriptor: TextureDescriptor,
}
impl Image {
    pub fn new(device: &Device, descriptor: TextureDescriptor, format: Format) -> Result<Self> {
        let mut texture = None;
        let mut view = None;
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
            device.device.CreateShaderResourceView(
                &texture,
                Some(&D3D11_SHADER_RESOURCE_VIEW_DESC {
                    Format: format.view(),
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
            Ok(Self {
                texture,
                view: view.ok_or_else(invalid)?,
                descriptor,
            })
        }
    }
}
fn invalid() -> Error {
    Error::from_hresult(HRESULT(0x8007_0057_u32 as i32))
}

#[cfg(test)]
mod tests;
