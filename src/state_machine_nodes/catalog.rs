use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use chataigne_state_machine::ProcessorFormulaUiState;
use golden_alchemist::{AlchemistFormula, FormulaId};
use golden_core::{
    node::{
        DashboardWidgetTargetDescriptor, NodeId, NodeReference, NodeUuid,
        PresentationHint, UserCreatableItem,
    },
    parameter::ParamValue,
    process_ctx::{ProcessTreeNodeSnapshot, ProcessTreeSnapshot},
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::app::state_machine_nodes_formula::formula_from_snapshot;

use super::{find_formula_library, FORMULA_NODE_TYPE, PROCESSOR_ITEM_KIND};

pub(super) const PROCESSOR_CREATE_PREFIX: &str = "state_processor:";
const PROCESSOR_PROJECT_CREATE_PREFIX: &str = "state_processor:project:";
const PROCESSOR_BUILTIN_CREATE_PREFIX: &str = "state_processor:builtin:";
const BUILTIN_FORMULA_DIR_ENV: &str = "CHATAIGNE_BUILTIN_FORMULAS_DIR";
const BUILTIN_FORMULA_DIR: &str = "builtin_formulas";
const EXPORTED_NODE_TREE_KIND: &str = "golden-ui.node-tree";
const ANODE_TYPE_TAG_PREFIX: &str = "alchemist.anode.type:";
const FORMULA_DESCRIPTION: &str = "Built-in Chataigne formula.";

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
}

#[derive(Clone, Debug)]
pub(crate) struct FormulaCatalogEntry {
    pub(crate) source: FormulaSourceRef,
    pub(crate) label: String,
    pub(crate) description: String,
    pub(crate) visibility: FormulaVisibility,
    pub(crate) processor_template: Option<ProcessorTemplateMeta>,
    formula: Option<AlchemistFormula>,
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
            formula: None,
        }
    }

    fn builtin_processor_template(
        source: FormulaSourceRef,
        label: impl Into<String>,
        description: impl Into<String>,
        visibility: FormulaVisibility,
        formula: AlchemistFormula,
    ) -> Self {
        let processor_template = Some(ProcessorTemplateMeta::from_source(&source));
        Self {
            source,
            label: label.into(),
            description: description.into(),
            visibility,
            processor_template,
            formula: Some(formula),
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
        Self::from_builtin_formula_dir(builtin_formula_dir())
            .expect("built-in formula files should load")
    }

    pub(crate) fn from_builtin_formula_dir(
        path: impl AsRef<Path>,
    ) -> Result<Self, BuiltinFormulaLoadError> {
        let path = path.as_ref();
        let entries = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => {
                return Err(BuiltinFormulaLoadError::Io {
                    path: path.to_path_buf(),
                    source: error,
                });
            }
        };

        let mut paths = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| BuiltinFormulaLoadError::Io {
                path: path.to_path_buf(),
                source: error,
            })?;
            let formula_path = entry.path();
            if formula_path.extension().and_then(|extension| extension.to_str()) != Some("json") {
                continue;
            }
            paths.push(formula_path);
        }
        paths.sort();

        let mut seen = HashSet::new();
        let mut catalog = Self::default();
        for formula_path in paths {
            let source =
                fs::read_to_string(&formula_path).map_err(|error| BuiltinFormulaLoadError::Io {
                    path: formula_path.clone(),
                    source: error,
                })?;
            let entry = BuiltinFormulaFile::decode(&formula_path, &source)?.into_entry()?;
            let key = builtin_source_key(&entry.source);
            if !seen.insert(key.clone()) {
                return Err(BuiltinFormulaLoadError::DuplicateFormula { source: key });
            }
            catalog.entries.push(entry);
        }
        Ok(catalog)
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
            .and_then(|entry| {
                let mut formula = entry.formula.clone()?;
                if formula.description.is_none() && !entry.description.is_empty() {
                    formula.description = Some(entry.description.clone());
                }
                Some(formula)
            })
            .ok_or_else(|| FormulaCatalogError::BuiltinFormulaNotFound {
                source: source.clone(),
            })
    }

    pub(crate) fn formula_ui_state(&self, source: &FormulaSourceRef) -> ProcessorFormulaUiState {
        match source {
            FormulaSourceRef::ProjectNode(_) => ProcessorFormulaUiState::project(),
            FormulaSourceRef::Builtin { .. } => self
                .builtin_entry(source)
                .map(|entry| {
                    ProcessorFormulaUiState::builtin(
                        entry.visibility.open_readonly_from_processor,
                        entry.visibility.can_duplicate_to_library,
                    )
                })
                .unwrap_or_else(|| ProcessorFormulaUiState::builtin(false, false)),
        }
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
        let has_builtin_items = self
            .processor_palette_entries()
            .any(|entry| matches!(entry.source, FormulaSourceRef::Builtin { .. }));
        let mut saw_project_item = false;

        self.processor_palette_entries()
            .filter_map(|entry| {
                let template = entry.processor_template.as_ref()?;

                let separator_before = match &entry.source {
                    FormulaSourceRef::ProjectNode(_) if has_builtin_items && !saw_project_item => {
                        saw_project_item = true;
                        true
                    }
                    FormulaSourceRef::ProjectNode(_) => {
                        saw_project_item = true;
                        false
                    }
                    FormulaSourceRef::Builtin { .. } => false,
                };

                Some(
                    UserCreatableItem::new(
                        &template.create_type,
                        PROCESSOR_ITEM_KIND,
                        &entry.label,
                    )
                    .with_separator_before(separator_before),
                )
            })
            .collect()
    }
}

