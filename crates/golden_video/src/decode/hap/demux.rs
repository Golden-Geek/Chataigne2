use super::super::{DecodeError, DecodeState, DecodedFrame, Decoder, DecoderKind, FrameStorage, ffmpeg};
use super::HapFrame;
impl Decoder {
    pub(in crate::decode) fn receive_hap(&mut self) -> Result<Option<DecodedFrame>, DecodeError> {
        if self.state == DecodeState::Ended {
            return Ok(None);
        }
        let DecoderKind::Hap(budget) = &self.kind else {
            return Err(DecodeError::UnsupportedCodec);
        };
        for _ in 0..self.limits.max_packets_per_frame {
            let mut packet = ffmpeg::Packet::empty();
            match packet.read(&mut self.input) {
                Ok(()) => {
                    if packet.size() > self.limits.max_packet_bytes {
                        return Err(DecodeError::LimitExceeded);
                    }
                    if packet.stream() != self.stream {
                        continue;
                    }
                    let timestamp = packet.pts().ok_or(DecodeError::MissingTimestamp)?;
                    let extent = crate::Extent2D::new(self.info.width, self.info.height)
                        .map_err(|_| DecodeError::LimitExceeded)?;
                    let frame = HapFrame::decode(
                        packet.data().ok_or_else(|| DecodeError::Hap("empty packet".into()))?,
                        extent,
                        budget,
                    )?;
                    return Ok(Some(DecodedFrame {
                        timestamp,
                        storage: FrameStorage::Hap(frame),
                    }));
                }
                Err(ffmpeg::Error::Eof) => {
                    self.state = DecodeState::Ended;
                    return Ok(None);
                }
                Err(error) => return Err(error.into()),
            }
        }
        Err(DecodeError::LimitExceeded)
    }
}
