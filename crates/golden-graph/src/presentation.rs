use std::collections::{BTreeMap, BTreeSet};

use golden_model::EntityId;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use thiserror::Error;

use crate::GraphNodeId;

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct FiniteCoordinate(f64);

impl FiniteCoordinate {
    pub fn new(value: f64) -> Result<Self, GeometryError> {
        value
            .is_finite()
            .then_some(Self(value))
            .ok_or(GeometryError::NonFiniteCoordinate)
    }

    pub const fn get(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for FiniteCoordinate {
    type Error = GeometryError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<FiniteCoordinate> for f64 {
    fn from(value: FiniteCoordinate) -> Self {
        value.0
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum GeometryError {
    #[error("graph coordinates must be finite")]
    NonFiniteCoordinate,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: FiniteCoordinate,
    pub y: FiniteCoordinate,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Result<Self, GeometryError> {
        Ok(Self {
            x: FiniteCoordinate::new(x)?,
            y: FiniteCoordinate::new(y)?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub width: FiniteCoordinate,
    pub height: FiniteCoordinate,
}

impl Size {
    pub fn new(width: f64, height: f64) -> Result<Self, GeometryError> {
        Ok(Self {
            width: FiniteCoordinate::new(width)?,
            height: FiniteCoordinate::new(height)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodePresentation {
    pub position: Point,
    pub size: Size,
    pub collapsed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphComment {
    pub id: EntityId,
    pub text: String,
    pub position: Point,
    pub size: Size,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphGroup {
    pub id: EntityId,
    pub label: SmolStr,
    pub nodes: BTreeSet<GraphNodeId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewportBookmark {
    pub id: EntityId,
    pub label: SmolStr,
    pub center: Point,
    pub zoom: FiniteCoordinate,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GraphPresentation {
    pub nodes: BTreeMap<GraphNodeId, NodePresentation>,
    pub comments: BTreeMap<EntityId, GraphComment>,
    pub groups: BTreeMap<EntityId, GraphGroup>,
    pub bookmarks: BTreeMap<EntityId, ViewportBookmark>,
}