fn builtin_formula_dir() -> PathBuf {
    std::env::var_os(BUILTIN_FORMULA_DIR_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(BUILTIN_FORMULA_DIR))
}

fn builtin_source_key(source: &FormulaSourceRef) -> String {
    match source {
        FormulaSourceRef::Builtin {
            package,
            formula_id,
            version,
        } => format!("{package}.{formula_id}@{version}"),
        FormulaSourceRef::ProjectNode(reference) => reference.uuid().0.to_string(),
    }
}

#[derive(Clone, Copy, Debug)]
struct BuiltinFormulaIdentity {
    package: &'static str,
    formula_id: &'static str,
    version: u32,
}

impl BuiltinFormulaIdentity {
    const ACTION: Self = Self {
        package: "chataigne",
        formula_id: "action",
        version: 1,
    };
    const MAPPING: Self = Self {
        package: "chataigne",
        formula_id: "mapping",
        version: 1,
    };

    fn from_file_name(file_name: &str) -> Option<Self> {
        match file_name.to_ascii_lowercase().as_str() {
            "action.json" => Some(Self::ACTION),
            "mapping.json" => Some(Self::MAPPING),
            _ => None,
        }
    }

    fn source(self) -> FormulaSourceRef {
        FormulaSourceRef::builtin(self.package, self.formula_id, self.version)
    }

    fn formula_id(self) -> FormulaId {
        FormulaId::new(format!("{}.{}", self.package, self.formula_id))
    }

    fn visibility(self) -> FormulaVisibility {
        FormulaVisibility {
            show_in_formula_library: false,
            show_in_processor_palette: true,
            can_duplicate_to_library: true,
            open_readonly_from_processor: true,
        }
    }
}

#[derive(Debug)]
struct BuiltinFormulaFile {
    identity: BuiltinFormulaIdentity,
    tree: ExportedNodeTree,
}

impl BuiltinFormulaFile {
    fn decode(path: &Path, source: &str) -> Result<Self, BuiltinFormulaLoadError> {
        let value =
            serde_json::from_str::<JsonValue>(source).map_err(BuiltinFormulaLoadError::Decode)?;
        if value.get("kind").and_then(JsonValue::as_str) != Some(EXPORTED_NODE_TREE_KIND) {
            return Err(BuiltinFormulaLoadError::InvalidExportedFormula {
                reason: format!(
                    "file '{}' is not an exported node-tree formula",
                    path.display()
                ),
            });
        }

        let file_name = path
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .ok_or_else(|| BuiltinFormulaLoadError::UnsupportedFormulaFile {
                path: path.to_path_buf(),
            })?;
        let identity = BuiltinFormulaIdentity::from_file_name(file_name).ok_or_else(|| {
            BuiltinFormulaLoadError::UnsupportedFormulaFile {
                path: path.to_path_buf(),
            }
        })?;
        let tree = serde_json::from_value(value).map_err(BuiltinFormulaLoadError::Decode)?;
        Ok(Self { identity, tree })
    }

    fn into_entry(self) -> Result<FormulaCatalogEntry, BuiltinFormulaLoadError> {
        self.tree.into_catalog_entry(self.identity)
    }
}

#[derive(Debug, Deserialize)]
struct ExportedNodeTree {
    kind: String,
    version: u32,
    nodes: Vec<ExportedNode>,
}

