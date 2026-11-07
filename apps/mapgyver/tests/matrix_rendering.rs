#![cfg(all(windows, feature = "native-qualification"))]
//! Matrix native raster and bounded runtime acceptance through app-owned public contracts.
#[cfg(feature = "application-process-fixture")]
mod matrix_controls;
mod matrix_pixels;
pub mod support;
