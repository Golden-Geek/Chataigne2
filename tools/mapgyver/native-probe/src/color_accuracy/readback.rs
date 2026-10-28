//! One bounded full-frame diagnostic copy/map, deliberately absent from the render path.

use crate::{ProbeResult, d3d::Gpu, texture::Texture};
use mapgyver_native_probe::allocation::Work;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;

pub fn image(gpu: &Gpu, texture: &ID3D11Texture2D, extent: [u32; 2]) -> ProbeResult<Vec<[u8; 4]>> {
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    // SAFETY: owned texture and writable descriptor.
    unsafe {
        texture.GetDesc(&mut desc);
    }
    if [desc.Width, desc.Height] != extent
        || desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM
        || !matches!(extent, [1920, 1080] | [3840, 2160])
    {
        return Err("unbounded or incompatible accuracy readback".into());
    }
    desc.Usage = D3D11_USAGE_STAGING;
    desc.BindFlags = 0;
    desc.MiscFlags = 0;
    desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
    let mut staging = None;
    // SAFETY: validated single-sample BGRA descriptor; owned output.
    unsafe {
        gpu.device.CreateTexture2D(&desc, None, Some(&mut staging))?;
    }
    let staging = Texture::new(
        staging.ok_or("no accuracy staging texture")?,
        &gpu.allocations,
        mapgyver_native_probe::allocation::Owner::Application,
    )?;
    let count = u64::from(extent[0]) * u64::from(extent[1]);
    gpu.allocations.work(Work::GpuCopy(count * 4));
    gpu.allocations.work(Work::Readback(count));
    // SAFETY: matching owned descriptors/devices; completion precedes CPU mapping.
    unsafe {
        gpu.context.CopyResource(&*staging, texture);
    }
    gpu.wait_submitted()?;
    let mut pixels = vec![[0; 4]; count as usize];
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    // SAFETY: copy completed. Respect padded RowPitch; never copy padding into the oracle.
    unsafe {
        gpu.context.Map(&*staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
        if mapped.pData.is_null() || mapped.RowPitch < extent[0] * 4 {
            gpu.context.Unmap(&*staging, 0);
            return Err("invalid full-image diagnostic map".into());
        }
        for y in 0..extent[1] as usize {
            std::ptr::copy_nonoverlapping(
                mapped.pData.cast::<u8>().add(y * mapped.RowPitch as usize),
                pixels.as_mut_ptr().cast::<u8>().add(y * extent[0] as usize * 4),
                extent[0] as usize * 4,
            );
        }
        gpu.context.Unmap(&*staging, 0);
    }
    Ok(pixels)
}
