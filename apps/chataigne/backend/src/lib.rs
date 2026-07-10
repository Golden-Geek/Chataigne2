//! Thin Chataigne composition boundary for the clean-sheet Golden workspace.

use golden_model::{EntityId, Revision};

mod composition;
mod dashboard;
mod host;
mod modules;
mod spatializer;

pub use composition::{ChataigneControlRuntime, CompositionError, CompositionStep};
pub use dashboard::{Dashboard, DashboardControl, DashboardRoute};
pub use host::{ChataigneHostError, chataigne_host};
pub use modules::{ModuleCatalog, ModuleDescriptor, ModuleFamily, chataigne_module_catalog};
pub use spatializer::{SpatialTarget, Spatializer, SpatializerError, WeightedTarget};

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
