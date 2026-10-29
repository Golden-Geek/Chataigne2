use std::marker::PhantomData;
use std::rc::Rc;

use windows::Win32::Graphics::Direct3D::D3D11_SRV_DIMENSION_TEXTURE2D;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_TYPELESS, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
    DXGI_FORMAT_R8G8B8A8_TYPELESS, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM_SRGB, DXGI_SAMPLE_DESC,
};
use windows::core::{Error, HRESULT, Interface, Result};

use crate::{AlphaMode, ColorEncoding, PixelFormat, TextureDescriptor};

/// Views and their texture stay on the owning render worker through completion.
pub(super) struct Texture {
    pub(super) texture: ID3D11Texture2D,
    pub(super) srv: ID3D11ShaderResourceView,
    decoded_srv: Option<ID3D11ShaderResourceView>,
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
        let data = D3D11_SUBRESOURCE_DATA {
            pSysMem: image.pixels().as_ptr().cast(),
            SysMemPitch: descriptor.extent.width() * 4,
            SysMemSlicePitch: 0,
        };
        Self::allocate(device, descriptor, true, Some(&data))
    }

    pub(super) fn new(device: &ID3D11Device, descriptor: TextureDescriptor) -> Result<Self> {
        Self::allocate(device, descriptor, false, None)
    }

    /// Source-local storage may expose hardware sRGB decoding without changing shared formats.
    pub(super) fn new_sampled(device: &ID3D11Device, descriptor: TextureDescriptor) -> Result<Self> {
        Self::allocate(device, descriptor, true, None)
    }

    fn allocate(
        device: &ID3D11Device,
        descriptor: TextureDescriptor,
        sampled: bool,
        data: Option<&D3D11_SUBRESOURCE_DATA>,
    ) -> Result<Self> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: descriptor.extent.width(),
            Height: descriptor.extent.height(),
            MipLevels: 1,
            ArraySize: 1,
            Format: if sampled && hardware_decode(descriptor) {
                typeless_format(descriptor.format)
            } else {
                format(descriptor.format)
            },
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE).0 as u32,
            ..Default::default()
        };
        let mut texture = None;
        // SAFETY: complete single-level descriptor and initialized output. Optional ImageData
        // validates tight four-byte storage and stays alive throughout this creation call.
        unsafe { device.CreateTexture2D(&desc, data.map(std::ptr::from_ref), Some(&mut texture))? };
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
            || !(desc.Format == format(descriptor.format)
                || (hardware_decode(descriptor) && desc.Format == typeless_format(descriptor.format)))
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
        let mut decoded_srv = None;
        let srv_desc = |view_format| D3D11_SHADER_RESOURCE_VIEW_DESC {
            Format: view_format,
            ViewDimension: D3D11_SRV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_SRV {
                    MostDetailedMip: 0,
                    MipLevels: 1,
                },
            },
        };
        // SAFETY: validated compatible storage and binding flags. All views are prepared before
        // submission. Raw views retain encoded bytes; the optional SRGB view decodes RGB only.
        unsafe {
            device.CreateShaderResourceView(&texture, Some(&srv_desc(format(descriptor.format))), Some(&mut srv))?;
            device.CreateRenderTargetView(
                &texture,
                Some(&D3D11_RENDER_TARGET_VIEW_DESC {
                    Format: format(descriptor.format),
                    ViewDimension: D3D11_RTV_DIMENSION_TEXTURE2D,
                    Anonymous: D3D11_RENDER_TARGET_VIEW_DESC_0 {
                        Texture2D: D3D11_TEX2D_RTV { MipSlice: 0 },
                    },
                }),
                Some(&mut rtv),
            )?;
            if desc.Format == typeless_format(descriptor.format) {
                device.CreateShaderResourceView(
                    &texture,
                    Some(&srv_desc(srgb_format(descriptor.format))),
                    Some(&mut decoded_srv),
                )?;
                if decoded_srv.is_none() {
                    return Err(missing_output());
                }
            }
        }
        Ok(Self {
            texture,
            srv: srv.ok_or_else(missing_output)?,
            decoded_srv,
            rtv: rtv.ok_or_else(missing_output)?,
            descriptor,
            _image_lease: None,
            _local: PhantomData,
        })
    }

    pub(super) fn sampling_view(&self) -> (&ID3D11ShaderResourceView, ColorEncoding) {
        match &self.decoded_srv {
            Some(view) => (view, ColorEncoding::Linear),
            None => (&self.srv, self.descriptor.color_encoding),
        }
    }
}

fn hardware_decode(descriptor: TextureDescriptor) -> bool {
    // Encoded-premultiplied storage must be unpremultiplied before decoding, in the shader.
    descriptor.color_encoding == ColorEncoding::Srgb && descriptor.alpha_mode != AlphaMode::Premultiplied
}

fn typeless_format(format: PixelFormat) -> DXGI_FORMAT {
    match format {
        PixelFormat::Rgba8Unorm => DXGI_FORMAT_R8G8B8A8_TYPELESS,
        PixelFormat::Bgra8Unorm => DXGI_FORMAT_B8G8R8A8_TYPELESS,
    }
}

fn srgb_format(format: PixelFormat) -> DXGI_FORMAT {
    match format {
        PixelFormat::Rgba8Unorm => DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
        PixelFormat::Bgra8Unorm => DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
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
