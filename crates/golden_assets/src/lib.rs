//! Persistent asset references and bounded worker preparation. Apps own catalogs and authoring
//! policy; decoders own formats. Neither source bytes nor prepared resources are project data.

mod identity;
mod input;
mod locator;
mod preparation;
mod scope;

pub use identity::{AssetId, AssetRecord};
pub use input::{AssetInput, SourceFingerprint};
pub use locator::{AbsoluteAssetPath, AssetLocator, LocatorError, MAX_ASSET_PATH_BYTES, RelativeAssetPath};
pub use preparation::{
    AssetDecoder, AssetPreparer, AssetRequestId, AssetRetirement, PreparationAdmissionError, PreparationFailure,
    PreparationLimits, PreparationOutcome, PreparationStartError, PreparedAsset,
};
pub use scope::{AssetBinding, AssetCapture, AssetScope};

#[cfg(test)]
mod tests;
