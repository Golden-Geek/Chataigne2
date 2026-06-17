use std::{error::Error, fmt, sync::Arc};

use golden_alchemist::{
    AlchemistFormula, AlchemistGraph, FormulaContextContract, FormulaId,
    FormulaPropertySchema, FormulaSurface,
};
use golden_core::{
    node::{NodeId, NodeReference, NodeUuid, UserCreatableItem},
    process_ctx::ProcessTreeSnapshot,
};
use serde::{Deserialize, Serialize};

use crate::app::state_machine_nodes_formula::formula_from_snapshot;

use super::{find_formula_library, FORMULA_NODE_TYPE, PROCESSOR_ITEM_KIND};

pub(super) const PROCESSOR_CREATE_PREFIX: &str = "state_processor:";
const PROCESSOR_PROJECT_CREATE_PREFIX: &str = "state_processor:project:";
const PROCESSOR_BUILTIN_CREATE_PREFIX: &str = "state_processor:builtin:";
pub(crate) const BUILTIN_FORMULA_PACKAGE: &str = "chataigne";
pub(crate) const BUILTIN_ACTION_FORMULA_ID: &str = "action";
pub(crate) const BUILTIN_MAPPING_FORMULA_ID: &str = "mapping";
pub(crate) const BUILTIN_FORMULA_VERSION: u32 = 1;

#[derive(Clone, Debug)]
pub(crate) enum FormulaSourceRef {
    ProjectNode(NodeReference),
    Builtin {
        package: Arc<str>,
        formula_id: Arc<str>,
        version: u32,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) enum ProcessorFormulaSourceState {
    Empty,
    Project { uuid: String },
    Builtin {
        package: String,
        formula_id: String,
        version: u32,
    },
}

impl Default for ProcessorFormulaSourceState {
    fn default() -> Self {
        Self::Empty
    }
}

impl ProcessorFormulaSourceState {
    pub(crate) fn from_source(source: &FormulaSourceRef) -> Self {
        match source {
            FormulaSourceRef::ProjectNode(reference) => Self::Project {
                uuid: reference.uuid().0.to_string(),
            },
            FormulaSourceRef::Builtin {
                package,
                formula_id,
                version,
            } => Self::Builtin {
                package: package.to_string(),
                formula_id: formula_id.to_string(),
                version: *version,
            },
        }
    }

    pub(crate) fn to_source_ref(
        &self,
    ) -> Result<Option<FormulaSourceRef>, FormulaSourceParseError> {
        match self {
            Self::Empty => Ok(None),
            Self::Project { uuid } => parse_project_formula_source(uuid).map(Some),
            Self::Builtin {
                package,
                formula_id,
                version,
            } => Ok(Some(FormulaSourceRef::builtin(
                package.as_str(),
                formula_id.as_str(),
                *version,
            ))),
        }
    }
}

impl FormulaSourceRef {
    pub(crate) fn project_uuid(uuid: NodeUuid) -> Self {
        Self::ProjectNode(NodeReference::new(uuid))
    }

    pub(crate) fn builtin(
        package: impl Into<Arc<str>>,
        formula_id: impl Into<Arc<str>>,
        version: u32,
    ) -> Self {
        Self::Builtin {
            package: package.into(),
            formula_id: formula_id.into(),
            version,
        }
    }

    pub(crate) fn processor_create_type(&self) -> String {
        match self {
            Self::ProjectNode(reference) => {
                format!("{}{}", PROCESSOR_PROJECT_CREATE_PREFIX, reference.uuid().0)
            }
            Self::Builtin {
                package,
                formula_id,
                version,
            } => {
                format!(
                    "{}{}.{}@{}",
                    PROCESSOR_BUILTIN_CREATE_PREFIX, package, formula_id, version
                )
            }
        }
    }

    pub(crate) fn parse_processor_create_type(
        node_type: &str,
    ) -> Result<Self, FormulaSourceParseError> {
        if let Some(uuid) = node_type.strip_prefix(PROCESSOR_PROJECT_CREATE_PREFIX) {
            return parse_project_formula_source(uuid);
        }

        if let Some(source) = node_type.strip_prefix(PROCESSOR_BUILTIN_CREATE_PREFIX) {
            return parse_builtin_formula_source(source);
        }

        if let Some(uuid) = node_type.strip_prefix(PROCESSOR_CREATE_PREFIX) {
            return parse_project_formula_source(uuid);
        }

        Err(FormulaSourceParseError::UnsupportedPrefix {
            node_type: node_type.to_owned(),
        })
    }
}

impl fmt::Display for FormulaSourceRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectNode(reference) => write!(f, "project:{}", reference.uuid().0),
            Self::Builtin {
                package,
                formula_id,
                version,
            } => write!(f, "builtin:{}.{}@{}", package, formula_id, version),
        }
    }
}

