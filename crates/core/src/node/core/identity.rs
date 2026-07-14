use crate::parameter::ParamValueProjection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use golden_model::{DeclId, NodeId, NodeUuid};

/// Persistent reference to another node.
///
/// The UUID is the source of truth and persists on disk.
/// The cached runtime id is optional and never serialized.
/// A cached display name is persisted to keep dangling references user-readable.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct NodeReference {
    /// Persistent target identity.
    pub uuid: NodeUuid,
    /// Optional projection applied when resolving source value from this target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projection: Option<ParamValueProjection>,
    /// Cached user-facing target name used when the reference is currently unresolved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_name: Option<String>,
    /// Best-effort runtime cache for faster lookups.
    #[serde(skip, default)]
    pub cached_id: Option<NodeId>,
    /// Optional relative path from the resolved root to this target.
    ///
    /// Each segment is a child `decl_id` under the previous segment.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relative_path_from_root: Vec<String>,
}

impl NodeReference {
    /// Creates a reference from a persistent UUID.
    pub fn new(uuid: NodeUuid) -> Self {
        Self {
            uuid,
            projection: None,
            cached_name: None,
            cached_id: None,
            relative_path_from_root: Vec::new(),
        }
    }

    /// Creates an empty reference with a nil UUID.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Creates a reference with an explicit runtime cache.
    pub fn with_cached_id(uuid: NodeUuid, cached_id: Option<NodeId>) -> Self {
        Self {
            uuid,
            projection: None,
            cached_name: None,
            cached_id,
            relative_path_from_root: Vec::new(),
        }
    }

    /// Creates a reference with explicit runtime cache and relative root path.
    pub fn with_hints(uuid: NodeUuid, cached_id: Option<NodeId>, relative_path_from_root: Vec<String>) -> Self {
        Self {
            uuid,
            projection: None,
            cached_name: None,
            cached_id,
            relative_path_from_root,
        }
    }

    /// Returns the persistent target UUID.
    pub fn uuid(&self) -> NodeUuid {
        self.uuid
    }

    /// Returns the optional projection attached to this reference.
    pub fn projection(&self) -> Option<ParamValueProjection> {
        self.projection
    }

    /// Sets or clears the projection attached to this reference.
    pub fn set_projection(&mut self, projection: Option<ParamValueProjection>) {
        self.projection = projection;
    }

    /// Clears the projection attached to this reference.
    pub fn clear_projection(&mut self) {
        self.projection = None;
    }

    /// Returns the cached runtime node id, when available.
    pub fn cached_id(&self) -> Option<NodeId> {
        self.cached_id
    }

    /// Replaces the cached runtime id.
    pub fn set_cached_id(&mut self, cached_id: Option<NodeId>) {
        self.cached_id = cached_id;
    }

    /// Clears the cached runtime id.
    pub fn clear_cached_id(&mut self) {
        self.cached_id = None;
    }

    /// Returns the cached user-facing target name, when available.
    pub fn cached_name(&self) -> Option<&str> {
        self.cached_name.as_deref()
    }

    /// Replaces the cached user-facing target name.
    pub fn set_cached_name(&mut self, cached_name: Option<String>) {
        self.cached_name = cached_name.filter(|name| !name.trim().is_empty());
    }

    /// Clears the cached user-facing target name.
    pub fn clear_cached_name(&mut self) {
        self.cached_name = None;
    }

    /// Returns the relative path from resolved root to target.
    pub fn relative_path_from_root(&self) -> &[String] {
        &self.relative_path_from_root
    }

    /// Replaces the relative path hint used for root-relative recovery.
    pub fn set_relative_path_from_root(&mut self, relative_path_from_root: Vec<String>) {
        self.relative_path_from_root = relative_path_from_root;
    }

    /// Clears the relative path hint used for root-relative recovery.
    pub fn clear_relative_path_from_root(&mut self) {
        self.relative_path_from_root.clear();
    }

    /// Returns `true` when this reference has no persistent target.
    pub fn is_empty(&self) -> bool {
        self.uuid.is_nil()
    }
}

impl Default for NodeReference {
    fn default() -> Self {
        Self::new(NodeUuid::default())
    }
}

impl PartialEq for NodeReference {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid && self.projection == other.projection
    }
}

impl Eq for NodeReference {}