impl ExportedNodeTree {
    fn into_catalog_entry(
        self,
        identity: BuiltinFormulaIdentity,
    ) -> Result<FormulaCatalogEntry, BuiltinFormulaLoadError> {
        if self.kind != EXPORTED_NODE_TREE_KIND {
            return Err(BuiltinFormulaLoadError::InvalidExportedFormula {
                reason: format!("unsupported node-tree kind '{}'", self.kind),
            });
        }
        if self.version != 1 {
            return Err(BuiltinFormulaLoadError::InvalidExportedFormula {
                reason: format!("unsupported node-tree version {}", self.version),
            });
        }
        if self.nodes.len() != 1 {
            return Err(BuiltinFormulaLoadError::InvalidExportedFormula {
                reason: "export must contain exactly one formula root".to_owned(),
            });
        }

        let manager_roles = self.manager_roles_by_uuid()?;
        let root_node = self.nodes.into_iter().next().expect("checked length");
        if root_node.node_type != FORMULA_NODE_TYPE {
            return Err(BuiltinFormulaLoadError::InvalidExportedFormula {
                reason: format!(
                    "export root '{}' is not an Alchemist formula",
                    root_node.label
                ),
            });
        }

        let root = root_node.node_id();
        let mut nodes = HashMap::new();
        push_exported_snapshot_node(
            root_node,
            None,
            None,
            true,
            &manager_roles,
            &mut nodes,
        )?;
        let snapshot = ProcessTreeSnapshot::new(root, nodes);
        let mut formula = formula_from_snapshot(&snapshot, root).map_err(|reason| {
            BuiltinFormulaLoadError::InvalidExportedFormula { reason }
        })?;

        formula.id = identity.formula_id();
        formula.version = identity.version;
        if formula.description.is_none() {
            formula.description = Some(FORMULA_DESCRIPTION.to_owned());
        }
        if formula.graph.metadata.description.is_none() {
            formula.graph.metadata.description = formula.description.clone();
        }

        Ok(FormulaCatalogEntry::builtin_processor_template(
            identity.source(),
            formula.label.clone(),
            formula.description.clone().unwrap_or_default(),
            identity.visibility(),
            formula,
        ))
    }

    fn manager_roles_by_uuid(
        &self,
    ) -> Result<HashMap<NodeUuid, String>, BuiltinFormulaLoadError> {
        let mut roles = HashMap::new();
        for node in &self.nodes {
            collect_exported_manager_roles(node, &mut roles)?;
        }
        Ok(roles)
    }
}

#[derive(Debug, Deserialize)]
struct ExportedNode {
    #[serde(rename = "sourceId")]
    source_id: u64,
    #[serde(rename = "sourceUuid")]
    source_uuid: Uuid,
    node_type: String,
    decl_id: String,
    label: String,
    #[serde(default)]
    data: ExportedNodeData,
    #[serde(default)]
    meta: ExportedNodeMeta,
    #[serde(default)]
    children: Vec<ExportedNode>,
}

impl ExportedNode {
    fn node_id(&self) -> NodeId {
        NodeId(self.source_id)
    }

    fn node_uuid(&self) -> NodeUuid {
        NodeUuid(self.source_uuid)
    }
}

