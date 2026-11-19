use super::*;
use std::io::Write;

fn append_frame(chunks: &mut Vec<u8>, delay: u32) {
    chunks.extend_from_slice(b"ANMF");
    chunks.extend_from_slice(&16u32.to_le_bytes());
    let mut frame = [0u8; 16];
    frame[12..15].copy_from_slice(&delay.to_le_bytes()[..3]);
    chunks.extend_from_slice(&frame);
}

fn file(chunks: &[u8]) -> tempfile::NamedTempFile {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(b"RIFF").unwrap();
    file.write_all(&(chunks.len() as u32 + 4).to_le_bytes()).unwrap();
    file.write_all(b"WEBP").unwrap();
    file.write_all(chunks).unwrap();
    file.flush().unwrap();
    file
}

#[test]
fn animated_duration_matches_demux_delays_and_skips_padded_metadata() {
    let mut chunks = b"JUNK\x03\0\0\0abc\0".to_vec();
    for delay in [240, 5, 560, 400] {
        append_frame(&mut chunks, delay);
    }
    let file = file(&chunks);
    assert_eq!(inspect(file.path(), || false).unwrap().duration_ms, 1300);
    assert!(matches!(
        inspect(file.path(), || true),
        Err(DecodeError::Ffmpeg(ffmpeg::Error::Exit))
    ));
}

#[test]
fn chunk_bounds_and_empty_animations_fail_before_publishing_timing() {
    for chunks in [b"ANMF\x20\0\0\0".as_slice(), b"JUNK\x03\0\0\0abc\0"] {
        let file = file(chunks);
        assert!(matches!(
            inspect(file.path(), || false),
            Err(DecodeError::Ffmpeg(ffmpeg::Error::InvalidData))
        ));
    }
}

fn overwrite_packet(x: u8, size: u32, timestamp: i64) -> ffmpeg::Packet {
    let mut bytes = vec![x, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 40, 0, 0, 2];
    bytes.extend_from_slice(b"VP8 ");
    bytes.extend_from_slice(&size.to_le_bytes());
    bytes.extend_from_slice(&[0, 0, 0, 0x9d, 0x01, 0x2a, 2, 0, 2, 0]);
    bytes.resize(24 + size as usize, 0);
    let mut packet = ffmpeg::Packet::copy(&bytes);
    packet.set_pts(Some(timestamp));
    packet.set_duration(40);
    packet
}

#[test]
fn packet_lookahead_stays_bounded_and_keeps_non_omitted_updates_in_order() {
    let mut animation = Animation {
        path: PathBuf::new(),
        interrupt: Arc::new(Mutex::new(|| false)),
        duration_ms: 160,
        lookahead: VecDeque::new(),
        independent_patches: false,
        parallel: None,
        started: false,
    };
    let mut packets = [
        overwrite_packet(0, 16, 0),
        overwrite_packet(1, 16, 40),
        overwrite_packet(2, 76, 80),
        overwrite_packet(3, 16, 120),
    ]
    .into_iter();
    for (timestamp, remaining) in [(0, 1), (40, 1), (80, 0), (120, 0)] {
        let (packet, omitted) = animation
            .next_packet(Some(160), (8, 8), 8, 110, || Ok(packets.next()))
            .unwrap()
            .unwrap();
        assert_eq!(packet.pts(), Some(timestamp));
        assert_eq!(omitted, None);
        assert_eq!(packets.len(), remaining);
        assert!(animation.lookahead.iter().map(ffmpeg::Packet::size).sum::<usize>() <= 220);
    }
}