fn parse_project_formula_source(
    uuid: &str,
) -> Result<FormulaSourceRef, FormulaSourceParseError> {
    uuid.parse::<uuid::Uuid>()
        .map(NodeUuid)
        .map(FormulaSourceRef::project_uuid)
        .map_err(|_| FormulaSourceParseError::InvalidProjectUuid {
            value: uuid.to_owned(),
        })
}

fn parse_builtin_formula_source(
    source: &str,
) -> Result<FormulaSourceRef, FormulaSourceParseError> {
    let (source_id, version) =
        source
            .rsplit_once('@')
            .ok_or_else(|| FormulaSourceParseError::InvalidBuiltinSource {
                value: source.to_owned(),
            })?;
    let version =
        version
            .parse::<u32>()
            .map_err(|_| FormulaSourceParseError::InvalidBuiltinVersion {
                value: source.to_owned(),
            })?;
    let (package, formula_id) =
        source_id
            .split_once('.')
            .ok_or_else(|| FormulaSourceParseError::InvalidBuiltinSource {
                value: source.to_owned(),
            })?;
    if package.is_empty() || formula_id.is_empty() {
        return Err(FormulaSourceParseError::InvalidBuiltinSource {
            value: source.to_owned(),
        });
    }
    Ok(FormulaSourceRef::builtin(package, formula_id, version))
}

#[derive(Clone, Debug)]
pub(crate) enum FormulaSourceParseError {
    UnsupportedPrefix { node_type: String },
    InvalidProjectUuid { value: String },
    InvalidBuiltinSource { value: String },
    InvalidBuiltinVersion { value: String },
}

impl fmt::Display for FormulaSourceParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPrefix { node_type } => {
                write!(f, "unsupported processor formula source '{node_type}'")
            }
            Self::InvalidProjectUuid { value } => {
                write!(f, "invalid project formula uuid '{value}'")
            }
            Self::InvalidBuiltinSource { value } => {
                write!(f, "invalid builtin formula source '{value}'")
            }
            Self::InvalidBuiltinVersion { value } => {
                write!(f, "invalid builtin formula version in '{value}'")
            }
        }
    }
}

impl Error for FormulaSourceParseError {}

#[derive(Clone, Debug)]
pub(crate) struct ProcessorTemplateMeta {
    pub(crate) create_type: String,
}

impl ProcessorTemplateMeta {
    fn from_source(source: &FormulaSourceRef) -> Self {
        Self {
            create_type: source.processor_create_type(),
        }
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) struct FormulaVisibility {
    pub(crate) show_in_formula_library: bool,
    pub(crate) show_in_processor_palette: bool,
    pub(crate) can_duplicate_to_library: bool,
    pub(crate) open_readonly_from_processor: bool,
}

impl FormulaVisibility {
    fn project_formula() -> Self {
        Self {
            show_in_formula_library: true,
            show_in_processor_palette: true,
            can_duplicate_to_library: false,
            open_readonly_from_processor: false,
        }
    }

