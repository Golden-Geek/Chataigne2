use super::protocol::{VideoSourceError, VideoSourceErrorKind};
use golden_video::decode::{ConversionError, DecodeError};

#[derive(Debug)]
pub(super) struct Failure(pub VideoSourceError);
impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self(VideoSourceError::new(VideoSourceErrorKind::Decode, message))
    }
}
impl From<&str> for Failure {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}
impl From<DecodeError> for Failure {
    fn from(error: DecodeError) -> Self {
        let kind = match error {
            DecodeError::NotAFile => VideoSourceErrorKind::Missing,
            DecodeError::UnsupportedCodec | DecodeError::UnsupportedHardware => VideoSourceErrorKind::Unsupported,
            DecodeError::LimitExceeded => VideoSourceErrorKind::Capacity,
            _ => VideoSourceErrorKind::Decode,
        };
        Self(VideoSourceError::new(kind, error.to_string()))
    }
}
impl From<ConversionError> for Failure {
    fn from(error: ConversionError) -> Self {
        let kind = match error {
            ConversionError::HardwareStorage
            | ConversionError::UnsupportedMatrix
            | ConversionError::UnsupportedTransfer => VideoSourceErrorKind::Unsupported,
            ConversionError::Failed(_) => VideoSourceErrorKind::Decode,
        };
        Self(VideoSourceError::new(kind, error.to_string()))
    }
}
