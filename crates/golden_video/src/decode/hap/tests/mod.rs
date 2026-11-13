use super::*;
mod fixtures;
mod storage;

fn decode(packet: &[u8], extent: Extent2D) -> Result<HapFrame, DecodeError> {
    HapFrame::decode(packet, extent, &ImageStorageBudget::new(64 * 1024 * 1024).unwrap())
}

fn section(kind: u8, data: &[u8]) -> Vec<u8> {
    let length = data.len() as u32;
    let mut result = length.to_le_bytes()[..3].to_vec();
    result.push(kind);
    result.extend_from_slice(data);
    result
}
#[test]
fn raw_snappy_and_odd_block_extents_preserve_compressed_bytes_and_formats() {
    for (kind, format, extent) in [
        (0xAB, HapFormat::Bc1Rgb, Extent2D::new(4, 4).unwrap()),
        (0xAE, HapFormat::Bc3Rgba, Extent2D::new(17, 9).unwrap()),
        (0xAF, HapFormat::Bc3YCoCg, Extent2D::new(1920, 1080).unwrap()),
    ] {
        let expected = vec![87; format.byte_len(extent).unwrap()];
        let raw = decode(&section(kind, &expected), extent).unwrap();
        assert_eq!(raw.format(), format);
        assert_eq!(raw.extent(), extent);
        assert_eq!(raw.blocks(), expected);
        let compressed = snap::raw::Encoder::new().compress_vec(&expected).unwrap();
        let decoded = decode(&section(kind + 0x10, &compressed), extent).unwrap();
        assert_eq!(decoded.blocks(), expected);
    }
}
fn complex(size: u32, offset: u32, data: &[u8]) -> Vec<u8> {
    let mut instructions = section(2, &[0xA]);
    instructions.extend(section(3, &size.to_le_bytes()));
    instructions.extend(section(4, &offset.to_le_bytes()));
    let mut body = section(1, &instructions);
    body.extend_from_slice(data);
    section(0xCB, &body)
}
#[test]
fn malformed_headers_offsets_sizes_and_expansion_are_rejected_before_out_of_bounds_access() {
    let extent = Extent2D::new(4, 4).unwrap();
    assert_eq!(decode(&complex(8, 0, &[12; 8]), extent).unwrap().blocks(), &[12; 8]);
    for packet in [
        vec![],
        vec![1, 0, 0],
        vec![0, 0, 0, 0xAB],
        vec![0, 0, 0, 0xCB, 255, 255, 255, 255],
        section(0xAB, &[0; 7]),
        complex(8, u32::MAX, &[0; 8]),
        complex(u32::MAX, 0, &[0; 8]),
        complex(9, 0, &[0; 9]),
        section(0xBB, &snap::raw::Encoder::new().compress_vec(&[0; 1024]).unwrap()),
    ] {
        assert!(decode(&packet, extent).is_err());
    }
    assert!(matches!(
        decode(&section(0xAC, &[0; 16]), extent),
        Err(DecodeError::UnsupportedCodec)
    ));
}

#[test]
fn malformed_chunk_tables_and_multi_image_ranges_cannot_escape_their_section() {
    let extent = Extent2D::new(4, 4).unwrap();
    for (kind, table) in [(3, vec![0; 3]), (4, vec![0; 3]), (2, vec![])] {
        let mut instructions = section(2, &[0xA]);
        instructions.extend(section(3, &8u32.to_le_bytes()));
        instructions.extend(section(kind, &table));
        let mut body = section(1, &instructions);
        body.extend_from_slice(&[0; 8]);
        assert!(decode(&section(0xCB, &body), extent).is_err());
    }
    let single = section(0xAB, &[0; 8]);
    let two = section(0x0D, &single.repeat(2));
    assert!(matches!(decode(&two, extent), Err(DecodeError::UnsupportedCodec)));
    assert!(decode(&section(0x0D, &single.repeat(3)), extent).is_err());
}

#[test]
fn excessive_chunk_work_is_rejected_before_reference_allocation() {
    let mut instructions = section(2, &vec![0xA; 1025]);
    instructions.extend(section(3, &vec![0; 1025 * 4]));
    let body = section(1, &instructions);
    assert!(matches!(
        decode(&section(0xCB, &body), Extent2D::new(4, 4).unwrap()),
        Err(DecodeError::LimitExceeded)
    ));
}
