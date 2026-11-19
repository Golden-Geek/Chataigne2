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
    assert_eq!(duration(file.path(), || false).unwrap(), 1300);
    assert!(matches!(
        duration(file.path(), || true),
        Err(DecodeError::Ffmpeg(ffmpeg::Error::Exit))
    ));
}

#[test]
fn chunk_bounds_and_empty_animations_fail_before_publishing_timing() {
    for chunks in [b"ANMF\x20\0\0\0".as_slice(), b"JUNK\x03\0\0\0abc\0"] {
        let file = file(chunks);
        assert!(matches!(
            duration(file.path(), || false),
            Err(DecodeError::Ffmpeg(ffmpeg::Error::InvalidData))
        ));
    }
}
