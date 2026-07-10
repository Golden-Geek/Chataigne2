//! Thin Chataigne composition boundary for the clean-sheet Golden workspace.

use golden_model::{EntityId, Revision};

mod composition;
mod dashboard;
mod host;
mod modules;
mod project;
mod spatializer;

pub use composition::{ChataigneControlRuntime, CompositionError, CompositionStep};
pub use dashboard::{Dashboard, DashboardControl, DashboardRoute};
pub use host::{ChataigneHostError, chataigne_host};
pub use modules::{ModuleCatalog, ModuleDescriptor, ModuleFamily, chataigne_module_catalog};
pub use project::{
    BuiltinFormulaManifest, ChataigneProjectV1, GraphAssetV1, ModuleConfigV1, ProjectEntityV1, ProjectValidationError,
    StatechartAssetV1, builtin_formula_manifests, chataigne_project_codec,
};
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
