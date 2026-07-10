//! Thin Chataigne composition boundary for the clean-sheet Golden workspace.

use golden_model::{EntityId, Revision};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChataigneProjectIdentity {
    pub id: EntityId,
    pub revision: Revision,
}

impl ChataigneProjectIdentity {
    pub fn new() -> Self {
        Self {
            id: EntityId::new(),
            revision: Revision::ZERO,
        }
    }
}

impl Default for ChataigneProjectIdentity {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
