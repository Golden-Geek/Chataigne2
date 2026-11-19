//! Skip obsolete VP8 updates only when a due overwrite makes their full canvas effect redundant.
use super::{Animation, DecodeError, ffmpeg};

const MAX_OMITTED: usize = 4;
pub(in crate::decode) type PacketWithOmission = (ffmpeg::Packet, Option<(i64, i64)>);

impl Animation {
    pub(in crate::decode) fn next_packet(
        &mut self,
        due: Option<i64>,
        canvas: (u32, u32),
        extra_packets: usize,
        mut read: impl FnMut() -> Result<Option<ffmpeg::Packet>, DecodeError>,
    ) -> Result<Option<PacketWithOmission>, DecodeError> {
        let Some(mut packet) = self
            .lookahead
            .take()
            .map_or_else(&mut read, |packet| Ok(Some(packet)))?
        else {
            return Ok(None);
        };
        let mut omitted = None;
        for _ in 0..MAX_OMITTED.min(extra_packets) {
            let Some((due, start, rect)) = due.zip(packet.pts()).and_then(|(due, start)| {
                if start >= due || packet.is_key() {
                    return None;
                }
                overwrite(&packet, canvas).map(|rect| (due, start, rect))
            }) else {
                break;
            };
            let Some(next) = read()? else {
                break;
            };
            if next.pts().is_some_and(|pts| {
                packet.duration() > 0
                    && start.checked_add(packet.duration()) == Some(pts)
                    && next.duration() > 0
                    && pts.checked_add(next.duration()).is_some_and(|end| end <= due)
            }) && overwrite(&next, canvas).is_some_and(|replacement| replacement.covers(rect))
            {
                let (first, _) = omitted.unwrap_or((start, start));
                omitted = Some((first, start));
                packet = next;
            } else {
                self.lookahead = Some(next);
                break;
            }
        }
        Ok(Some((packet, omitted)))
    }
}

#[derive(Clone, Copy)]
struct Rect {
    x: u32,
    y: u32,
    right: u32,
    bottom: u32,
}

impl Rect {
    fn covers(self, other: Self) -> bool {
        self.x <= other.x && self.y <= other.y && self.right >= other.right && self.bottom >= other.bottom
    }
}

fn overwrite(packet: &ffmpeg::Packet, (canvas_width, canvas_height): (u32, u32)) -> Option<Rect> {
    if packet.is_corrupt() {
        return None;
    }
    let (header, mut chunks) = packet.data()?.split_first_chunk::<16>()?;
    // RFC 9649 ANMF flags: replace the rectangle and leave it in place afterward.
    // Lossless updates may change the canvas representation; always decode those.
    if header[15] != 2 {
        return None;
    }
    let field = |index| u32::from_le_bytes([header[index], header[index + 1], header[index + 2], 0]);
    let (x, y, width, height) = (2 * field(0), 2 * field(3), 1 + field(6), 1 + field(9));
    let (right, bottom) = (x + width, y + height);
    if right > canvas_width || bottom > canvas_height {
        return None;
    }
    let (mut tag, mut bytes, mut rest) = chunk(chunks)?;
    if tag == b"ALPH" && bytes.len() > 1 && bytes[0] & 0b11 <= 1 {
        chunks = rest;
        (tag, bytes, rest) = chunk(chunks)?;
    }
    let (frame, _) = bytes.split_first_chunk::<10>()?;
    if tag != b"VP8 " || !rest.is_empty() || frame[0] & 1 != 0 || frame[3..6] != [0x9d, 0x01, 0x2a] {
        return None;
    }
    let vp8_width = u32::from(u16::from_le_bytes([frame[6], frame[7]]) & 0x3fff);
    let vp8_height = u32::from(u16::from_le_bytes([frame[8], frame[9]]) & 0x3fff);
    (width == vp8_width && height == vp8_height).then_some(Rect { x, y, right, bottom })
}

fn chunk(data: &[u8]) -> Option<(&[u8; 4], &[u8], &[u8])> {
    let (tag, data) = data.split_first_chunk::<4>()?;
    let (len, data) = data.split_first_chunk::<4>()?;
    let len = usize::try_from(u32::from_le_bytes(*len)).ok()?;
    let padded = len.checked_add(len % 2)?;
    let (payload, rest) = data.split_at_checked(padded)?;
    Some((tag, &payload[..len], rest))
}
