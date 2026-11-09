use std::marker::PhantomData;
use std::rc::Rc;

use windows::Win32::Graphics::Direct3D::D3D11_SRV_DIMENSION_TEXTURE2D;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::{Error, HRESULT, Interface, Result};

use crate::{AlphaMode, ColorEncoding, PixelFormat, TextureDescriptor};

/// Views and their texture stay on the owning render worker through completion.
pub(super) struct Texture {
    pub(super) texture: ID3D11Texture2D,
    pub(super) srv: ID3D11ShaderResourceView,
    decoded_srv: Option<ID3D11ShaderResourceView>,
    pub(super) rtv: ID3D11RenderTargetView,
    pub(super) descriptor: TextureDescriptor,
    // Contents follow the reused allocation, rather than the render-plan generation.
    pub(super) image_sequence: std::cell::Cell<Option<u64>>,
    pub(super) image_visible: std::cell::Cell<u64>,
    pub(super) software_upload: std::cell::Cell<Option<super::SoftwareUploadTrace>>,
    _image_lease: Option<ImageLease>,
    _local: PhantomData<Rc<()>>,
    // Last field: release all native texture/views before publishing this object's release.
    allocation: Option<super::allocations::TextureAllocation>,
}

enum ImageLease {
    Prepared {
        _lease: golden_assets::PreparedAsset<crate::image::ImageData>,
    },
    Canvas {
        _image: std::sync::Arc<crate::image::ImageData>,
    },
}

impl Texture {
    pub(super) fn from_asset(
        device: &ID3D11Device,
        image: &golden_assets::PreparedAsset<crate::image::ImageData>,
    ) -> Result<Self> {
        let mut texture = Self::from_image(device, image.value())?;
        texture._image_lease = Some(ImageLease::Prepared { _lease: image.clone() });
        Ok(texture)
    }
    pub(super) fn from_canvas_image(
        device: &ID3D11Device,
        image: &std::sync::Arc<crate::image::ImageData>,
    ) -> Result<Self> {
        let mut texture = Self::from_image(device, image)?;
        texture._image_lease = Some(ImageLease::Canvas { _image: image.clone() });
        Ok(texture)
    }

    pub(super) fn from_image(device: &ID3D11Device, image: &crate::image::ImageData) -> Result<Self> {
        let descriptor = image.descriptor();
        let data = D3D11_SUBRESOURCE_DATA {
            pSysMem: image.pixels().as_ptr().cast(),
            SysMemPitch: descriptor
                .extent
                .width()
                .checked_mul(descriptor.format.bytes_per_pixel() as u32)
                .ok_or_else(|| Error::from_hresult(HRESULT(0x8007_0057_u32 as i32)))?,
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
                typeless_format(descriptor.format).ok_or_else(missing_output)?
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
        // validates tightly packed format-sized storage and stays alive throughout creation.
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
                || (hardware_decode(descriptor) && Some(desc.Format) == typeless_format(descriptor.format)))
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
            if Some(desc.Format) == typeless_format(descriptor.format) {
                device.CreateShaderResourceView(
                    &texture,
                    Some(&srv_desc(srgb_format(descriptor.format).ok_or_else(missing_output)?)),
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
            image_sequence: std::cell::Cell::new(None),
            image_visible: std::cell::Cell::new(0),
            software_upload: std::cell::Cell::new(None),
            _image_lease: None,
            _local: PhantomData,
            allocation: None,
        })
    }

    pub(super) fn sampling_view(&self) -> (&ID3D11ShaderResourceView, ColorEncoding) {
        match &self.decoded_srv {
            Some(view) => (view, ColorEncoding::Linear),
            None => (&self.srv, self.descriptor.color_encoding),
        }
    }

    pub(super) fn track(&mut self, owner: &super::allocations::TextureAllocations) -> Result<()> {
        debug_assert!(
            self.allocation.is_none(),
            "one native object has one allocation observation"
        );
        let bytes = self
            .descriptor
            .byte_len()
            .map_err(|_| Error::from_hresult(HRESULT(0x8007_0057_u32 as i32)))?;
        self.allocation = Some(owner.register(bytes));
        Ok(())
    }
}

fn hardware_decode(descriptor: TextureDescriptor) -> bool {
    // Encoded-premultiplied storage must be unpremultiplied before decoding, in the shader.
    descriptor.color_encoding == ColorEncoding::Srgb
        && descriptor.alpha_mode != AlphaMode::Premultiplied
        && srgb_format(descriptor.format).is_some()
}

