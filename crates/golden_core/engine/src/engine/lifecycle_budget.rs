use std::num::NonZeroUsize;

use super::{Engine, EngineEditError};
use crate::edit::NodeTree;
use crate::node::Node;

/// Guardrails for callback-generated structure, including materialization outside timed ticks.
/// Explicit detached input trees are not expansion work; their own generated follow-ups are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LifecycleLimits {
    /// Maximum drains/preprocessing rounds in one materialization scope.
    pub max_passes: NonZeroUsize,
    /// Maximum generated edit requests plus newly materialized nodes in that scope.
    pub max_work: NonZeroUsize,
}

impl Default for LifecycleLimits {
    fn default() -> Self {
        Self {
            max_passes: NonZeroUsize::new(64).unwrap(),
            max_work: NonZeroUsize::new(65_536).unwrap(),
        }
    }
}

pub(super) struct LifecycleBudget {
    limits: LifecycleLimits,
    passes: usize,
    work: usize,
}

impl LifecycleBudget {
    pub(super) fn new(limits: LifecycleLimits) -> Self {
        Self {
            limits,
            passes: 0,
            work: 0,
        }
    }

    fn charge(used: &mut usize, amount: usize, limit: usize, resource: &'static str) -> Result<(), EngineEditError> {
        let attempted = used.saturating_add(amount);
        if attempted > limit {
            return Err(EngineEditError::LifecycleBudgetExceeded {
                resource,
                limit,
                attempted,
            });
        }
        *used = attempted;
        Ok(())
    }
}

impl<T: Node> Engine<T> {
    /// Returns the callback-expansion limits used by newly entered lifecycle scopes.
    pub fn lifecycle_limits(&self) -> LifecycleLimits {
        self.lifecycle_limits
    }

    /// Sets limits for subsequent scopes; an active scope retains its admitted limits.
    pub fn set_lifecycle_limits(&mut self, limits: LifecycleLimits) {
        self.lifecycle_limits = limits;
    }

    pub(super) fn charge_lifecycle_pass(&mut self) -> Result<(), EngineEditError> {
        if let Some(budget) = &mut self.lifecycle_budget {
            LifecycleBudget::charge(&mut budget.passes, 1, budget.limits.max_passes.get(), "passes")?;
        }
        Ok(())
    }

    pub(super) fn charge_lifecycle_work(&mut self, amount: usize) -> Result<(), EngineEditError> {
        if let Some(budget) = &mut self.lifecycle_budget {
            LifecycleBudget::charge(&mut budget.work, amount, budget.limits.max_work.get(), "work")?;
        }
        Ok(())
    }

    /// Reserve the complete detached forest before inserting any of it. Iterators
    /// keep traversal memory proportional to depth, including very wide trees.
    pub(super) fn reserve_lifecycle_trees<'a>(
        &mut self,
        trees: impl IntoIterator<Item = &'a NodeTree>,
    ) -> Result<(), EngineEditError> {
        if self.lifecycle_budget.is_none() {
            return Ok(());
        }
        for tree in trees {
            self.charge_lifecycle_work(1)?;
            let mut stack = vec![tree.children.iter()];
            while let Some(children) = stack.last_mut() {
                if let Some(child) = children.next() {
                    self.charge_lifecycle_work(1)?;
                    stack.push(child.children.iter());
                } else {
                    stack.pop();
                }
            }
        }
        Ok(())
    }
}
