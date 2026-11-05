//! One worker-owned indexed payload; its existing fixed byte charge includes all storage.
use super::PatternParameters;
use windows::Win32::Graphics::{
    Direct3D::D3D11_SRV_DIMENSION_BUFFEREX, Direct3D11::*, Dxgi::Common::DXGI_FORMAT_R32_TYPELESS,
};
use windows::core::Result;

pub(in crate::native::pipeline) struct PatternBuffer {
    buffer: ID3D11Buffer,
    view: ID3D11ShaderResourceView,
}

impl PatternBuffer {
    pub(in crate::native::pipeline) fn new(device: &ID3D11Device) -> Result<Self> {
        let bytes = size_of::<PatternParameters>() as u32;
        let mut buffer = None;
        let mut view = None;
        // SAFETY: fixed four-byte-aligned read-only shader payload. Both objects are
        // prepared on this device before submission and retained by its local Pipeline.
        unsafe {
            device.CreateBuffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: bytes,
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                    MiscFlags: D3D11_RESOURCE_MISC_BUFFER_ALLOW_RAW_VIEWS.0 as u32,
                    ..Default::default()
                },
                None,
                Some(&mut buffer),
            )?;
            let buffer = buffer.ok_or_else(super::super::missing_output)?;
            device.CreateShaderResourceView(
                &buffer,
                Some(&D3D11_SHADER_RESOURCE_VIEW_DESC {
                    Format: DXGI_FORMAT_R32_TYPELESS,
                    ViewDimension: D3D11_SRV_DIMENSION_BUFFEREX,
                    Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                        BufferEx: D3D11_BUFFEREX_SRV {
                            FirstElement: 0,
                            NumElements: bytes / 4,
                            Flags: D3D11_BUFFEREX_SRV_FLAG_RAW.0 as u32,
                        },
                    },
                }),
                Some(&mut view),
            )?;
            Ok(Self {
                buffer,
                view: view.ok_or_else(super::super::missing_output)?,
            })
        }
    }

    pub(in crate::native::pipeline) fn bind(&self, context: &ID3D11DeviceContext, data: &PatternParameters) {
        // SAFETY: complete fixed-size payload, synchronous CPU copy into prepared storage;
        // slot 2 is reserved for this read-only view and cleared after the draw.
        unsafe {
            context.UpdateSubresource(&self.buffer, 0, None, std::ptr::from_ref(data).cast(), 0, 0);
            context.PSSetShaderResources(2, Some(&[Some(self.view.clone())]));
        }
    }
}