fn typeless_format(format: PixelFormat) -> Option<DXGI_FORMAT> {
    Some(match format {
        PixelFormat::Rgba8Unorm => DXGI_FORMAT_R8G8B8A8_TYPELESS,
        PixelFormat::Bgra8Unorm => DXGI_FORMAT_B8G8R8A8_TYPELESS,
        PixelFormat::Bgrx8Unorm => DXGI_FORMAT_B8G8R8X8_TYPELESS,
        PixelFormat::Rgba8Snorm => DXGI_FORMAT_R8G8B8A8_TYPELESS,
        PixelFormat::Rgba16Float | PixelFormat::Rgba16Unorm | PixelFormat::Rgba16Snorm => {
            DXGI_FORMAT_R16G16B16A16_TYPELESS
        }
        PixelFormat::Rgb10A2Unorm | PixelFormat::Rgb10XrA2Unorm => DXGI_FORMAT_R10G10B10A2_TYPELESS,
        PixelFormat::Rgba32Float => DXGI_FORMAT_R32G32B32A32_TYPELESS,
        PixelFormat::R8Unorm => DXGI_FORMAT_R8_TYPELESS,
        PixelFormat::R8Snorm => DXGI_FORMAT_R8_TYPELESS,
        PixelFormat::Rg8Unorm => DXGI_FORMAT_R8G8_TYPELESS,
        PixelFormat::Rg8Snorm => DXGI_FORMAT_R8G8_TYPELESS,
        PixelFormat::R16Unorm => DXGI_FORMAT_R16_TYPELESS,
        PixelFormat::R16Snorm => DXGI_FORMAT_R16_TYPELESS,
        PixelFormat::Rg16Unorm => DXGI_FORMAT_R16G16_TYPELESS,
        PixelFormat::Rg16Snorm => DXGI_FORMAT_R16G16_TYPELESS,
        PixelFormat::R16Float => DXGI_FORMAT_R16_TYPELESS,
        PixelFormat::Rg16Float => DXGI_FORMAT_R16G16_TYPELESS,
        PixelFormat::R32Float => DXGI_FORMAT_R32_TYPELESS,
        PixelFormat::Rg11B10Float | PixelFormat::A8Unorm => return None,
        PixelFormat::Rg32Float => DXGI_FORMAT_R32G32_TYPELESS,
        PixelFormat::Bgr565Unorm | PixelFormat::Bgra5551Unorm | PixelFormat::Bgra4444Unorm => return None,
        PixelFormat::Rgba8Uint | PixelFormat::Rgba8Sint => DXGI_FORMAT_R8G8B8A8_TYPELESS,
        PixelFormat::Rgba16Uint | PixelFormat::Rgba16Sint => DXGI_FORMAT_R16G16B16A16_TYPELESS,
        PixelFormat::Rgba32Uint | PixelFormat::Rgba32Sint => DXGI_FORMAT_R32G32B32A32_TYPELESS,
        PixelFormat::Rg8Uint | PixelFormat::Rg8Sint => DXGI_FORMAT_R8G8_TYPELESS,
        PixelFormat::Rg16Uint | PixelFormat::Rg16Sint => DXGI_FORMAT_R16G16_TYPELESS,
        PixelFormat::Rg32Uint | PixelFormat::Rg32Sint => DXGI_FORMAT_R32G32_TYPELESS,
        PixelFormat::R8Uint | PixelFormat::R8Sint => DXGI_FORMAT_R8_TYPELESS,
        PixelFormat::R16Uint | PixelFormat::R16Sint => DXGI_FORMAT_R16_TYPELESS,
        PixelFormat::R32Uint | PixelFormat::R32Sint => DXGI_FORMAT_R32_TYPELESS,
        PixelFormat::Rgb10A2Uint => DXGI_FORMAT_R10G10B10A2_TYPELESS,
    })
}

