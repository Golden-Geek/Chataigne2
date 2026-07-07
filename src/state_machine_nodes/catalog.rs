use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
};

use golden_alchemist::{
    AlchemistFormula, ManagedRegionDefinition, ManagedRegionId, ManagedRegionKind,
    SurfaceItemKind,
};
use golden_core::{
    app::ProjectNode,
    edit::NodeTree,
    node::{
        DashboardWidgetTargetDescriptor, DeclId, Node, NodeId, NodeMeta,
        NodeReference, NodeUserPermissions, NodeUuid, PresentationHint,
        UserCreatableItem,
    },
    parameter::ParamValue,
    process_ctx::{ProcessTreeNodeSnapshot, ProcessTreeSnapshot},
};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::app::{
    state_machine_nodes_formula::{
        formula_from_snapshot, FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX,
        FORMULA_EXTERNAL_READ_ONLY_TAG, FORMULA_MANAGED_REGIONS_JSON_DECL_ID,
        PROPERTIES_DECL_ID, PROPERTY_FOLDER_NODE_TYPE, PROPERTY_MANAGER_NODE_TYPE,
    },
    AppNode,
};

use super::{find_formula_library, FORMULA_NODE_TYPE, PROCESSOR_ITEM_KIND};

pub(super) const PROCESSOR_CREATE_PREFIX: &str = "state_processor:";
const PROCESSOR_PROJECT_CREATE_PREFIX: &str = "state_processor:project:";
const BUILTIN_FORMULA_DIR_ENV: &str = "CHATAIGNE_BUILTIN_FORMULAS_DIR";
const BUILTIN_FORMULA_DIR: &str = "builtin_formulas";
const EXPORTED_NODE_TREE_KIND: &str = "golden-ui.node-tree";
const ANODE_TYPE_TAG_PREFIX: &str = "alchemist.anode.type:";

#[derive(Clone, Debug)]
pub(crate) enum FormulaSourceRef {
    ProjectNode(NodeReference),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) enum ProcessorFormulaSourceState {
    Empty,
    Project { uuid: String },
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
        }
    }

    pub(crate) fn to_source_ref(
        &self,
    ) -> Result<Option<FormulaSourceRef>, FormulaSourceParseError> {
        match self {
            Self::Empty => Ok(None),
            Self::Project { uuid } => parse_project_formula_source(uuid).map(Some),
        }
    }
}

impl FormulaSourceRef {
    pub(crate) fn project_uuid(uuid: NodeUuid) -> Self {
        Self::ProjectNode(NodeReference::new(uuid))
    }

    pub(crate) fn processor_create_type(&self) -> String {
        match self {
            Self::ProjectNode(reference) => {
                format!("{}{}", PROCESSOR_PROJECT_CREATE_PREFIX, reference.uuid().0)
            }
        }
    }

