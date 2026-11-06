use super::*;
use windows::Win32::Graphics::Direct3D11::D3D11_RESOURCE_MISC_SHARED;

#[test]
#[ignore = "explicit native planar 4:2:0 precision/plane/storage qualification on every available GPU path"]
fn hardware_planar_video_decodes_distinct_luma_and_chroma_planes_and_retires_storage() {
    let done = super::super::formats::support::watchdog("planar video qualification");
    let hardware = adapters()
        .unwrap()
        .into_iter()
        .filter(|adapter| !adapter.software)
        .collect::<Vec<_>>();
    assert!(!hardware.is_empty());
    for adapter in hardware {
        for native in [103, 104, 105] {
            if native == 105 {
                // Recheck only this known adapter-dependent producer prerequisite, not the inventory.
                let device = Device::new(adapter.id).unwrap();
                if let Err(error) = super::super::formats::inventory::texture(
                    &device,
                    DXGI_FORMAT(native),
                    D3D11_RESOURCE_MISC_SHARED.0 as u32,
                ) {
                    assert_eq!(error.code().0, 0x8007_0057_u32 as i32);
                    println!(
                        "PLANAR_VIDEO_UNAVAILABLE {}",
                        serde_json::json!({
                            "adapter":format!("{:?}", adapter.id), "description":adapter.description,
                            "native_format":native, "hresult":error.code().0,
                            "scope":"SDK shared allocation unavailable; native reception remains unaccepted",
                        })
                    );
                    continue;
                }
            }
            let scale = match native {
                103 => 1,
                104 => 4,
                _ => 256,
            };
            let maximum = match native {
                103 => 255,
                104 => 1023,
                _ => 65535,
            };
            let luma = [32, 80, 126, 200, 96, 160, 64, 220, 0, 235, 100, 150, 55, 190, 210, 255]
                .into_iter()
                .enumerate()
                .map(|(i, value)| {
                    if i == 15 {
                        maximum
                    } else {
                        value * scale + if native == 105 { 13 + (i as u32 * 7) % 64 } else { 0 }
                    }
                })
                .collect::<Vec<_>>();
            let chroma = [[128, 128], [96, 160], [128, 128], [100, 150]]
                .into_iter()
                .enumerate()
                .map(|(i, [u, v])| {
                    [
                        u * scale + if native == 105 && i == 3 { 123 } else { 0 },
                        v * scale + if native == 105 && i == 3 { 47 } else { 0 },
                    ]
                })
                .collect::<Vec<_>>();
            let linear = (0..16)
                .map(|i| {
                    let [u, v] = chroma[(i / 8) * 2 + (i % 4) / 2];
                    reference(luma[i], u, v, 1, scale, 1)
                })
                .collect::<Vec<_>>();
            for mode in 0..5 {
                let codes = luma.iter().copied().chain(chroma.iter().flatten().copied());
                let bytes = codes
                    .enumerate()
                    .flat_map(|(i, code)| {
                        if native == 103 {
                            vec![code as u8]
                        } else {
                            let word = if native == 104 {
                                (code << 6) | if mode == 4 { [1, 63, 17, 42][i % 4] } else { 0 }
                            } else {
                                code
                            };
                            (word as u16).to_le_bytes().to_vec()
                        }
                    })
                    .collect::<Vec<_>>();
                super::subsampled::verify(
                    &adapter,
                    native as u32,
                    mode,
                    Extent2D::new(4, 4).unwrap(),
                    &bytes,
                    &linear,
                    "PLANAR_VIDEO_IMPORT",
                );
            }
        }
    }
    done.send(()).unwrap();
}
