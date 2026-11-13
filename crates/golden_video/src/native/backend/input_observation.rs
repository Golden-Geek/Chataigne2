//! Immutable source observations for one render input pass.
use crate::image::{ImageStream, StreamSample};
use crate::native::frames::{GpuFrameStream, Sample};
use crate::plan::{ExternalSource, ExternalTexture};
use std::time::Instant;

pub(super) struct ImageObservation<'a> {
    pub stream: &'a ImageStream,
    pub sample: Option<StreamSample>,
}

pub(super) enum Observation<'a> {
    Gpu {
        stream: &'a GpuFrameStream,
        sample: Option<Sample>,
    },
    Image(ImageObservation<'a>),
    Other,
}

pub(super) fn capture(sources: &[ExternalTexture], instant: Instant) -> impl ExactSizeIterator<Item = Observation<'_>> {
    // Complete every nonblocking read before any source upload/copy can delay the input
    // pass. The compiled external-source limit bounds this temporary metadata vector;
    // cloned immutable images retain their existing admission rather than new storage.
    sources
        .iter()
        .map(move |source| match &source.source {
            Some(ExternalSource::GpuFrames(stream)) => Observation::Gpu {
                stream,
                sample: stream.sample_at(instant),
            },
            Some(ExternalSource::Frames(stream)) => Observation::Image(ImageObservation {
                stream,
                sample: stream.sample(),
            }),
            _ => Observation::Other,
        })
        .collect::<Vec<_>>()
        .into_iter()
}
