//! Qualification-only owned GPU fixtures. They exercise public runtime consumers without
//! importing app policy or providing an alternative production renderer.

mod consumer;
mod marker;
pub(super) mod pipeline_helper;
mod source;

pub use consumer::{ConsumerFrame, ConsumerTick, FixtureConsumer, FixtureConsumerConfig, FixtureConsumerSnapshot};
pub use marker::{
    CardStamp, MARKER_CELLS, MARKER_COLUMNS, MARKER_EXTENT, MARKER_ORIGIN, MARKER_ROWS, MarkerError, decode_marker,
    encode_marker, marker_hlsl,
};
pub use source::{FixtureSource, FixtureSourceConfig, FixtureSourceSnapshot, FixtureSubmission};

#[cfg(test)]
mod tests;
