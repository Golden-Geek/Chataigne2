//! Shared spatial-panel binding policy; domain projections and authoring stay with their owners.
pub mod protocol;
mod query;
pub(in crate::media) use query::inspect;

#[cfg(test)]
mod tests;
