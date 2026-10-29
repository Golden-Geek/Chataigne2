use std::marker::PhantomData;
use std::rc::Rc;

use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::core::{Error, HRESULT, Interface, Result};

use crate::{PixelFormat, TextureDescriptor};

/// Views and their texture stay on the owning render worker through completion.
pub(super) struct Texture {
    pub(super) texture: ID3D11Texture2D,
    pub(super) srv: ID3D11ShaderResourceView,
    pub(super) rtv: ID3D11RenderTargetView,
    pub(super) descriptor: TextureDescriptor,
    _image_lease: Option<golden_assets::PreparedAsset<crate::image::ImageData>>,
    _local: PhantomData<Rc<()>>,
}

impl Texture {
    pub(super) fn from_asset(
        device: &ID3D11Device,
        image: &golden_assets::PreparedAsset<crate::image::ImageData>,
    ) -> Result<Self> {
        let mut texture = Self::from_image(device, image.value())?;
        texture._image_lease = Some(image.clone());
        Ok(texture)
    }

    pub(super) fn from_image(device: &ID3D11Device, image: &crate::image::ImageData) -> Result<Self> {
        let descriptor = image.descriptor();
        let desc = D3D11_TEXTURE2D_DESC {
            Width: descriptor.extent.width(),
            Height: descriptor.extent.height(),
            MipLevels: 1,
            ArraySize: 1,
            Format: format(descriptor.format),
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE).0 as u32,
            ..Default::default()
        };
        let data = D3D11_SUBRESOURCE_DATA {
            pSysMem: image.pixels().as_ptr().cast(),
            SysMemPitch: descriptor.extent.width() * 4,
            SysMemSlicePitch: 0,
        };
        let mut texture = None;
        // SAFETY: ImageData validates tight four-byte storage and bounded dimensions. The initial
        // bytes stay immutable and alive through this creation call; no per-frame upload follows.
        unsafe { device.CreateTexture2D(&desc, Some(&data), Some(&mut texture))? };
        Self::from_native(device, texture.ok_or_else(missing_output)?, descriptor)
    }

    pub(super) fn new(device: &ID3D11Device, descriptor: TextureDescriptor) -> Result<Self> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: descriptor.extent.width(),
            Height: descriptor.extent.height(),
            MipLevels: 1,
            ArraySize: 1,
            Format: format(descriptor.format),
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE).0 as u32,
            ..Default::default()
        };
        let mut texture = None;
        // SAFETY: complete single-level descriptor and an initialized COM output slot.
        unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture))? };
        Self::from_native(device, texture.ok_or_else(missing_output)?, descriptor)
    }

    pub(super) fn from_native(
        device: &ID3D11Device,
        texture: ID3D11Texture2D,
        descriptor: TextureDescriptor,
    ) -> Result<Self> {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: owned texture, copied description and retained device interface.
        let owner = unsafe {
            texture.GetDesc(&mut desc);
            texture.GetDevice()?
        };
        let required = (D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE).0 as u32;
        if owner.as_raw() != device.as_raw()
            || desc.Width != descriptor.extent.width()
            || desc.Height != descriptor.extent.height()
            || desc.Format != format(descriptor.format)
            || desc.MipLevels != 1
            || desc.ArraySize != 1
            || desc.SampleDesc.Count != 1
            || desc.SampleDesc.Quality != 0
            || desc.BindFlags & required != required
        {
            return Err(Error::new(
                HRESULT(0x8007_0057_u32 as i32),
                "texture does not match its device, layout or required view bindings",
            ));
        }
        let mut srv = None;
        let mut rtv = None;
        // SAFETY: verified UNORM format and binding flags. Views are allocated before submission.
        // sRGB conversion is explicit in the shader; an sRGB SRV would decode twice.
        unsafe {
            device.CreateShaderResourceView(&texture, None, Some(&mut srv))?;
            device.CreateRenderTargetView(&texture, None, Some(&mut rtv))?;
        }
        Ok(Self {
            texture,
            srv: srv.ok_or_else(missing_output)?,
            rtv: rtv.ok_or_else(missing_output)?,
            descriptor,
            _image_lease: None,
            _local: PhantomData,
        })
    }
}

pub(super) fn format(format: PixelFormat) -> DXGI_FORMAT {
    match format {
        PixelFormat::Rgba8Unorm => DXGI_FORMAT_R8G8B8A8_UNORM,
        PixelFormat::Bgra8Unorm => DXGI_FORMAT_B8G8R8A8_UNORM,
    }
}

fn missing_output() -> Error {
    Error::new(HRESULT(0x8000_4005_u32 as i32), "D3D11 returned no texture or view")
}
