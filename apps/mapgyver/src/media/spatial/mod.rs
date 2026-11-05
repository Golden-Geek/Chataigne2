//! Shared spatial-panel binding policy; domain projections and authoring stay with their owners.
mod geometry;
pub(crate) use geometry::translate_position;
pub mod protocol;
mod query;
pub(in crate::media) use query::inspect;

#[cfg(test)]
mod tests;
