use super::{DecoderKind, ffmpeg};

/// Explicit software codec compute admission. Slice workers cooperate on the current frame;
/// neither automatic CPU-count pools nor delayed frame-threading queues can be selected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SoftwareDecodeThreads {
    /// Serial codec execution.
    #[default]
    One,
    /// At most two slice workers when the codec supports them.
    Two,
    /// At most four slice workers when the codec supports them.
    Four,
}

impl SoftwareDecodeThreads {
    fn count(self) -> usize {
        match self {
            Self::One => 1,
            Self::Two => 2,
            Self::Four => 4,
        }
    }
}

pub(super) fn configuration(
    context: &ffmpeg::codec::context::Context,
    kind: &DecoderKind,
    limit: SoftwareDecodeThreads,
) -> ffmpeg::codec::threading::Config {
    let capable = matches!(kind, DecoderKind::Software)
        && ffmpeg::decoder::find(context.id()).is_some_and(|codec| {
            codec
                .capabilities()
                .contains(ffmpeg::codec::capabilities::Capabilities::SLICE_THREADS)
        });
    if capable && limit.count() > 1 {
        let mut config = ffmpeg::codec::threading::Config::count(limit.count());
        config.kind = ffmpeg::codec::threading::Type::Slice;
        config
    } else {
        ffmpeg::codec::threading::Config::count(1)
    }
}