    fn builtin_processor_template() -> Self {
        Self {
            show_in_formula_library: false,
            show_in_processor_palette: true,
            can_duplicate_to_library: true,
            open_readonly_from_processor: true,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FormulaCatalogEntry {
    pub(crate) source: FormulaSourceRef,
    pub(crate) label: String,
    pub(crate) description: String,
    pub(crate) visibility: FormulaVisibility,
    pub(crate) processor_template: Option<ProcessorTemplateMeta>,
}

impl FormulaCatalogEntry {
    fn processor_template(
        source: FormulaSourceRef,
        label: impl Into<String>,
        description: impl Into<String>,
        visibility: FormulaVisibility,
    ) -> Self {
        let processor_template = Some(ProcessorTemplateMeta::from_source(&source));
        Self {
            source,
            label: label.into(),
            description: description.into(),
            visibility,
            processor_template,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct FormulaCatalog {
    entries: Vec<FormulaCatalogEntry>,
}

impl FormulaCatalog {
    pub(crate) fn from_snapshot(snapshot: &ProcessTreeSnapshot) -> Self {
        let mut catalog = Self::with_builtins();
        if let Some(library) = find_formula_library(snapshot) {
            catalog.add_project_formulas(snapshot, library);
        }
        catalog
    }

    pub(crate) fn with_builtins() -> Self {
        let mut catalog = Self::default();
        catalog.entries.push(Self::builtin_processor_entry(
            BUILTIN_ACTION_FORMULA_ID,
            "Action",
            "Built-in action formula surface.",
        ));
        catalog.entries.push(Self::builtin_processor_entry(
            BUILTIN_MAPPING_FORMULA_ID,
            "Mapping",
            "Built-in mapping formula surface.",
        ));
        catalog
    }

    fn builtin_processor_entry(
        formula_id: &'static str,
        label: &'static str,
        description: &'static str,
    ) -> FormulaCatalogEntry {
        FormulaCatalogEntry::processor_template(
            FormulaSourceRef::builtin(
                BUILTIN_FORMULA_PACKAGE,
                formula_id,
                BUILTIN_FORMULA_VERSION,
            ),
            label,
            description,
            FormulaVisibility::builtin_processor_template(),
        )
    }

    fn add_project_formulas(&mut self, snapshot: &ProcessTreeSnapshot, library: NodeId) {
        self.entries.extend(
            snapshot
                .child_ids(library)
                .into_iter()
                .filter_map(|formula_id| {
                    let formula = snapshot.node(formula_id)?;
                    (formula.node_type == FORMULA_NODE_TYPE).then(|| {
                        FormulaCatalogEntry::processor_template(
                            FormulaSourceRef::project_uuid(formula.uuid),
                            formula.label.clone(),
                            String::new(),
                            FormulaVisibility::project_formula(),
                        )
                    })
                }),
        );
    }

    pub(crate) fn processor_palette_entries(&self) -> impl Iterator<Item = &FormulaCatalogEntry> {
        self.entries.iter().filter(|entry| {
            entry.visibility.show_in_processor_palette && entry.processor_template.is_some()
        })
    }

    #[allow(dead_code)]
    pub(crate) fn resolve(
        &self,
        snapshot: &ProcessTreeSnapshot,
        source: &FormulaSourceRef,
    ) -> Result<AlchemistFormula, FormulaCatalogError> {
        match source {
            FormulaSourceRef::ProjectNode(reference) => {
                let uuid = reference.uuid();
                let formula_node = snapshot
                    .node_id_by_uuid(uuid)
                    .filter(|formula| {
                        snapshot
                            .node(*formula)
                            .is_some_and(|node| node.node_type == FORMULA_NODE_TYPE)
                    })
                    .ok_or(FormulaCatalogError::ProjectFormulaNotFound { uuid })?;
                formula_from_snapshot(snapshot, formula_node)
                    .map_err(FormulaCatalogError::InvalidProjectFormula)
            }
            FormulaSourceRef::Builtin { .. } => self.resolve_builtin(source),
        }
    }

    pub(crate) fn resolve_builtin(
        &self,
        source: &FormulaSourceRef,
    ) -> Result<AlchemistFormula, FormulaCatalogError> {
        self.builtin_entry(source)
            .map(builtin_formula_from_entry)
            .ok_or_else(|| FormulaCatalogError::BuiltinFormulaNotFound {
                source: source.clone(),
            })
    }

    fn builtin_entry(&self, source: &FormulaSourceRef) -> Option<&FormulaCatalogEntry> {
        let FormulaSourceRef::Builtin {
            package,
            formula_id,
            version,
        } = source
        else {
            return None;
        };

        self.entries.iter().find(|entry| {
            matches!(
                &entry.source,
                FormulaSourceRef::Builtin {
                    package: entry_package,
                    formula_id: entry_formula_id,
                    version: entry_version,
                } if entry_package.as_ref() == package.as_ref()
                    && entry_formula_id.as_ref() == formula_id.as_ref()
                    && entry_version == version
            )
        })
    }

    pub(super) fn processor_palette_items(&self) -> Vec<UserCreatableItem> {
        self.processor_palette_entries()
            .filter_map(|entry| {
                let template = entry.processor_template.as_ref()?;
                Some(UserCreatableItem::new(
                    &template.create_type,
                    PROCESSOR_ITEM_KIND,
                    &entry.label,
                ))
            })
            .collect()
    }
}

fn builtin_formula_from_entry(entry: &FormulaCatalogEntry) -> AlchemistFormula {
    let FormulaSourceRef::Builtin {
        package,
        formula_id,
        version,
    } = &entry.source
    else {
        unreachable!("builtin formula entry requires a builtin source");
    };

    AlchemistFormula {
        id: FormulaId::new(format!("{}.{}", package, formula_id)),
        version: *version,
        label: entry.label.clone(),
        description: Some(entry.description.clone()),
        tags: vec![format!("builtin:{}.{}@{}", package, formula_id, version)],
        graph: AlchemistGraph::new(),
        properties: FormulaPropertySchema::default(),
        surface: FormulaSurface::default(),
        context_contract: FormulaContextContract::default(),
        migrations: Vec::new(),
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) enum FormulaCatalogError {
    ProjectFormulaNotFound { uuid: NodeUuid },
    InvalidProjectFormula(String),
    BuiltinFormulaNotFound { source: FormulaSourceRef },
}

impl fmt::Display for FormulaCatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectFormulaNotFound { uuid } => {
                write!(f, "project formula '{}' was not found", uuid.0)
            }
            Self::InvalidProjectFormula(error) => {
                write!(f, "project formula is invalid: {error}")
            }
            Self::BuiltinFormulaNotFound { source } => {
                write!(f, "builtin formula source '{}' was not found", source)
            }
        }
    }
}

impl Error for FormulaCatalogError {}
