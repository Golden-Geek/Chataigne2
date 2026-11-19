use super::*;
use golden_video::decode::{DecodeLimits, SoftwareDecodeThreads};
use std::io::Write;

fn fixture() -> Vec<u8> {
    // Six VP8 frames over a 4×4 canvas. Partial updates change the alpha and color
    // in the top left. The opaque remainder still depends on the earlier canvas.
    let encoded = concat!(
        "524946466402000057454250565038580a00000012000000030000030000414e494d0600000000000000",
        "0000414e4d467000000000000000000003000003000028000002414c504811000000000808ffff0808ff",
        "ffffffffffffffffff00565038203e0000005001009d012a0400040001001c25b00004740000fecbb7ff",
        "ff4afe217faffbf7f99ff3bb9c7ffeb80fffe701fccb7ffe665f9fecb57257fd03f658940000414e4d46",
        "5400000000000000000001000001000037000002414c504805000000003131313100565038202e000000",
        "3001009d012a0200020001001c25a000037000fee0b97fffb17d6dfb47673fffec66fffe8cdfffd19bfa",
        "ca000000414e4d464e0000000000000000000100000100005a000002414c504805000000005a5a5a5a00",
        "56503820280000003001009d012a0200020001001c25a000037000fede127fd554f5fad6ffff9b03ff6f",
        "3ff047968000414e4d464a00000000000000000001000001000028000002414c50480500000000838383",
        "830056503820240000003001009d012a0200020001001c2500006e0000fede127b64ef4bfffc6ff56be4",
        "c8700000414e4d466400000000000000000003000003000055000002414c50481100000000acacffffac",
        "acffffffffffffffffffff0056503820320000005001009d012a0400040001001c25b00004740000fecb",
        "b7ffff57760dd3fbd5ffffce03fff9c07f32dffebef7d5e54e640000414e4d4650000000000000000000",
        "01000001000046000002414c50480500000000d5d5d5d500565038202a0000005001009d012a02000200",
        "01001c25980004618000feda26ffffc6f1fe4bbfb77e3dfffa89f2b3ddf34000",
    );
    encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn file(bytes: &[u8]) -> tempfile::NamedTempFile {
    let mut file = tempfile::Builder::new().suffix(".webp").tempfile().unwrap();
    file.write_all(bytes).unwrap();
    file.flush().unwrap();
    file
}

fn open(file: &tempfile::NamedTempFile) -> Reader {
    Reader::new(
        Decoder::open(
            file.path(),
            DecodeLimits {
                max_pixels: 4096,
                max_cpu_frame_bytes: 16 * 1024 * 1024,
                max_packet_bytes: 1024 * 1024,
                max_packets_per_frame: 256,
                software_threads: SoftwareDecodeThreads::One,
            },
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn forward_omission_preserves_exact_canvas_neighbours_and_held_seeks() {
    let fixture = file(&fixture());
    let ms = Duration::from_millis;
    let mut exact = open(&fixture);
    let expected_95 = exact.poll(ms(95), None).unwrap().unwrap();
    let expected_185 = exact.poll(ms(185), None).unwrap().unwrap();

    let mut playing = open(&fixture);
    assert!(playing.advance_for_playback(ms(225), Some(ms(0))).unwrap());
    let following = playing.next.as_ref().unwrap();
    assert_eq!(following.timestamp(), 185);
    assert_eq!(following.omitted_before(), Some((40, 95)));
    assert_eq!(playing.selected_neighbours().unwrap().unwrap().next, Some(ms(40)));
    playing.mark_published();
    assert!(playing.advance_for_playback(ms(225), Some(ms(0))).unwrap());
    let frame = playing.selected_neighbours().unwrap().unwrap();
    assert_eq!(
        (frame.previous, frame.position, frame.next),
        (Some(ms(95)), ms(185), Some(ms(225)))
    );
    assert_eq!(playing.software_image().unwrap().pixels(), expected_185.pixels());

    let mut paused = open(&fixture);
    assert!(paused.advance_for_playback(ms(225), Some(ms(0))).unwrap());
    paused.mark_published();
    assert_eq!(
        paused.poll(ms(120), None).unwrap().unwrap().pixels(),
        expected_95.pixels()
    );
    assert_eq!(paused.neighbours().unwrap().unwrap().position, ms(95));
}

#[test]
fn updates_with_blending_or_disposal_are_always_decoded() {
    let ms = Duration::from_millis;
    for flags in [0, 3] {
        let mut bytes = fixture();
        bytes[164 + 8 + 15] = flags;
        let fixture = file(&bytes);
        let mut reader = open(&fixture);
        assert!(reader.advance_for_playback(ms(225), Some(ms(0))).unwrap());
        let following = reader.next.as_ref().unwrap();
        assert_eq!(following.timestamp(), 40);
        assert_eq!(following.omitted_before(), None);
    }
}
