use std::collections::BTreeSet;

use indexmap::IndexMap;

use crate::{GraphCommentId, GraphGroupId, GraphNodeId, ViewportBookmarkId};

/// Per-node editor presentation state, independent of domain semantics.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NodePresentation {
    pub position: [f64; 2],
    pub size: Option<[f64; 2]>,
    pub collapsed: bool,
}

impl Default for NodePresentation {
    fn default() -> Self {
        Self {
            position: [0.0; 2],
            size: None,
            collapsed: false,
        }
    }
}

/// Free-form graph comment presentation.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphComment {
    pub id: GraphCommentId,
    pub text: String,
    pub position: [f64; 2],
    pub size: [f64; 2],
}

/// Visual group of graph nodes.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphGroup {
    pub id: GraphGroupId,
    pub label: String,
    pub nodes: BTreeSet<GraphNodeId>,
    pub position: [f64; 2],
    pub size: [f64; 2],
}

/// Named viewport location.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ViewportBookmark {
    pub id: ViewportBookmarkId,
    pub label: String,
    pub origin: [f64; 2],
    pub zoom: f64,
}

/// Selection-independent graph presentation document.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphPresentation {
    pub nodes: IndexMap<GraphNodeId, NodePresentation>,
    pub comments: IndexMap<GraphCommentId, GraphComment>,
    pub groups: IndexMap<GraphGroupId, GraphGroup>,
    pub viewport_bookmarks: IndexMap<ViewportBookmarkId, ViewportBookmark>,
}