fn srgb_format(format: PixelFormat) -> Option<DXGI_FORMAT> {
    match format {
        PixelFormat::Rgba8Unorm => Some(DXGI_FORMAT_R8G8B8A8_UNORM_SRGB),
        PixelFormat::Bgra8Unorm => Some(DXGI_FORMAT_B8G8R8A8_UNORM_SRGB),
        // Keep BGRX's sRGB decode in the shader: the native decoder on the integrated GPU
        // exceeds the source-conformance precision target. Its raw SRV still supplies alpha one.
        PixelFormat::Bgrx8Unorm => None,
        PixelFormat::Rgba16Float
        | PixelFormat::Rgba32Float
        | PixelFormat::Rgba8Snorm
        | PixelFormat::Rgba16Unorm
        | PixelFormat::Rgba16Snorm
        | PixelFormat::Rgb10A2Unorm
        | PixelFormat::R8Unorm
        | PixelFormat::R8Snorm
        | PixelFormat::Rg8Unorm
        | PixelFormat::Rg8Snorm
        | PixelFormat::R16Unorm
        | PixelFormat::R16Snorm
        | PixelFormat::Rg16Unorm
        | PixelFormat::Rg16Snorm
        | PixelFormat::R16Float
        | PixelFormat::Rg16Float
        | PixelFormat::R32Float
        | PixelFormat::Rg11B10Float
        | PixelFormat::A8Unorm
        | PixelFormat::Rg32Float
        | PixelFormat::Bgr565Unorm
        | PixelFormat::Bgra5551Unorm
        | PixelFormat::Bgra4444Unorm
        | PixelFormat::Rgb10XrA2Unorm
        | PixelFormat::Rgba8Uint
        | PixelFormat::Rgba8Sint
        | PixelFormat::Rgba16Uint
        | PixelFormat::Rgba16Sint
        | PixelFormat::Rgba32Uint
        | PixelFormat::Rgba32Sint
        | PixelFormat::Rg8Uint
        | PixelFormat::Rg8Sint
        | PixelFormat::Rg16Uint
        | PixelFormat::Rg16Sint
        | PixelFormat::Rg32Uint
        | PixelFormat::Rg32Sint
        | PixelFormat::R8Uint
        | PixelFormat::R8Sint
        | PixelFormat::R16Uint
        | PixelFormat::R16Sint
        | PixelFormat::R32Uint
        | PixelFormat::R32Sint
        | PixelFormat::Rgb10A2Uint => None,
    }
}

pub(super) fn format(format: PixelFormat) -> DXGI_FORMAT {
    match format {
        PixelFormat::Rgba8Unorm => DXGI_FORMAT_R8G8B8A8_UNORM,
        PixelFormat::Bgra8Unorm => DXGI_FORMAT_B8G8R8A8_UNORM,
        PixelFormat::Bgrx8Unorm => DXGI_FORMAT_B8G8R8X8_UNORM,
        PixelFormat::Rgba8Snorm => DXGI_FORMAT_R8G8B8A8_SNORM,
        PixelFormat::Rgba16Unorm => DXGI_FORMAT_R16G16B16A16_UNORM,
        PixelFormat::Rgba16Snorm => DXGI_FORMAT_R16G16B16A16_SNORM,
        // XR_BIAS has no shader/RTV operations of its own. Its compatible raw UNORM
        // storage preserves every bit; the sampling shader applies the explicit bias.
        PixelFormat::Rgb10A2Unorm | PixelFormat::Rgb10XrA2Unorm => DXGI_FORMAT_R10G10B10A2_UNORM,
        PixelFormat::Rgba16Float => DXGI_FORMAT_R16G16B16A16_FLOAT,
        PixelFormat::Rgba32Float => DXGI_FORMAT_R32G32B32A32_FLOAT,
        PixelFormat::R8Unorm => DXGI_FORMAT_R8_UNORM,
        PixelFormat::R8Snorm => DXGI_FORMAT_R8_SNORM,
        PixelFormat::Rg8Unorm => DXGI_FORMAT_R8G8_UNORM,
        PixelFormat::Rg8Snorm => DXGI_FORMAT_R8G8_SNORM,
        PixelFormat::R16Unorm => DXGI_FORMAT_R16_UNORM,
        PixelFormat::R16Snorm => DXGI_FORMAT_R16_SNORM,
        PixelFormat::Rg16Unorm => DXGI_FORMAT_R16G16_UNORM,
        PixelFormat::Rg16Snorm => DXGI_FORMAT_R16G16_SNORM,
        PixelFormat::R16Float => DXGI_FORMAT_R16_FLOAT,
        PixelFormat::Rg16Float => DXGI_FORMAT_R16G16_FLOAT,
        PixelFormat::R32Float => DXGI_FORMAT_R32_FLOAT,
        PixelFormat::Rg11B10Float => DXGI_FORMAT_R11G11B10_FLOAT,
        PixelFormat::A8Unorm => DXGI_FORMAT_A8_UNORM,
        PixelFormat::Rg32Float => DXGI_FORMAT_R32G32_FLOAT,
        PixelFormat::Bgr565Unorm => DXGI_FORMAT_B5G6R5_UNORM,
        PixelFormat::Bgra5551Unorm => DXGI_FORMAT_B5G5R5A1_UNORM,
        PixelFormat::Bgra4444Unorm => DXGI_FORMAT_B4G4R4A4_UNORM,
        PixelFormat::Rgba8Uint => DXGI_FORMAT_R8G8B8A8_UINT,
        PixelFormat::Rgba8Sint => DXGI_FORMAT_R8G8B8A8_SINT,
        PixelFormat::Rgba16Uint => DXGI_FORMAT_R16G16B16A16_UINT,
        PixelFormat::Rgba16Sint => DXGI_FORMAT_R16G16B16A16_SINT,
        PixelFormat::Rgba32Uint => DXGI_FORMAT_R32G32B32A32_UINT,
        PixelFormat::Rgba32Sint => DXGI_FORMAT_R32G32B32A32_SINT,
        PixelFormat::Rg8Uint => DXGI_FORMAT_R8G8_UINT,
        PixelFormat::Rg8Sint => DXGI_FORMAT_R8G8_SINT,
        PixelFormat::Rg16Uint => DXGI_FORMAT_R16G16_UINT,
        PixelFormat::Rg16Sint => DXGI_FORMAT_R16G16_SINT,
        PixelFormat::Rg32Uint => DXGI_FORMAT_R32G32_UINT,
        PixelFormat::Rg32Sint => DXGI_FORMAT_R32G32_SINT,
        PixelFormat::R8Uint => DXGI_FORMAT_R8_UINT,
        PixelFormat::R8Sint => DXGI_FORMAT_R8_SINT,
        PixelFormat::R16Uint => DXGI_FORMAT_R16_UINT,
        PixelFormat::R16Sint => DXGI_FORMAT_R16_SINT,
        PixelFormat::R32Uint => DXGI_FORMAT_R32_UINT,
        PixelFormat::R32Sint => DXGI_FORMAT_R32_SINT,
        PixelFormat::Rgb10A2Uint => DXGI_FORMAT_R10G10B10A2_UINT,
    }
}

