use super::*;

#[test]
#[ignore = "explicit native paired 4:2:2 luma/chroma/precision/storage qualification on both GPUs"]
fn hardware_paired_video_decodes_each_luma_pair_and_retires_storage() {
    let done = super::super::formats::support::watchdog("paired video qualification");
    let hardware = adapters()
        .unwrap()
        .into_iter()
        .filter(|adapter| !adapter.software)
        .collect::<Vec<_>>();
    assert!(!hardware.is_empty());
    for adapter in hardware {
        for native in [107, 108, 109] {
            for mode in 0..5 {
                let scale = match native {
                    107 => 1,
                    108 => 4,
                    _ => 256,
                };
                let maximum = match native {
                    107 => 255,
                    108 => 1023,
                    _ => 65535,
                };
                let low = u32::from(native == 109);
                let pairs = [
                    [32 * scale, 128 * scale, 96 * scale, 128 * scale],
                    [126 * scale, 96 * scale, 200 * scale, 160 * scale],
                    [0, 128 * scale, maximum, 128 * scale],
                    [
                        100 * scale + 13 * low,
                        100 * scale + 123 * low,
                        220 * scale + 17 * low,
                        150 * scale + 47 * low,
                    ],
                ];
                let bytes = pairs
                    .iter()
                    .flat_map(|pair| {
                        pair.iter()
                            .enumerate()
                            .flat_map(|(component, &code)| {
                                if native == 107 {
                                    vec![code as u8]
                                } else {
                                    // A declared opaque case also proves ignored low Y210 padding bits.
                                    let word = if native == 108 {
                                        (code << 6) | if mode == 4 { [1, 63, 17, 42][component] } else { 0 }
                                    } else {
                                        code
                                    };
                                    (word as u16).to_le_bytes().to_vec()
                                }
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                let linear = pairs
                    .iter()
                    .flat_map(|&[y0, u, y1, v]| [reference(y0, u, v, 1, scale, 1), reference(y1, u, v, 1, scale, 1)])
                    .collect::<Vec<_>>();
                super::subsampled::verify(
                    &adapter,
                    native,
                    mode,
                    Extent2D::new(4, 2).unwrap(),
                    &bytes,
                    &linear,
                    "PAIRED_VIDEO_IMPORT",
                );
            }
        }
    }
    done.send(()).unwrap();
}
