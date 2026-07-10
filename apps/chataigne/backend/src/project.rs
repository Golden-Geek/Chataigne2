use std::collections::{BTreeMap, BTreeSet};

use golden_alchemist::{FORMULA_FILE_VERSION, FormulaFileV1};
use golden_model::EntityId;
use golden_persistence::{MigrationRegistry, PersistenceLimits, ProjectCodec};
use golden_values::Value;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use thiserror::Error;

use crate::Dashboard;

pub const CHATAIGNE_PROJECT_SCHEMA: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChataigneProjectV1 {
    pub project_id: EntityId,
    pub entities: Vec<ProjectEntityV1>,
    pub graphs: Vec<GraphAssetV1>,
    pub formulas: Vec<FormulaFileV1>,
    pub statecharts: Vec<StatechartAssetV1>,
    pub contexts: BTreeMap<SmolStr, serde_json::Value>,
    pub processors: BTreeMap<SmolStr, serde_json::Value>,
    pub modules: Vec<ModuleConfigV1>,
    pub dashboards: Vec<Dashboard>,
    pub presentation: serde_json::Value,
}

impl ChataigneProjectV1 {
    pub fn empty() -> Self {
        Self {
            project_id: EntityId::new(),
            entities: Vec::new(),
            graphs: Vec::new(),
            formulas: Vec::new(),
            statecharts: Vec::new(),
            contexts: BTreeMap::new(),
            processors: BTreeMap::new(),
            modules: Vec::new(),
            dashboards: Vec::new(),
            presentation: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    pub fn validate(&self) -> Result<(), ProjectValidationError> {
        if self.entities.len() > 100_000 {
            return Err(ProjectValidationError::EntityLimit);
        }
        let entities = self.entities.iter().map(|entity| entity.id).collect::<BTreeSet<_>>();
        if entities.len() != self.entities.len() {
            return Err(ProjectValidationError::DuplicateEntity);
        }
        let parents = self
            .entities
            .iter()
            .map(|entity| (entity.id, entity.parent))
            .collect::<BTreeMap<_, _>>();
        for entity in &self.entities {
            if entity.parent.is_some_and(|parent| !entities.contains(&parent)) {
                return Err(ProjectValidationError::MissingParent);
            }
            let mut cursor = entity.parent;
            let mut visited = BTreeSet::new();
            while let Some(parent) = cursor {
                if !visited.insert(parent) {
                    return Err(ProjectValidationError::HierarchyCycle);
                }
                cursor = parents.get(&parent).copied().flatten();
            }
        }
        if self
            .graphs
            .iter()
            .any(|graph| graph.domain_id.is_empty() || graph.domain_schema_version == 0)
        {
            return Err(ProjectValidationError::InvalidGraphSchema);
        }
        if self
            .formulas
            .iter()
            .any(|formula| formula.file_version != FORMULA_FILE_VERSION)
        {
            return Err(ProjectValidationError::InvalidFormulaSchema);
        }
        let catalog = crate::chataigne_module_catalog();
        let mut module_ids = BTreeSet::new();
        for module in &self.modules {
            if !module_ids.insert(module.id) {
                return Err(ProjectValidationError::DuplicateModule);
            }
            if catalog.get(&module.type_id).is_none() {
                return Err(ProjectValidationError::UnknownModule(module.type_id.clone()));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectEntityV1 {
    pub id: EntityId,
    pub parent: Option<EntityId>,
    pub name: SmolStr,
    pub parameters: BTreeMap<SmolStr, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphAssetV1 {
    pub id: EntityId,
    pub domain_id: SmolStr,
    pub domain_schema_version: u32,
    pub document: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatechartAssetV1 {
    pub id: EntityId,
    pub schema_version: u32,
    pub document: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleConfigV1 {
    pub id: EntityId,
    pub type_id: SmolStr,
    pub settings: BTreeMap<SmolStr, Value>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BuiltinFormulaManifest {
    pub id: &'static str,
    pub application_version: &'static str,
    pub file_schema_version: u32,
    pub immutable: bool,
}

pub const fn builtin_formula_manifests() -> [BuiltinFormulaManifest; 2] {
    [
        BuiltinFormulaManifest {
            id: "chataigne.action",
            application_version: env!("CARGO_PKG_VERSION"),
            file_schema_version: FORMULA_FILE_VERSION,
            immutable: true,
        },
        BuiltinFormulaManifest {
            id: "chataigne.mapping",
            application_version: env!("CARGO_PKG_VERSION"),
            file_schema_version: FORMULA_FILE_VERSION,
            immutable: true,
        },
    ]
}

pub fn chataigne_project_codec() -> ProjectCodec<ChataigneProjectV1> {
    ProjectCodec::new(
        "chataigne",
        env!("CARGO_PKG_VERSION"),
        CHATAIGNE_PROJECT_SCHEMA,
        PersistenceLimits::default(),
        MigrationRegistry::default(),
    )
    .expect("static Chataigne project codec configuration is valid")
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ProjectValidationError {
    #[error("project contains more than 100,000 entities")]
    EntityLimit,
    #[error("project entity identifiers must be unique")]
    DuplicateEntity,
    #[error("project entity references a missing parent")]
    MissingParent,
    #[error("project entity hierarchy contains a cycle")]
    HierarchyCycle,
    #[error("graph domain identifiers and schema versions must be explicit")]
    InvalidGraphSchema,
    #[error("formula file schema is unsupported")]
    InvalidFormulaSchema,
    #[error("module identifiers must be unique")]
    DuplicateModule,
    #[error("module type is not registered by Chataigne: {0}")]
    UnknownModule(SmolStr),
}