pub(super) fn source_format(storage: u32) -> Option<PixelFormat> {
    if let Some(format) = typeless_default(DXGI_FORMAT(storage as i32)) {
        return Some(format);
    }
    // CopyResource preserves bits within a typeless family. Receive into the raw UNORM
    // member so transfer/alpha interpretation remains explicit, including premultiplied sRGB.
    match DXGI_FORMAT(storage as i32) {
        DXGI_FORMAT_AYUV | DXGI_FORMAT_Y410 | DXGI_FORMAT_Y416 | DXGI_FORMAT_NV12 | DXGI_FORMAT_P010
        | DXGI_FORMAT_P016 | DXGI_FORMAT_YUY2 | DXGI_FORMAT_Y210 | DXGI_FORMAT_Y216 => {
            return Some(PixelFormat::Rgba32Float);
        }
        DXGI_FORMAT_R8G8B8A8_UNORM_SRGB => return Some(PixelFormat::Rgba8Unorm),
        DXGI_FORMAT_B8G8R8A8_UNORM_SRGB => return Some(PixelFormat::Bgra8Unorm),
        DXGI_FORMAT_B8G8R8X8_UNORM_SRGB => return Some(PixelFormat::Bgrx8Unorm),
        DXGI_FORMAT_R10G10B10_XR_BIAS_A2_UNORM => return Some(PixelFormat::Rgb10XrA2Unorm),
        _ => {}
    }
    [
        PixelFormat::Bgra8Unorm,
        PixelFormat::Rgba8Unorm,
        PixelFormat::Rgba16Float,
        PixelFormat::Rgba32Float,
        PixelFormat::Bgrx8Unorm,
        PixelFormat::Rgba8Snorm,
        PixelFormat::Rgba16Unorm,
        PixelFormat::Rgba16Snorm,
        PixelFormat::Rgb10A2Unorm,
        PixelFormat::R8Unorm,
        PixelFormat::R8Snorm,
        PixelFormat::Rg8Unorm,
        PixelFormat::Rg8Snorm,
        PixelFormat::R16Unorm,
        PixelFormat::R16Snorm,
        PixelFormat::Rg16Unorm,
        PixelFormat::Rg16Snorm,
        PixelFormat::R16Float,
        PixelFormat::Rg16Float,
        PixelFormat::R32Float,
        PixelFormat::Rg11B10Float,
        PixelFormat::A8Unorm,
        PixelFormat::Rg32Float,
        PixelFormat::Bgr565Unorm,
        PixelFormat::Bgra5551Unorm,
        PixelFormat::Bgra4444Unorm,
        PixelFormat::Rgba8Uint,
        PixelFormat::Rgba8Sint,
        PixelFormat::Rgba16Uint,
        PixelFormat::Rgba16Sint,
        PixelFormat::Rgba32Uint,
        PixelFormat::Rgba32Sint,
        PixelFormat::Rg8Uint,
        PixelFormat::Rg8Sint,
        PixelFormat::Rg16Uint,
        PixelFormat::Rg16Sint,
        PixelFormat::Rg32Uint,
        PixelFormat::Rg32Sint,
        PixelFormat::R8Uint,
        PixelFormat::R8Sint,
        PixelFormat::R16Uint,
        PixelFormat::R16Sint,
        PixelFormat::R32Uint,
        PixelFormat::R32Sint,
        PixelFormat::Rgb10A2Uint,
    ]
    .into_iter()
    .find(|candidate| format(*candidate).0 as u32 == storage)
}

