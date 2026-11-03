//! App-owned source resolution and shared preview preparation; Golden owns presentation.

pub mod capture;
#[cfg(all(windows, feature = "native-video"))]
pub(crate) mod host;
pub mod layout;
pub mod protocol;
pub(crate) mod runtime_status;

#[cfg(test)]
mod tests;
