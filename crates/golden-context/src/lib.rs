//! Checked context axes and deterministic dense lane layouts.

use std::collections::BTreeMap;

use golden_model::EntityId;
use golden_values::Value;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContextItem {
    pub id: EntityId,
    pub label: SmolStr,
    pub value: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContextAxis {
    pub id: EntityId,
    pub label: SmolStr,
    pub items: Vec<ContextItem>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContextLimits {
    pub maximum_items_per_axis: usize,
    pub maximum_lanes: usize,
}

impl Default for ContextLimits {
    fn default() -> Self {
        Self {
            maximum_items_per_axis: 4_096,
            maximum_lanes: 16_384,
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContextKey(pub Vec<EntityId>);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LaneLayout {
    axes: Vec<ContextAxis>,
    lane_count: usize,
}

impl LaneLayout {
    pub fn compile(axes: Vec<ContextAxis>, limits: ContextLimits) -> Result<Self, ContextError> {
        let mut lane_count = 1usize;
        for axis in &axes {
            if axis.items.len() > limits.maximum_items_per_axis {
                return Err(ContextError::AxisBudgetExceeded {
                    count: axis.items.len(),
                    maximum: limits.maximum_items_per_axis,
                });
            }
            lane_count = lane_count
                .checked_mul(axis.items.len())
                .ok_or(ContextError::CardinalityOverflow)?;
            if lane_count > limits.maximum_lanes {
                return Err(ContextError::LaneBudgetExceeded {
                    count: lane_count,
                    maximum: limits.maximum_lanes,
                });
            }
        }
        Ok(Self { axes, lane_count })
    }

    pub const fn lane_count(&self) -> usize {
        self.lane_count
    }

    pub fn key_at(&self, lane: usize) -> Option<ContextKey> {
        if lane >= self.lane_count {
            return None;
        }
        let mut remainder = lane;
        let mut key = vec![EntityId::default(); self.axes.len()];
        for (axis_index, axis) in self.axes.iter().enumerate().rev() {
            let item_index = remainder % axis.items.len();
            remainder /= axis.items.len();
            key[axis_index] = axis.items[item_index].id;
        }
        Some(ContextKey(key))
    }

    pub fn migration_to(&self, replacement: &Self) -> Vec<(usize, usize)> {
        let old = (0..self.lane_count)
            .filter_map(|index| self.key_at(index).map(|key| (key, index)))
            .collect::<BTreeMap<_, _>>();
        (0..replacement.lane_count)
            .filter_map(|new_index| {
                let key = replacement.key_at(new_index)?;
                old.get(&key).copied().map(|old_index| (old_index, new_index))
            })
            .collect()
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum ContextError {
    #[error("context cardinality overflowed platform size")]
    CardinalityOverflow,
    #[error("axis contains {count} items, exceeding limit {maximum}")]
    AxisBudgetExceeded { count: usize, maximum: usize },
    #[error("context contains {count} lanes, exceeding limit {maximum}")]
    LaneBudgetExceeded { count: usize, maximum: usize },
}

#[cfg(test)]
mod tests;