/// Typeless registrations carry no view interpretation. Keep deterministic visual defaults;
/// a declared caller may choose another supported member of the authenticated family.
pub(super) fn typeless_default(storage: DXGI_FORMAT) -> Option<PixelFormat> {
    Some(match storage {
        DXGI_FORMAT_R32G32B32A32_TYPELESS => PixelFormat::Rgba32Float,
        DXGI_FORMAT_R16G16B16A16_TYPELESS => PixelFormat::Rgba16Float,
        DXGI_FORMAT_R32G32_TYPELESS => PixelFormat::Rg32Float,
        DXGI_FORMAT_R10G10B10A2_TYPELESS => PixelFormat::Rgb10A2Unorm,
        DXGI_FORMAT_R8G8B8A8_TYPELESS => PixelFormat::Rgba8Unorm,
        DXGI_FORMAT_R16G16_TYPELESS => PixelFormat::Rg16Float,
        DXGI_FORMAT_R32_TYPELESS => PixelFormat::R32Float,
        DXGI_FORMAT_R8G8_TYPELESS => PixelFormat::Rg8Unorm,
        DXGI_FORMAT_R16_TYPELESS => PixelFormat::R16Float,
        DXGI_FORMAT_R8_TYPELESS => PixelFormat::R8Unorm,
        DXGI_FORMAT_B8G8R8A8_TYPELESS => PixelFormat::Bgra8Unorm,
        DXGI_FORMAT_B8G8R8X8_TYPELESS => PixelFormat::Bgrx8Unorm,
        _ => return None,
    })
}

pub(super) fn typeless_accepts(storage: DXGI_FORMAT, interpretation: PixelFormat) -> bool {
    typeless_default(storage).is_some() && typeless_format(interpretation) == Some(storage)
}

/// Stable fixed-shader storage tags, not a user setting or transport protocol.
pub(super) fn sample_storage(format: PixelFormat) -> u32 {
    match format {
        PixelFormat::Bgr565Unorm => 1,
        PixelFormat::Bgra5551Unorm => 2,
        PixelFormat::Rgb10XrA2Unorm => 3,
        PixelFormat::Rgba8Uint | PixelFormat::Rg8Uint | PixelFormat::R8Uint => 4,
        PixelFormat::Rgba16Uint | PixelFormat::Rg16Uint | PixelFormat::R16Uint => 5,
        PixelFormat::Rgba32Uint | PixelFormat::Rg32Uint | PixelFormat::R32Uint => 6,
        PixelFormat::Rgba8Sint | PixelFormat::Rg8Sint | PixelFormat::R8Sint => 7,
        PixelFormat::Rgba16Sint | PixelFormat::Rg16Sint | PixelFormat::R16Sint => 8,
        PixelFormat::Rgba32Sint | PixelFormat::Rg32Sint | PixelFormat::R32Sint => 9,
        PixelFormat::Rgb10A2Uint => 10,
        _ => 0,
    }
}

pub(super) fn integer_slot(format: PixelFormat) -> Option<u32> {
    match sample_storage(format) {
        4..=6 | 10 => Some(3),
        7..=9 => Some(4),
        _ => None,
    }
}

fn missing_output() -> Error {
    Error::new(HRESULT(0x8000_4005_u32 as i32), "D3D11 returned no texture or view")
}
