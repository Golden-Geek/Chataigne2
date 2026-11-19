//! Skip obsolete VP8 updates only when a due overwrite makes their full canvas effect redundant.
use super::{Animation, DecodeError, ffmpeg};

const MAX_OMITTED: usize = 8;
pub(super) const MAX_PATCH_BYTES: usize = 2 * 1024 * 1024;
pub(in crate::decode) type PacketWithOmission = (ffmpeg::Packet, Option<(i64, i64)>);

impl Animation {
    pub(in crate::decode) fn next_packet(
        &mut self,
        due: Option<i64>,
        canvas: (u32, u32),
        extra_packets: usize,
        max_packet_bytes: usize,
        mut read: impl FnMut() -> Result<Option<ffmpeg::Packet>, DecodeError>,
    ) -> Result<Option<PacketWithOmission>, DecodeError> {
        self.started = true;
        if self.lookahead.is_empty() {
            let Some(packet) = read()? else {
                return Ok(None);
            };
            self.lookahead.push_back(packet);
        }
        let mut selected = 0;
        if let Some(due) = due
            && let Some((mut end, mut changed)) = expired_overwrite(&self.lookahead[0], due, canvas)
        {
            let mut queued_bytes = self
                .lookahead
                .iter()
                .fold(0usize, |total, packet| total.saturating_add(packet.size()));
            for index in 1..=MAX_OMITTED.min(extra_packets) {
                if index == self.lookahead.len() {
                    // One read may add up to max_packet_bytes before we know its size.
                    // This keeps the whole queue within two packet-limit units.
                    if queued_bytes > max_packet_bytes {
                        break;
                    }
                    let Some(packet) = read()? else {
                        break;
                    };
                    queued_bytes = queued_bytes.saturating_add(packet.size());
                    self.lookahead.push_back(packet);
                }
                let next = &self.lookahead[index];
                if next.pts() != Some(end) {
                    break;
                }
                let Some((next_end, rect)) = expired_overwrite(next, due, canvas) else {
                    break;
                };
                if rect.covers(changed) {
                    selected = index;
                }
                changed = changed.union(rect);
                end = next_end;
            }
        }
        let omitted = if selected == 0 {
            None
        } else {
            let first = self.lookahead[0].pts().unwrap();
            let last = self.lookahead[selected - 1].pts().unwrap();
            self.lookahead.drain(..selected);
            Some((first, last))
        };
        Ok(self.lookahead.pop_front().map(|packet| (packet, omitted)))
    }
}

#[derive(Clone, Copy)]
pub(super) struct Rect {
    pub(super) x: u32,
    pub(super) y: u32,
    right: u32,
    bottom: u32,
}

impl Rect {
    pub(super) fn extent(self) -> (u32, u32) {
        (self.right - self.x, self.bottom - self.y)
    }

    pub(super) fn is_full(self, canvas: (u32, u32)) -> bool {
        (self.x, self.y) == (0, 0) && self.extent() == canvas
    }

    fn covers(self, other: Self) -> bool {
        self.x <= other.x && self.y <= other.y && self.right >= other.right && self.bottom >= other.bottom
    }

    fn union(self, other: Self) -> Self {
        Self {
            x: self.x.min(other.x),
            y: self.y.min(other.y),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }
}

fn expired_overwrite(packet: &ffmpeg::Packet, due: i64, canvas: (u32, u32)) -> Option<(i64, Rect)> {
    if packet.is_key() || packet.duration() <= 0 {
        return None;
    }
    let end = packet.pts()?.checked_add(packet.duration())?;
    (end <= due).then_some(())?;
    Some((end, overwrite(packet, canvas)?))
}

fn overwrite(packet: &ffmpeg::Packet, (canvas_width, canvas_height): (u32, u32)) -> Option<Rect> {
    if packet.is_corrupt() {
        return None;
    }
    let (header, mut chunks) = packet.data()?.split_first_chunk::<16>()?;
    let rect = overwrite_rect(header, (canvas_width, canvas_height))?;
    let (mut tag, mut bytes, mut rest) = chunk(chunks)?;
    if tag == b"ALPH" && bytes.len() > 1 && bytes[0] & 0b11 <= 1 {
        chunks = rest;
        (tag, bytes, rest) = chunk(chunks)?;
    }
    (tag == b"VP8 " && rest.is_empty() && matches_vp8_header(bytes, rect.extent())).then_some(rect)
}

pub(super) fn overwrite_rect(header: &[u8; 16], canvas: (u32, u32)) -> Option<Rect> {
    // RFC 9649 ANMF flags: replace the rectangle and leave it in place afterward.
    if header[15] != 2 {
        return None;
    }
    let field = |index| u32::from_le_bytes([header[index], header[index + 1], header[index + 2], 0]);
    let (x, y, width, height) = (2 * field(0), 2 * field(3), 1 + field(6), 1 + field(9));
    let (right, bottom) = (x + width, y + height);
    (right <= canvas.0 && bottom <= canvas.1).then_some(Rect { x, y, right, bottom })
}

pub(super) fn matches_vp8_header(bytes: &[u8], extent: (u32, u32)) -> bool {
    let Some((frame, _)) = bytes.split_first_chunk::<10>() else {
        return false;
    };
    let width = u32::from(u16::from_le_bytes([frame[6], frame[7]]) & 0x3fff);
    let height = u32::from(u16::from_le_bytes([frame[8], frame[9]]) & 0x3fff);
    frame[0] & 1 == 0 && frame[3..6] == [0x9d, 0x01, 0x2a] && (width, height) == extent
}

pub(super) fn independent_patch(packet: &ffmpeg::Packet, canvas: (u32, u32)) -> Option<Rect> {
    let data = packet.data()?;
    if data.len() > MAX_PATCH_BYTES || data.get(16..20) != Some(b"ALPH") {
        return None;
    }
    overwrite(packet, canvas)
}

fn chunk(data: &[u8]) -> Option<(&[u8; 4], &[u8], &[u8])> {
    let (tag, data) = data.split_first_chunk::<4>()?;
    let (len, data) = data.split_first_chunk::<4>()?;
    let len = usize::try_from(u32::from_le_bytes(*len)).ok()?;
    let padded = len.checked_add(len % 2)?;
    let (payload, rest) = data.split_at_checked(padded)?;
    Some((tag, &payload[..len], rest))
}
