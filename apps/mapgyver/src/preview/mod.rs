//! App-owned source resolution and shared preview preparation; Golden owns presentation.

pub mod capture;
#[cfg(all(windows, feature = "native-video"))]
pub(crate) mod host;
pub mod layout;
pub mod protocol;

#[cfg(test)]
mod tests;