#[derive(Debug, Default, Deserialize)]
struct ExportedNodeMeta {
    #[serde(default)]
    label: Option<String>,
    #[serde(default = "default_enabled")]
    enabled: bool,
    #[serde(default)]
    can_be_disabled: bool,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    presentation: PresentationHint,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ExportedNodeData {
    Node {
        #[allow(dead_code)]
        node_type: String,
    },
    Parameter { param: ExportedParameter },
}

impl Default for ExportedNodeData {
    fn default() -> Self {
        Self::Node {
            node_type: String::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ExportedParameter {
    value: JsonValue,
}

fn push_exported_snapshot_node(
    mut node: ExportedNode,
    parent: Option<NodeId>,
    next_sibling: Option<NodeId>,
    parent_enabled: bool,
    manager_roles: &HashMap<NodeUuid, String>,
    nodes: &mut HashMap<NodeId, ProcessTreeNodeSnapshot>,
) -> Result<(), BuiltinFormulaLoadError> {
    let id = node.node_id();
    let uuid = node.node_uuid();
    let first_child = node.children.first().map(ExportedNode::node_id);
    let enabled = parent_enabled && node.meta.enabled;
    let child_count = node.children.len();
    let mut tags = std::mem::take(&mut node.meta.tags);
    if node.node_type == "alchemist_anode"
        && !tags
            .iter()
            .any(|tag| tag.starts_with(ANODE_TYPE_TAG_PREFIX))
    {
        if let Some(type_id) = exported_anode_type(&node, manager_roles)? {
            tags.push(format!("{ANODE_TYPE_TAG_PREFIX}{type_id}"));
        }
    }
    let param_value = exported_param_value(&node)?;
    let label = node.meta.label.unwrap_or(node.label);
    let decl_id = node.decl_id;
    let short_name = decl_id
        .rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or(decl_id.as_str())
        .to_owned();

    let children = node.children;
    nodes.insert(
        id,
        ProcessTreeNodeSnapshot {
            id,
            uuid,
            parent,
            first_child,
            next_sibling,
            node_type: node.node_type,
            decl_id,
            short_name,
            label,
            tags,
            presentation: node.meta.presentation,
            enabled,
            can_be_disabled: node.meta.can_be_disabled,
            child_count,
            param_value,
            param_constraints: None,
            dashboard_widget_target: DashboardWidgetTargetDescriptor::inspector_only(),
            script_properties: HashMap::new(),
            script_methods: Vec::new(),
        },
    );

    let mut iter = children.into_iter().peekable();
    while let Some(child) = iter.next() {
        let next_child = iter.peek().map(ExportedNode::node_id);
        push_exported_snapshot_node(
            child,
            Some(id),
            next_child,
            enabled,
            manager_roles,
            nodes,
        )?;
    }
    Ok(())
}

fn exported_param_value(
    node: &ExportedNode,
) -> Result<Option<ParamValue>, BuiltinFormulaLoadError> {
    match &node.data {
        ExportedNodeData::Node { .. } => Ok(None),
        ExportedNodeData::Parameter { param } => parse_exported_param_value(&param.value)
            .map(Some)
            .map_err(|reason| BuiltinFormulaLoadError::InvalidExportedFormula {
                reason: format!("parameter '{}' has invalid value: {reason}", node.label),
            }),
    }
}

fn parse_exported_param_value(value: &JsonValue) -> Result<ParamValue, String> {
    let kind = value
        .get("kind")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| "missing value kind".to_owned())?;
    match kind {
        "trigger" => Ok(ParamValue::Trigger()),
        "int" => Ok(ParamValue::Int(
            value_field(value)?
                .as_i64()
                .ok_or_else(|| "int value must be an integer".to_owned())?
                .try_into()
                .map_err(|_| "int value is outside i32 range".to_owned())?,
        )),
        "float" => Ok(ParamValue::Float(number_value(value)?)),
        "str" => Ok(ParamValue::Str(string_value(value)?.to_owned())),
        "file" => Ok(ParamValue::File(string_value(value)?.to_owned())),
        "enum" => Ok(ParamValue::Enum(string_value(value)?.to_owned())),
        "bool" => Ok(ParamValue::Bool(
            value_field(value)?
                .as_bool()
                .ok_or_else(|| "bool value must be a boolean".to_owned())?,
        )),
        "vec2" => {
            let values = number_array(value, 2)?;
            Ok(ParamValue::Vec2(values[0], values[1]))
        }
        "vec3" => {
            let values = number_array(value, 3)?;
            Ok(ParamValue::Vec3(values[0], values[1], values[2]))
        }
        "color" => {
            let color = value.get("value").unwrap_or(value);
            let color = color
                .as_object()
                .ok_or_else(|| "color value must be an object".to_owned())?;
            Ok(ParamValue::Color(
                object_number(color, "r")?,
                object_number(color, "g")?,
                object_number(color, "b")?,
                object_number(color, "a")?,
            ))
        }
        "reference" => {
            let uuid = value
                .get("uuid")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| "reference value must contain a uuid".to_owned())?
                .parse::<Uuid>()
                .map_err(|error| format!("reference uuid is invalid: {error}"))?;
            Ok(ParamValue::Reference(NodeReference::new(NodeUuid(uuid))))
        }
        other => Err(format!("unsupported value kind '{other}'")),
    }
}

fn value_field(value: &JsonValue) -> Result<&JsonValue, String> {
    value
        .get("value")
        .ok_or_else(|| "missing value payload".to_owned())
}

fn string_value(value: &JsonValue) -> Result<&str, String> {
    value_field(value)?
        .as_str()
        .ok_or_else(|| "value must be a string".to_owned())
}

fn number_value(value: &JsonValue) -> Result<f64, String> {
    value_field(value)?
        .as_f64()
        .ok_or_else(|| "value must be a number".to_owned())
}

fn number_array(value: &JsonValue, expected_len: usize) -> Result<Vec<f64>, String> {
    let values = value_field(value)?
        .as_array()
        .ok_or_else(|| "value must be an array".to_owned())?;
    if values.len() != expected_len {
        return Err(format!(
            "value must contain {expected_len} numeric components"
        ));
    }
    values
        .iter()
        .map(|value| {
            value
                .as_f64()
                .ok_or_else(|| "array component must be numeric".to_owned())
        })
        .collect()
}

fn object_number(
    object: &serde_json::Map<String, JsonValue>,
    key: &str,
) -> Result<f64, String> {
    object
        .get(key)
        .and_then(JsonValue::as_f64)
        .ok_or_else(|| format!("color component '{key}' must be numeric"))
}

fn collect_exported_manager_roles(
    node: &ExportedNode,
    roles: &mut HashMap<NodeUuid, String>,
) -> Result<(), BuiltinFormulaLoadError> {
    if node.node_type == "alchemist_property_manager" {
        if let Some(role) = exported_child_string(node, "role")? {
            roles.insert(node.node_uuid(), role);
        }
    }
    for child in &node.children {
        collect_exported_manager_roles(child, roles)?;
    }
    Ok(())
}

fn exported_anode_type(
    node: &ExportedNode,
    manager_roles: &HashMap<NodeUuid, String>,
) -> Result<Option<&'static str>, BuiltinFormulaLoadError> {
    let Some(manager_uuid) = exported_manager_reference(node)? else {
        return Ok(None);
    };
    Ok(manager_roles
        .get(&manager_uuid)
        .and_then(|role| match role.as_str() {
            "condition" => Some(chataigne_state_machine::alchemist::CONDITIONS_MANAGER_TYPE),
            "filter" => Some(chataigne_state_machine::alchemist::FILTERS_MANAGER_TYPE),
            "input" => Some(chataigne_state_machine::alchemist::INPUTS_MANAGER_TYPE),
            "output" => Some(chataigne_state_machine::alchemist::OUTPUTS_MANAGER_TYPE),
            _ => None,
        }))
}

fn exported_manager_reference(
    node: &ExportedNode,
) -> Result<Option<NodeUuid>, BuiltinFormulaLoadError> {
    let Some(config) = node.children.iter().find(|child| child.decl_id == "config") else {
        return Ok(None);
    };
    let Some(manager) = config
        .children
        .iter()
        .find(|child| child.decl_id == "config/manager_id")
    else {
        return Ok(None);
    };
    let Some(value) = exported_param_value(manager)? else {
        return Ok(None);
    };
    Ok(match value {
        ParamValue::Reference(reference) if !reference.is_empty() => Some(reference.uuid()),
        _ => None,
    })
}

fn exported_child_string(
    node: &ExportedNode,
    decl_id: &str,
) -> Result<Option<String>, BuiltinFormulaLoadError> {
    let Some(child) = node.children.iter().find(|child| child.decl_id == decl_id) else {
        return Ok(None);
    };
    let Some(value) = exported_param_value(child)? else {
        return Ok(None);
    };
    Ok(match value {
        ParamValue::Str(value) | ParamValue::Enum(value) => Some(value),
        _ => None,
    })
}

const fn default_enabled() -> bool {
    true
}

#[derive(Debug)]
pub(crate) enum BuiltinFormulaLoadError {
    Decode(serde_json::Error),
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    DuplicateFormula { source: String },
    UnsupportedFormulaFile { path: PathBuf },
    InvalidExportedFormula { reason: String },
}

impl fmt::Display for BuiltinFormulaLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(error) => write!(f, "failed to decode builtin formula file: {error}"),
            Self::Io { path, source } => write!(
                f,
                "failed to read builtin formula file '{}': {source}",
                path.display()
            ),
            Self::DuplicateFormula { source } => {
                write!(f, "builtin formula files contain duplicate formula '{source}'")
            }
            Self::UnsupportedFormulaFile { path } => write!(
                f,
                "builtin formula file '{}' is not a supported built-in formula identity",
                path.display()
            ),
            Self::InvalidExportedFormula { reason } => {
                write!(f, "invalid exported builtin formula: {reason}")
            }
        }
    }
}

impl Error for BuiltinFormulaLoadError {}

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
