use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::locator::AssetLocator;

/// Persistent project identity, independent of source paths, content and prepared resources.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
pub struct AssetId(Uuid);

impl AssetId {
    /// Allocates a new authored identity using a version-four UUID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns the persistent UUID; this is never a backend resource handle.
    pub const fn get(self) -> Uuid {
        self.0
    }
}

impl Default for AssetId {
    fn default() -> Self {
        Self::new()
    }
}

/// Serializable source record. The app owns its catalog and relink/history policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AssetRecord {
    /// Stable identity retained when the source is moved or relinked.
    pub id: AssetId,
    /// Authored source location; existence and content are worker observations.
    pub locator: AssetLocator,
}
