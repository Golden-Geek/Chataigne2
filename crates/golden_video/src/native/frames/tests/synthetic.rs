use super::super::{GpuFrameBudget, GpuFramePublisher, GpuImage, producer::Input};
use crate::{
    Extent2D,
    decode::D3d11Storage,
    native::{AdapterId, device::Device},
};
use std::time::{Duration, Instant};
use windows::Win32::Graphics::{Direct3D11::*, Dxgi::Common::*};

pub(in crate::native) fn prepared(
    adapter: AdapterId,
    storage: D3d11Storage,
    flags: u32,
    y: u16,
    budget: &GpuFrameBudget,
) -> (GpuFramePublisher, ID3D11Texture2D) {
    let device = Device::new(adapter).unwrap();
    let p010 = storage == D3d11Storage::P010;
    let unit = if p010 { 2 } else { 1 };
    let mut bytes = Vec::new();
    for value in std::iter::repeat_n(y, 64 * 48).chain(std::iter::repeat_n(if p010 { 512 } else { 128 }, 64 * 24)) {
        if p010 {
            bytes.extend_from_slice(&(value << 6).to_le_bytes());
        } else {
            bytes.push(value as u8);
        }
    }
    let mut texture = None;
    unsafe {
        device
            .device
            .CreateTexture2D(
                &D3D11_TEXTURE2D_DESC {
                    Width: 64,
                    Height: 48,
                    ArraySize: 1,
                    MipLevels: 1,
                    Format: if p010 { DXGI_FORMAT_P010 } else { DXGI_FORMAT_NV12 },
                    SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                    Usage: D3D11_USAGE_DEFAULT,
                    ..Default::default()
                },
                Some(&D3D11_SUBRESOURCE_DATA {
                    pSysMem: bytes.as_ptr().cast(),
                    SysMemPitch: 64 * unit,
                    SysMemSlicePitch: 64 * 72 * unit,
                }),
                Some(&mut texture),
            )
            .unwrap()
    };
    let publisher =
        GpuFramePublisher::prepare(device, Extent2D::new(64, 48).unwrap(), storage, flags, budget, None).unwrap();
    (publisher, texture.unwrap())
}
pub(in crate::native) fn submit(publisher: &mut GpuFramePublisher, texture: &ID3D11Texture2D) -> bool {
    publisher
        .submit_owned(
            texture,
            0,
            Input::Texture {
                _texture: texture.clone(),
            },
        )
        .unwrap()
}
pub(in crate::native) fn completed(publisher: &mut GpuFramePublisher) -> GpuImage {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(image) = publisher.poll().unwrap() {
            return image;
        }
        assert!(Instant::now() < deadline, "GPU frame conversion completion timeout");
        std::thread::sleep(Duration::from_millis(1));
    }
}
