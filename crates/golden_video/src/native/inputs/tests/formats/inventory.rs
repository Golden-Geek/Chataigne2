use crate::native::{adapters, device::Device};
use std::collections::BTreeMap;
use windows::Win32::Graphics::{
    Direct3D11::*,
    Dxgi::{Common::*, IDXGIResource},
};
use windows::core::Interface;

#[test]
#[ignore = "explicit native allocation/capability inventory; no desktop input or pixel claim"]
fn hardware_spout_backend_format_inventory() {
    let done = super::support::watchdog("native format allocation inventory");
    let hardware = adapters()
        .unwrap()
        .into_iter()
        .filter(|adapter| !adapter.software)
        .collect::<Vec<_>>();
    assert!(!hardware.is_empty());
    for adapter in hardware {
        let device = Device::new(adapter.id).unwrap();
        let peer = Device::new(adapter.id).unwrap();
        let mut available = Vec::new();
        let mut rejected = BTreeMap::<i32, Vec<i32>>::new();
        // Current Windows DXGI enum through A4B4G4R4_UNORM. Reserved ordinals return native
        // errors. UNKNOWN/FORCE_UINT are not image formats. Legacy metadata is a separate namespace.
        for ordinal in 1..=191 {
            let format = DXGI_FORMAT(ordinal);
            let support = unsafe { device.device.CheckFormatSupport(format) }.ok();
            let shared = texture(&device, format, D3D11_RESOURCE_MISC_SHARED.0 as u32);
            let shared = match shared {
                Ok(texture) => texture,
                Err(error) => {
                    rejected.entry(error.code().0).or_default().push(ordinal);
                    continue;
                }
            };
            let mut alias = None;
            let opened = unsafe { shared.cast::<IDXGIResource>().unwrap().GetSharedHandle() }
                .and_then(|handle| unsafe { peer.device.OpenSharedResource::<ID3D11Texture2D>(handle, &mut alias) });
            if let Err(error) = opened {
                rejected.entry(error.code().0).or_default().push(ordinal);
                continue;
            }
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            unsafe { alias.unwrap().GetDesc(&mut desc) };
            assert_eq!(desc.Format, format);
            assert_eq!((desc.Width, desc.Height), (8, 8));
            let bridge = texture(
                &device,
                format,
                (D3D11_RESOURCE_MISC_SHARED_NTHANDLE | D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX).0 as u32,
            );
            available.push(serde_json::json!({
                "dxgi_format":ordinal, "format_support":support, "sdk_shared_allocation":true,
                "foreign_alias_exact":true, "native_bridge_allocation":bridge.is_ok(),
                "bridge_hresult":bridge.err().map(|error| error.code().0),
                "current_native_mapping":crate::native::texture::source_format(ordinal as u32).is_some(),
            }));
        }
        for known in [2, 10, 11, 13, 24, 28, 29, 31, 87, 88, 91, 93] {
            assert!(available.iter().any(|record| record["dxgi_format"] == known));
        }
        unsafe { device.device.GetDeviceRemovedReason().unwrap() };
        eprintln!(
            "SPOUT_FORMAT_INVENTORY {}",
            serde_json::json!({
                "adapter": format!("{:?}",adapter.id), "description":adapter.description,
                "native_ordinals":[1,191], "extent":[8,8], "available":available, "rejected_by_hresult":rejected,
                "scope":"Exact SDK shared allocator flags plus same-adapter foreign opening; NT bridge creation. No pixel, typed-view interpretation or complete import claim. SDK legacy0/21/22 normalization is separate.",
            })
        );
    }
    done.send(()).unwrap();
}

pub(in crate::native::inputs::tests) fn texture(
    device: &Device,
    format: DXGI_FORMAT,
    flags: u32,
) -> windows::core::Result<ID3D11Texture2D> {
    let mut texture = None;
    // Same descriptor used by the pinned SDK's CreateSharedDX11Texture; no GPU commands.
    unsafe {
        device.device.CreateTexture2D(
            &D3D11_TEXTURE2D_DESC {
                Width: 8,
                Height: 8,
                MipLevels: 1,
                ArraySize: 1,
                Format: format,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: (D3D11_BIND_SHADER_RESOURCE | D3D11_BIND_RENDER_TARGET).0 as u32,
                MiscFlags: flags,
                ..Default::default()
            },
            None,
            Some(&mut texture),
        )?;
    }
    Ok(texture.unwrap())
}
