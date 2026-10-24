//! A byte buffer is a value, not encoded text or an application-specific frame.

use serde::{Deserialize, Serialize};

/// Owned binary data for typed parameter handles and the Params DSL.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ByteArray(pub Vec<u8>);

impl ByteArray {
    /// Borrows the bytes without encoding or copying them.
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
    /// Moves the underlying buffer to its next owner.
    pub fn into_inner(self) -> Vec<u8> {
        self.0
    }
}

impl From<Vec<u8>> for ByteArray {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

impl From<ByteArray> for Vec<u8> {
    fn from(bytes: ByteArray) -> Self {
        bytes.0
    }
}
