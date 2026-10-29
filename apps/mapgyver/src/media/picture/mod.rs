//! Picture format decoding and app-owned preparation demand. These owners run outside engine/UI.
mod decoder;
mod preparation;
pub use decoder::PictureDecoder;
pub(crate) use preparation::Pictures;

#[cfg(test)]
mod tests;