    pub(crate) fn parse_processor_create_type(
        node_type: &str,
    ) -> Result<Self, FormulaSourceParseError> {
        if let Some(uuid) = node_type.strip_prefix(PROCESSOR_PROJECT_CREATE_PREFIX) {
            return parse_project_formula_source(uuid);
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

#[derive(Clone, Debug)]
pub(crate) enum FormulaSourceParseError {
    UnsupportedPrefix { node_type: String },
    InvalidProjectUuid { value: String },
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
    pub(crate) label: String,
    pub(crate) visibility: FormulaVisibility,
    pub(crate) processor_template: Option<ProcessorTemplateMeta>,
    is_builtin_external: bool,
}

impl FormulaCatalogEntry {
    fn processor_template(
        source: FormulaSourceRef,
        label: impl Into<String>,
        visibility: FormulaVisibility,
    ) -> Self {
        let processor_template = Some(ProcessorTemplateMeta::from_source(&source));
        Self {
            label: label.into(),
            visibility,
            processor_template,
            is_builtin_external: false,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct FormulaCatalog {
    entries: Vec<FormulaCatalogEntry>,
}

impl FormulaCatalog {
    pub(crate) fn from_snapshot(snapshot: &ProcessTreeSnapshot) -> Self {
        let mut catalog = Self::default();
        if let Some(library) = find_formula_library(snapshot) {
            catalog.add_project_formulas(snapshot, library);
        }
        catalog
    }

    fn add_project_formulas(&mut self, snapshot: &ProcessTreeSnapshot, library: NodeId) {
        self.entries.extend(
            snapshot
                .child_ids(library)
                .into_iter()
                .filter_map(|formula_id| {
                    let formula = snapshot.node(formula_id)?;
                    (formula.node_type == FORMULA_NODE_TYPE).then(|| {
                        let mut entry = FormulaCatalogEntry::processor_template(
                            FormulaSourceRef::project_uuid(formula.uuid),
                            formula.label.clone(),
                            FormulaVisibility::project_formula(),
                        );
                        entry.is_builtin_external = formula
                            .tags
                            .iter()
                            .any(|tag| tag.starts_with(FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX));
                        entry.visibility.open_readonly_from_processor =
                            formula.tags.iter().any(|tag| tag == FORMULA_EXTERNAL_READ_ONLY_TAG);
                        entry.visibility.can_duplicate_to_library =
                            entry.visibility.open_readonly_from_processor;
                        entry
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
        }
    }

    pub(super) fn processor_palette_items(&self) -> Vec<UserCreatableItem> {
        let has_builtin_items = self
            .processor_palette_entries()
            .any(|entry| entry.is_builtin_external);
        let mut saw_session_item = false;

        self.processor_palette_entries()
            .filter_map(|entry| {
                let template = entry.processor_template.as_ref()?;

                let separator_before = if entry.is_builtin_external {
                    false
                } else if has_builtin_items && !saw_session_item {
                    saw_session_item = true;
                    true
                } else {
                    saw_session_item = true;
                    false
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

    pub(crate) fn builtin_formula_trees(
        path: impl AsRef<Path>,
    ) -> Result<Vec<NodeTree>, BuiltinFormulaLoadError> {
        let path = path.as_ref();
        let entries = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Vec::new());
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

        paths
            .into_iter()
            .map(|formula_path| {
                let source = fs::read_to_string(&formula_path).map_err(|error| {
                    BuiltinFormulaLoadError::Io {
                        path: formula_path.clone(),
                        source: error,
                    }
                })?;
                BuiltinFormulaFile::decode(&formula_path, &source)?.into_node_tree()
            })
            .collect()
    }

    pub(crate) fn default_builtin_formula_trees() -> Result<Vec<NodeTree>, BuiltinFormulaLoadError>
    {
        Self::builtin_formula_trees(builtin_formula_dir())
    }

    pub(crate) fn external_formula_tree_from_file(
        path: impl AsRef<Path>,
    ) -> Result<NodeTree, BuiltinFormulaLoadError> {
        let path = path.as_ref();
        let source =
            fs::read_to_string(path).map_err(|error| BuiltinFormulaLoadError::Io {
                path: path.to_path_buf(),
                source: error,
            })?;
        let tree = decode_exported_formula_tree(path, &source)?;
        let identity_hint = path
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .and_then(BuiltinFormulaIdentity::from_file_name);
        let icon = sibling_icon_data_uri(path)?;
        tree.into_external_node_tree(false, identity_hint, icon)
    }
}

fn builtin_formula_dir() -> PathBuf {
    std::env::var_os(BUILTIN_FORMULA_DIR_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(BUILTIN_FORMULA_DIR))
}

/// Looks for a sibling `.svg`/`.png` file matching `formula_path`'s file stem and,
/// if found, returns it encoded as a data URI to use as the formula's icon.
fn sibling_icon_data_uri(formula_path: &Path) -> Result<Option<String>, BuiltinFormulaLoadError> {
    for (extension, mime) in [("svg", "image/svg+xml"), ("png", "image/png")] {
        let icon_path = formula_path.with_extension(extension);
        match fs::read(&icon_path) {
            Ok(bytes) => {
                let encoded = BASE64_STANDARD.encode(bytes);
                return Ok(Some(format!("data:{mime};base64,{encoded}")));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(BuiltinFormulaLoadError::Io {
                    path: icon_path,
                    source: error,
                });
            }
        }
    }
    Ok(None)
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

    fn stable_node_uuid(self) -> NodeUuid {
        let value = match self.formula_id {
            "action" => "11111111-2222-4333-8444-000000000001",
            "mapping" => "11111111-2222-4333-8444-000000000002",
            _ => "11111111-2222-4333-8444-000000000000",
        };
        NodeUuid(Uuid::parse_str(value).expect("built-in formula uuid should be valid"))
    }

    fn external_builtin_tag(self) -> String {
        format!(
            "{FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX}{}.{}@{}",
            self.package, self.formula_id, self.version
        )
    }
}

#[derive(Debug)]
struct BuiltinFormulaFile {
    identity: BuiltinFormulaIdentity,
    tree: ExportedNodeTree,
    icon: Option<String>,
}

impl BuiltinFormulaFile {
    fn decode(path: &Path, source: &str) -> Result<Self, BuiltinFormulaLoadError> {
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
        let tree = decode_exported_formula_tree(path, source)?;
        let icon = sibling_icon_data_uri(path)?;
        Ok(Self { identity, tree, icon })
    }

    fn into_node_tree(self) -> Result<NodeTree, BuiltinFormulaLoadError> {
        self.tree.into_node_tree(self.identity, self.icon)
    }
}

#[derive(Debug, Deserialize)]
struct ExportedNodeTree {
    kind: String,
    version: u32,
    nodes: Vec<ExportedNode>,
}

impl ExportedNodeTree {
    fn into_node_tree(
        self,
        identity: BuiltinFormulaIdentity,
        icon: Option<String>,
    ) -> Result<NodeTree, BuiltinFormulaLoadError> {
        self.into_formula_node_tree(
            Some(identity.stable_node_uuid()),
            true,
            Some(identity.external_builtin_tag()),
            Some(identity),
            icon,
        )
    }

    fn into_external_node_tree(
        self,
        read_only: bool,
        identity_hint: Option<BuiltinFormulaIdentity>,
        icon: Option<String>,
    ) -> Result<NodeTree, BuiltinFormulaLoadError> {
        self.into_formula_node_tree(None, read_only, None, identity_hint, icon)
    }

    fn into_formula_node_tree(
        self,
        forced_uuid: Option<NodeUuid>,
        read_only: bool,
        root_provenance_tag: Option<String>,
        builtin_identity: Option<BuiltinFormulaIdentity>,
        icon: Option<String>,
    ) -> Result<NodeTree, BuiltinFormulaLoadError> {
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
        let mut root_node = self.nodes.into_iter().next().expect("checked length");
        if root_node.node_type != FORMULA_NODE_TYPE {
            return Err(BuiltinFormulaLoadError::InvalidExportedFormula {
                reason: format!(
                    "export root '{}' is not an Alchemist formula",
                    root_node.label
                ),
            });
        }

        if let Some(managed_regions_json) =
            derived_managed_regions_json(&root_node, &manager_roles, builtin_identity)?
        {
            set_exported_managed_regions_json(&mut root_node, managed_regions_json)?;
        }

        exported_node_to_tree(
            root_node,
            forced_uuid,
            read_only,
            root_provenance_tag,
            icon,
            &manager_roles,
        )
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

fn decode_exported_formula_tree(
    path: &Path,
    source: &str,
) -> Result<ExportedNodeTree, BuiltinFormulaLoadError> {
    let value = serde_json::from_str::<JsonValue>(source).map_err(BuiltinFormulaLoadError::Decode)?;
    if value.get("kind").and_then(JsonValue::as_str) != Some(EXPORTED_NODE_TREE_KIND) {
        return Err(BuiltinFormulaLoadError::InvalidExportedFormula {
            reason: format!(
                "file '{}' is not an exported node-tree formula",
                path.display()
            ),
        });
    }
    serde_json::from_value(value).map_err(BuiltinFormulaLoadError::Decode)
}

#[derive(Clone, Debug, Deserialize)]
struct ExportedNode {
    #[serde(rename = "sourceId")]
    source_id: u64,
    #[serde(rename = "sourceUuid")]
    source_uuid: Uuid,
    node_type: String,
    decl_id: String,
    label: String,
    #[serde(default)]
    data: JsonValue,
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

#[derive(Clone, Debug, Default, Deserialize)]
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
    match exported_node_data(node)? {
        ExportedNodeData::Node { .. } => Ok(None),
        ExportedNodeData::Parameter { param } => parse_exported_param_value(&param.value)
            .map(Some)
            .map_err(|reason| BuiltinFormulaLoadError::InvalidExportedFormula {
                reason: format!("parameter '{}' has invalid value: {reason}", node.label),
            }),
    }
}

fn exported_node_data(
    node: &ExportedNode,
) -> Result<ExportedNodeData, BuiltinFormulaLoadError> {
    serde_json::from_value(node.data.clone()).map_err(BuiltinFormulaLoadError::Decode)
}

fn exported_node_to_tree(
    mut node: ExportedNode,
    forced_uuid: Option<NodeUuid>,
    read_only: bool,
    root_provenance_tag: Option<String>,
    root_icon: Option<String>,
    manager_roles: &HashMap<NodeUuid, String>,
) -> Result<NodeTree, BuiltinFormulaLoadError> {
    let mut tags = std::mem::take(&mut node.meta.tags);
    if let Some(tag) = root_provenance_tag {
        if !tags.iter().any(|existing| existing == &tag) {
            tags.push(tag);
        }
    }
    if read_only && !tags.iter().any(|tag| tag == FORMULA_EXTERNAL_READ_ONLY_TAG) {
        tags.push(FORMULA_EXTERNAL_READ_ONLY_TAG.to_owned());
    }
    if node.node_type == "alchemist_anode"
        && !tags
            .iter()
            .any(|tag| tag.starts_with(ANODE_TYPE_TAG_PREFIX))
    {
        if let Some(type_id) = exported_anode_type(&node, manager_roles)? {
            tags.push(format!("{ANODE_TYPE_TAG_PREFIX}{type_id}"));
        }
    }

    let label = node.meta.label.take().unwrap_or_else(|| node.label.clone());
    let mut meta = NodeMeta::new(label);
    meta.uuid = forced_uuid.unwrap_or_else(|| node.node_uuid());
    meta.decl_id = DeclId(node.decl_id.clone());
    meta.short_name = node
        .decl_id
        .rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or(node.decl_id.as_str())
        .to_owned();
    meta.enabled = node.meta.enabled;
    meta.can_be_disabled = node.meta.can_be_disabled && !read_only;
    meta.tags = tags;
    meta.presentation = node.meta.presentation;
    if let Some(icon) = root_icon {
        meta.presentation.icon = Some(icon);
    }
    if read_only {
        meta.user_permissions = NodeUserPermissions::none();
    }

    let project_data = exported_project_data(&node.data, read_only)?;
    let mut decoded =
        <AppNode as ProjectNode>::project_decode_node(&node.node_type, &project_data, &meta)
            .map_err(|reason| BuiltinFormulaLoadError::InvalidExportedFormula {
                reason: format!("failed to decode exported node '{}': {reason}", node.label),
            })?;
    decoded.node_data_mut().meta = meta;

    let mut tree = NodeTree::new(decoded);
    for child in node.children {
        tree.push_child(exported_node_to_tree(
            child,
            None,
            read_only,
            None,
            None,
            manager_roles,
        )?);
    }
    Ok(tree)
}

fn exported_project_data(
    data: &JsonValue,
    read_only: bool,
) -> Result<JsonValue, BuiltinFormulaLoadError> {
    match data.get("kind").and_then(JsonValue::as_str) {
        Some("parameter") => exported_parameter_project_data(
            data.get("param").cloned().unwrap_or(JsonValue::Null),
            read_only,
        ),
        Some("node") => Ok(JsonValue::Null),
        _ => Ok(data.clone()),
    }
}

fn exported_parameter_project_data(
    param: JsonValue,
    force_read_only: bool,
) -> Result<JsonValue, BuiltinFormulaLoadError> {
    let mut project = serde_json::Map::new();
    for field in ["value", "default_value"] {
        let Some(value) = param.get(field).cloned() else {
            continue;
        };
        let decoded = parse_exported_param_value(&value).map_err(|reason| {
            BuiltinFormulaLoadError::InvalidExportedFormula {
                reason: format!("parameter {field} has invalid value: {reason}"),
            }
        })?;
        project.insert(
            field.to_owned(),
            serde_json::to_value(decoded).map_err(BuiltinFormulaLoadError::Decode)?,
        );
    }
    for field in ["event_behaviour", "read_only", "persist_read_only_value"] {
        if let Some(value) = param.get(field).cloned() {
            project.insert(field.to_owned(), value);
        }
    }
    if force_read_only {
        project.insert("read_only".to_owned(), JsonValue::Bool(true));
    }
    Ok(JsonValue::Object(project))
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

fn derived_managed_regions_json(
    root_node: &ExportedNode,
    manager_roles: &HashMap<NodeUuid, String>,
    identity: Option<BuiltinFormulaIdentity>,
) -> Result<Option<String>, BuiltinFormulaLoadError> {
    if exported_child_string(root_node, FORMULA_MANAGED_REGIONS_JSON_DECL_ID)?
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        return Ok(None);
    }

    let root = root_node.node_id();
    let mut nodes = HashMap::new();
    push_exported_snapshot_node(
        root_node.clone(),
        None,
        None,
        true,
        manager_roles,
        &mut nodes,
    )?;
    let snapshot = ProcessTreeSnapshot::new(root, nodes);
    let regions = managed_regions_from_property_managers(&snapshot, root, identity);
    if regions.is_empty() {
        return Ok(None);
    }
    serde_json::to_string(&regions)
        .map(Some)
        .map_err(BuiltinFormulaLoadError::Decode)
}

fn set_exported_managed_regions_json(
    root_node: &mut ExportedNode,
    managed_regions_json: String,
) -> Result<(), BuiltinFormulaLoadError> {
    let Some(child) = root_node
        .children
        .iter_mut()
        .find(|child| child.decl_id == FORMULA_MANAGED_REGIONS_JSON_DECL_ID)
    else {
        return Err(BuiltinFormulaLoadError::InvalidExportedFormula {
            reason: format!(
                "built-in formula '{}' does not expose managed region metadata",
                root_node.label
            ),
        });
    };
    let Some(param) = child.data.get_mut("param") else {
        return Err(BuiltinFormulaLoadError::InvalidExportedFormula {
            reason: format!(
                "built-in formula '{}' has invalid managed region metadata parameter",
                root_node.label
            ),
        });
    };
    param["value"] = serde_json::json!({
        "kind": "str",
        "value": managed_regions_json,
    });
    Ok(())
}

fn managed_regions_from_property_managers(
    snapshot: &ProcessTreeSnapshot,
    formula_node: NodeId,
    identity: Option<BuiltinFormulaIdentity>,
) -> Vec<ManagedRegionDefinition> {
    let Some(properties) = snapshot.find_child_by_decl_id(formula_node, PROPERTIES_DECL_ID) else {
        return Vec::new();
    };

    let mut used_ids = HashSet::new();
    let mut regions = Vec::new();
    collect_managed_regions_from_property_managers(
        snapshot,
        properties,
        identity,
        &mut used_ids,
        &mut regions,
    );
    regions
}

fn collect_managed_regions_from_property_managers(
    snapshot: &ProcessTreeSnapshot,
    container: NodeId,
    identity: Option<BuiltinFormulaIdentity>,
    used_ids: &mut HashSet<String>,
    regions: &mut Vec<ManagedRegionDefinition>,
) {
    for child in snapshot.child_ids(container) {
        let Some(node) = snapshot.node(child) else {
            continue;
        };
        if node.node_type == PROPERTY_MANAGER_NODE_TYPE {
            if let Some(region) =
                managed_region_from_property_manager(snapshot, child, identity, used_ids)
            {
                regions.push(region);
            }
        } else if node.node_type == PROPERTY_FOLDER_NODE_TYPE {
            collect_managed_regions_from_property_managers(
                snapshot, child, identity, used_ids, regions,
            );
        }
    }
}

fn managed_region_from_property_manager(
    snapshot: &ProcessTreeSnapshot,
    manager: NodeId,
    identity: Option<BuiltinFormulaIdentity>,
    used_ids: &mut HashSet<String>,
) -> Option<ManagedRegionDefinition> {
    let node = snapshot.node(manager)?;
    let role = snapshot_child_string(snapshot, manager, "role")
        .unwrap_or_else(|| "condition".to_owned());
    let (kind, accepted_role) = managed_region_contract(identity, &role)?;
    let id = stable_managed_region_id(&node.label, &role, used_ids);

    Some(ManagedRegionDefinition {
        id: ManagedRegionId::new(id),
        kind,
        label: node.label.clone(),
        input_socket: None,
        output_socket: None,
        accepted_roles: vec![accepted_role],
    })
}

fn managed_region_contract(
    identity: Option<BuiltinFormulaIdentity>,
    role: &str,
) -> Option<(ManagedRegionKind, SurfaceItemKind)> {
    match role {
        "condition" => Some((
            ManagedRegionKind::TriggerInput,
            SurfaceItemKind::Input,
        )),
        "filter" => Some((
            ManagedRegionKind::FilterPipeline,
            SurfaceItemKind::Filter,
        )),
        "input" => Some((ManagedRegionKind::InputSet, SurfaceItemKind::Input)),
        "output"
            if identity
                .is_some_and(|identity| identity.formula_id == BuiltinFormulaIdentity::ACTION.formula_id) =>
        {
            Some((ManagedRegionKind::CommandSet, SurfaceItemKind::Command))
        }
        "output" => Some((ManagedRegionKind::OutputSet, SurfaceItemKind::Output)),
        _ => None,
    }
}

fn stable_managed_region_id(
    label: &str,
    role: &str,
    used_ids: &mut HashSet<String>,
) -> String {
    let fallback = slug_identifier(role);
    let base = slug_identifier(label);
    let base = if base.is_empty() { fallback } else { base };
    let mut id = base.clone();
    let mut index = 2;
    while !used_ids.insert(id.clone()) {
        id = format!("{base}_{index}");
        index += 1;
    }
    id
}

fn slug_identifier(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_was_separator = false;
    for character in value.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
            previous_was_separator = false;
        } else if !previous_was_separator && !slug.is_empty() {
            slug.push('_');
            previous_was_separator = true;
        }
    }
    if previous_was_separator {
        slug.pop();
    }
    slug
}

fn snapshot_child_string(
    snapshot: &ProcessTreeSnapshot,
    node: NodeId,
    decl_id: &str,
) -> Option<String> {
    let child = snapshot.find_child_by_decl_id(node, decl_id)?;
    match snapshot.node(child)?.param_value.as_ref()? {
        ParamValue::Str(value) | ParamValue::Enum(value) => Some(value.clone()),
        _ => None,
    }
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
        }
    }
}

impl Error for FormulaCatalogError {}
