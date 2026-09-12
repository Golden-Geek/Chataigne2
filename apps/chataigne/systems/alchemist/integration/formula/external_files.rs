use super::*;

pub(super) fn preserve_external_formula_child(decl_id: &str) -> bool {
    matches!(
        decl_id,
        "is_valid"
            | "diagnostics_json"
            | FORMULA_MANAGED_REGIONS_JSON_DECL_ID
            | FORMULA_EXTERNAL_FILE_DECL_ID
            | FORMULA_EXTERNAL_SOURCE_DECL_ID
            | FORMULA_EXTERNAL_DELETE_FILE_DECL_ID
            | FORMULA_COPY_SOURCE_DECL_ID
    )
}

pub(super) fn imported_formula_content_children(imported: NodeTree) -> Vec<NodeTree> {
    imported
        .children
        .into_iter()
        .filter_map(|child| {
            let decl_id = child.node.node_data().meta.decl_id.0.as_str();
            if decl_id == FORMULA_MANAGED_REGIONS_JSON_DECL_ID {
                return None;
            }
            (!preserve_external_formula_child(decl_id)).then_some(child)
        })
        .collect()
}

pub(super) fn write_formula_node_to_file(
    snapshot: &ProcessTreeSnapshot,
    formula_node: NodeId,
    path: &Path,
) -> Result<(), String> {
    write_formula_node_to_file_with_root_uuid(snapshot, formula_node, path, None)
}

pub(super) fn write_formula_node_to_file_with_root_uuid(
    snapshot: &ProcessTreeSnapshot,
    formula_node: NodeId,
    path: &Path,
    root_uuid: Option<NodeUuid>,
) -> Result<(), String> {
    let Some(json) =
        FormulaCatalog::export_formula_json_with_root_uuid(snapshot, formula_node, root_uuid)
    else {
        return Err("formula could not be serialized".to_owned());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!("failed to create formula folder '{}': {error}", parent.display())
        })?;
    }
    fs::write(path, json)
        .map_err(|error| format!("failed to write formula file '{}': {error}", path.display()))
}

pub(super) enum SharedFormulaFileRename {
    Renamed(PathBuf),
    Blocked,
}

pub(super) fn shared_formula_path_for_label(shared_dir: &Path, label: &str) -> PathBuf {
    shared_dir.join(format!("{}.json", shared_formula_file_stem(label)))
}

pub(super) fn shared_formula_file_stem(label: &str) -> String {
    let mut stem = String::new();
    for ch in label.trim().chars() {
        if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
            stem.push('_');
        } else {
            stem.push(ch);
        }
    }
    let mut stem = stem
        .trim_matches(|ch| ch == ' ' || ch == '.')
        .to_owned();
    if stem.is_empty() {
        stem = "Shared Formula".to_owned();
    }
    if is_windows_reserved_file_stem(&stem) {
        stem.insert(0, '_');
    }
    stem
}

pub(super) fn is_windows_reserved_file_stem(stem: &str) -> bool {
    let device_name = stem
        .split('.')
        .next()
        .unwrap_or(stem)
        .to_ascii_uppercase();
    matches!(device_name.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || device_name
            .strip_prefix("COM")
            .and_then(|suffix| suffix.parse::<u8>().ok())
            .is_some_and(|index| (1..=9).contains(&index))
        || device_name
            .strip_prefix("LPT")
            .and_then(|suffix| suffix.parse::<u8>().ok())
            .is_some_and(|index| (1..=9).contains(&index))
}

pub(super) fn paths_refer_to_same_file(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

impl AlchemistFormulaDefinition {
    pub(super) fn is_external_file_formula(&self) -> bool {
        self.node_data()
            .meta
            .tags
            .iter()
            .any(|tag| tag == FORMULA_EXTERNAL_FILE_TAG)
    }

    pub(super) fn is_read_only_external_formula(&self) -> bool {
        self.node_data()
            .meta
            .tags
            .iter()
            .any(|tag| tag == FORMULA_EXTERNAL_READ_ONLY_TAG)
    }

    pub(super) fn reconcile_external_formula_file_parameter(&self, ctx: &mut ProcessCtx) {
        if !self.is_external_file_formula() {
            return;
        }
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        if snapshot
            .find_child_by_decl_id(self.id(), FORMULA_EXTERNAL_FILE_DECL_ID)
            .is_none()
            && !child_add_pending(ctx, self.id(), FORMULA_EXTERNAL_FILE_DECL_ID)
        {
            ctx.add_child(self.id(), external_formula_file_parameter(), None);
        }
    }

    pub(super) fn reconcile_external_formula_operation_parameters(&self, ctx: &mut ProcessCtx) {
        if !self.is_external_file_formula() {
            return;
        }
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        if snapshot
            .find_child_by_decl_id(self.id(), FORMULA_EXTERNAL_SOURCE_DECL_ID)
            .is_none()
            && !child_add_pending(ctx, self.id(), FORMULA_EXTERNAL_SOURCE_DECL_ID)
        {
            ctx.add_child(self.id(), external_formula_source_parameter(), None);
        }
        if snapshot
            .find_child_by_decl_id(self.id(), FORMULA_EXTERNAL_DELETE_FILE_DECL_ID)
            .is_none()
            && !child_add_pending(ctx, self.id(), FORMULA_EXTERNAL_DELETE_FILE_DECL_ID)
        {
            ctx.add_child(self.id(), external_formula_delete_file_parameter(), None);
        }
    }

    pub(super) fn reconcile_formula_copy_source_parameter(&self, ctx: &mut ProcessCtx) {
        if self.is_external_file_formula() {
            return;
        }
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        if snapshot
            .find_child_by_decl_id(self.id(), FORMULA_COPY_SOURCE_DECL_ID)
            .is_none()
            && !child_add_pending(ctx, self.id(), FORMULA_COPY_SOURCE_DECL_ID)
        {
            ctx.add_child(self.id(), formula_copy_source_parameter(), None);
        }
    }

    pub(super) fn external_formula_file_path(&self, ctx: &ProcessCtx) -> Option<PathBuf> {
        let snapshot = ctx.tree_snapshot()?;
        let value = child_param(snapshot, self.id(), FORMULA_EXTERNAL_FILE_DECL_ID)?;
        match value {
            ParamValue::File(path) | ParamValue::Str(path) => {
                let path = path.trim();
                (!path.is_empty()).then(|| PathBuf::from(path))
            }
            _ => None,
        }
    }

    pub(super) fn rename_shared_formula_file_for_label(
        &self,
        ctx: &mut ProcessCtx,
        label: &str,
    ) -> Option<SharedFormulaFileRename> {
        if !self.is_external_file_formula() || self.is_read_only_external_formula() {
            return None;
        }
        let snapshot = ctx.tree_snapshot_arc()?;
        let shared_dir = shared_formula_dir_from_snapshot(snapshot.as_ref())?;
        let current_path = self.external_formula_file_path(ctx)?;
        if !current_path.starts_with(&shared_dir) {
            return None;
        }
        let next_path = shared_formula_path_for_label(&shared_dir, label);
        if next_path == current_path {
            return None;
        }
        let file_param =
            snapshot.find_child_by_decl_id(self.id(), FORMULA_EXTERNAL_FILE_DECL_ID)?;
        let current_exists = current_path.exists();
        if current_exists && next_path.exists() && !paths_refer_to_same_file(&current_path, &next_path)
        {
            ctx.set_node_warning_with(
                self.id(),
                Some(FORMULA_EXTERNAL_FILE_WARNING_ID),
                "Shared formula file could not be renamed",
                Some(&format!("target file '{}' already exists", next_path.display())),
            );
            return Some(SharedFormulaFileRename::Blocked);
        }
        if let Some(parent) = next_path.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                ctx.set_node_warning_with(
                    self.id(),
                    Some(FORMULA_EXTERNAL_FILE_WARNING_ID),
                    "Shared formula file could not be renamed",
                    Some(&format!("failed to create folder '{}': {error}", parent.display())),
                );
                return Some(SharedFormulaFileRename::Blocked);
            }
        }
        if current_exists && !paths_refer_to_same_file(&current_path, &next_path) {
            if let Err(error) = fs::rename(&current_path, &next_path) {
                ctx.set_node_warning_with(
                    self.id(),
                    Some(FORMULA_EXTERNAL_FILE_WARNING_ID),
                    "Shared formula file could not be renamed",
                    Some(&format!(
                        "failed to rename '{}' to '{}': {error}",
                        current_path.display(),
                        next_path.display()
                    )),
                );
                return Some(SharedFormulaFileRename::Blocked);
            }
        }
        ctx.edits.push(Edit::SetParam {
            node: file_param,
            value: ParamValue::File(next_path.to_string_lossy().into_owned()),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        Some(SharedFormulaFileRename::Renamed(next_path))
    }

    pub(super) fn source_reference_param(
        &self,
        ctx: &ProcessCtx,
        decl_id: &str,
    ) -> Option<(NodeId, NodeReference)> {
        let snapshot = ctx.tree_snapshot()?;
        let param = snapshot.find_child_by_decl_id(self.id(), decl_id)?;
        match snapshot.node(param)?.param_value.as_ref()? {
            ParamValue::Reference(reference) if !reference.uuid().is_nil() => {
                Some((param, reference.clone()))
            }
            _ => None,
        }
    }

    pub(super) fn resolve_source_reference(
        snapshot: &ProcessTreeSnapshot,
        reference: &NodeReference,
    ) -> Option<NodeId> {
        reference
            .cached_id()
            .filter(|node| {
                snapshot
                    .node(*node)
                    .is_some_and(|snapshot_node| snapshot_node.uuid == reference.uuid())
            })
            .or_else(|| snapshot.node_id_by_uuid(reference.uuid()))
    }

    pub(super) fn is_external_formula_file_param(&self, ctx: &ProcessCtx, param: NodeId) -> bool {
        ctx.tree_snapshot()
            .and_then(|snapshot| snapshot.node(param))
            .is_some_and(|node| node.decl_id == FORMULA_EXTERNAL_FILE_DECL_ID)
    }

    pub(super) fn is_external_formula_source_param(&self, ctx: &ProcessCtx, param: NodeId) -> bool {
        ctx.tree_snapshot()
            .and_then(|snapshot| snapshot.node(param))
            .is_some_and(|node| node.decl_id == FORMULA_EXTERNAL_SOURCE_DECL_ID)
    }

    pub(super) fn is_external_formula_delete_file_param(&self, ctx: &ProcessCtx, param: NodeId) -> bool {
        ctx.tree_snapshot()
            .and_then(|snapshot| snapshot.node(param))
            .is_some_and(|node| node.decl_id == FORMULA_EXTERNAL_DELETE_FILE_DECL_ID)
    }

    pub(super) fn is_formula_copy_source_param(&self, ctx: &ProcessCtx, param: NodeId) -> bool {
        ctx.tree_snapshot()
            .and_then(|snapshot| snapshot.node(param))
            .is_some_and(|node| node.decl_id == FORMULA_COPY_SOURCE_DECL_ID)
    }

    pub(super) fn is_formula_internal_param(&self, param: NodeId) -> bool {
        param == self.is_valid.id()
            || param == self.diagnostics_json.id()
            || param == self.managed_regions_json.id()
    }

    pub(super) fn write_external_formula_from_source(&mut self, ctx: &mut ProcessCtx) -> bool {
        if !self.is_external_file_formula() {
            return false;
        }
        let Some((source_param, reference)) =
            self.source_reference_param(ctx, FORMULA_EXTERNAL_SOURCE_DECL_ID)
        else {
            return false;
        };
        let Some(path) = self.external_formula_file_path(ctx) else {
            ctx.set_node_warning_with(
                self.id(),
                Some(FORMULA_EXTERNAL_FILE_WARNING_ID),
                "External formula file could not be saved",
                Some("missing target file path"),
            );
            return true;
        };
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return true;
        };
        let Some(source) = Self::resolve_source_reference(snapshot.as_ref(), &reference) else {
            ctx.set_node_warning_with(
                self.id(),
                Some(FORMULA_EXTERNAL_FILE_WARNING_ID),
                "External formula file could not be saved",
                Some("source formula no longer exists"),
            );
            return true;
        };
        if let Err(error) =
            write_formula_node_to_file(snapshot.as_ref(), source, path.as_path())
        {
            ctx.set_node_warning_with(
                self.id(),
                Some(FORMULA_EXTERNAL_FILE_WARNING_ID),
                "External formula file could not be saved",
                Some(&error),
            );
            return true;
        }
        ctx.set_param(
            source_param,
            ParamValue::Reference(NodeReference::empty()),
        );
        self.sync_external_formula_file(ctx);
        true
    }

    pub(super) fn copy_formula_from_source(&mut self, ctx: &mut ProcessCtx) -> bool {
        if self.is_external_file_formula() {
            return false;
        }
        let Some((source_param, reference)) =
            self.source_reference_param(ctx, FORMULA_COPY_SOURCE_DECL_ID)
        else {
            return false;
        };
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return true;
        };
        let Some(source) = Self::resolve_source_reference(snapshot.as_ref(), &reference) else {
            ctx.set_node_warning_with(
                self.id(),
                Some(FORMULA_EXTERNAL_FILE_WARNING_ID),
                "Formula could not be copied",
                Some("source formula no longer exists"),
            );
            return true;
        };
        match FormulaCatalog::formula_node_tree(snapshot.as_ref(), source) {
            Ok(tree) => {
                self.replace_formula_contents(ctx, tree);
                ctx.set_param(
                    source_param,
                    ParamValue::Reference(NodeReference::empty()),
                );
                true
            }
            Err(error) => {
                ctx.set_node_warning_with(
                    self.id(),
                    Some(FORMULA_EXTERNAL_FILE_WARNING_ID),
                    "Formula could not be copied",
                    Some(&error.to_string()),
                );
                true
            }
        }
    }

    pub(super) fn save_external_formula_file(&self, ctx: &mut ProcessCtx) {
        if !self.is_external_file_formula() || self.is_read_only_external_formula() {
            return;
        }
        let Some(path) = self.external_formula_file_path(ctx) else {
            return;
        };
        self.save_external_formula_file_to_path(ctx, path.as_path());
    }

    pub(super) fn save_external_formula_file_to_path(&self, ctx: &mut ProcessCtx, path: &Path) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let root_uuid = shared_formula_dir_from_snapshot(snapshot.as_ref())
            .filter(|shared_dir| path.starts_with(shared_dir))
            .and_then(|_| {
                FormulaCatalog::shared_formula_file_identity(path)
                    .ok()
                    .map(|(uuid, _)| uuid)
            });
        match write_formula_node_to_file_with_root_uuid(
            snapshot.as_ref(),
            self.id(),
            path,
            root_uuid,
        ) {
            Ok(()) => ctx.clear_node_warning(self.id(), Some(FORMULA_EXTERNAL_FILE_WARNING_ID)),
            Err(error) => ctx.set_node_warning_with(
                self.id(),
                Some(FORMULA_EXTERNAL_FILE_WARNING_ID),
                "External formula file could not be saved",
                Some(&error),
            ),
        }
    }

    pub(super) fn delete_external_formula_file_if_requested(&self, ctx: &mut ProcessCtx) -> bool {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return false;
        };
        let Some(param) =
            snapshot.find_child_by_decl_id(self.id(), FORMULA_EXTERNAL_DELETE_FILE_DECL_ID)
        else {
            return false;
        };
        if !matches!(
            snapshot.node(param).and_then(|node| node.param_value.as_ref()),
            Some(ParamValue::Bool(true))
        ) {
            return false;
        }
        let Some(path) = self.external_formula_file_path(ctx) else {
            ctx.set_param(param, ParamValue::Bool(false));
            return true;
        };
        if !shared_formula_dir_from_snapshot(snapshot.as_ref())
            .is_some_and(|shared_dir| path.starts_with(shared_dir))
        {
            ctx.set_param(param, ParamValue::Bool(false));
            return true;
        }
        match fs::remove_file(&path) {
            Ok(()) => ctx.clear_node_warning(self.id(), Some(FORMULA_EXTERNAL_FILE_WARNING_ID)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                ctx.clear_node_warning(self.id(), Some(FORMULA_EXTERNAL_FILE_WARNING_ID))
            }
            Err(error) => ctx.set_node_warning_with(
                self.id(),
                Some(FORMULA_EXTERNAL_FILE_WARNING_ID),
                "Shared formula file could not be removed",
                Some(&format!("{}: {error}", path.display())),
            ),
        }
        ctx.set_param(param, ParamValue::Bool(false));
        true
    }

    pub(super) fn sync_external_formula_file(&mut self, ctx: &mut ProcessCtx) -> bool {
        if !self.is_external_file_formula() {
            return false;
        }
        let Some(path) = self.external_formula_file_path(ctx) else {
            ctx.clear_node_warning(self.id(), Some(FORMULA_EXTERNAL_FILE_WARNING_ID));
            return false;
        };
        match FormulaCatalog::external_formula_tree_from_file(&path) {
            Ok(tree) => {
                ctx.clear_node_warning(self.id(), Some(FORMULA_EXTERNAL_FILE_WARNING_ID));
                self.replace_external_formula_contents(ctx, tree);
                true
            }
            Err(error) => {
                ctx.set_node_warning_with(
                    self.id(),
                    Some(FORMULA_EXTERNAL_FILE_WARNING_ID),
                    "External formula file could not be loaded",
                    Some(&format!("{}: {error}", path.display())),
                );
                false
            }
        }
    }

    pub(super) fn replace_external_formula_contents(&mut self, ctx: &mut ProcessCtx, imported: NodeTree) {
        self.sync_external_formula_label(ctx, &imported);
        self.replace_formula_contents(ctx, imported);
    }

    pub(super) fn sync_external_formula_label(&self, ctx: &mut ProcessCtx, imported: &NodeTree) {
        let label = imported.node.node_data().meta.label.trim();
        if label.is_empty() || label == self.node_data().meta.label {
            return;
        }
        ctx.patch_node_meta(
            self.id(),
            NodeMetaPatch {
                label: Some(label.to_owned()),
                ..NodeMetaPatch::default()
            },
        );
    }

    pub(super) fn replace_formula_contents(&mut self, ctx: &mut ProcessCtx, imported: NodeTree) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let children = imported_formula_content_children(imported);
        for child in snapshot.child_ids(self.id()) {
            let Some(node) = snapshot.node(child) else {
                continue;
            };
            if preserve_external_formula_child(node.decl_id.as_str()) {
                continue;
            }
            ctx.edits.push(Edit::RemoveNode { node: child });
        }
        self.managed_regions_json.set(ctx, String::new());
        for child in children {
            ctx.edits.push(Edit::AddNodeTree {
                tree: child,
                parent: self.id(),
                prev_sibling: None,
            });
        }
        self.schedule_formula_reconcile(ctx);
    }

    pub(super) fn schedule_formula_reconcile(&self, ctx: &mut ProcessCtx) {
        ctx.edits.push(Edit::CallNodeMutation {
            node: self.id(),
            callback: Box::new(|node, ctx| {
                let Some(formula) = node
                    .as_any_mut()
                    .downcast_mut::<AlchemistFormulaDefinition>()
                else {
                    return Ok(());
                };
                formula.reconcile_properties(ctx);
                formula.sync_property_getters(ctx);
                let materialized_formula = formula.sync_anode_sockets(ctx, None);
                formula.validate(ctx, materialized_formula);
                formula.enforce_external_formula_permissions(ctx);
                Ok(())
            }),
            needs_tree_snapshot: true,
        });
    }

    pub(super) fn enforce_external_formula_permissions(&self, ctx: &mut ProcessCtx) {
        if !self.is_read_only_external_formula() {
            return;
        }
        self.enforce_external_formula_subtree_permissions(ctx, self.id());
    }

    pub(super) fn schedule_external_formula_permission_enforcement(&self, ctx: &mut ProcessCtx) {
        if !self.is_read_only_external_formula() {
            return;
        }
        ctx.edits.push(Edit::CallNodeMutation {
            node: self.id(),
            callback: Box::new(|node, ctx| {
                if let Some(formula) =
                    node.as_any_mut()
                        .downcast_mut::<AlchemistFormulaDefinition>()
                {
                    formula.enforce_external_formula_permissions(ctx);
                } else if let Some(AppNode::AlchemistFormulaDefinition(formula)) =
                    node.as_any_mut().downcast_mut::<AppNode>()
                {
                    formula.enforce_external_formula_permissions(ctx);
                }
                Ok(())
            }),
            needs_tree_snapshot: true,
        });
    }

    pub(super) fn enforce_external_formula_subtree_permissions(&self, ctx: &mut ProcessCtx, root: NodeId) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            Self::enforce_external_formula_node_permissions(ctx, root);
            return;
        };
        let mut pending = vec![root];
        while let Some(node_id) = pending.pop() {
            if snapshot.node(node_id).is_none() {
                continue;
            }
            pending.extend(snapshot.child_ids(node_id));
            Self::enforce_external_formula_node_permissions(ctx, node_id);
        }
    }

    pub(super) fn enforce_external_formula_node_permissions(ctx: &mut ProcessCtx, node_id: NodeId) {
        ctx.edits.push(Edit::CallNodeMutation {
            node: node_id,
            callback: Box::new(|node, _ctx| {
                if let Some(parameter) = node.as_any_mut().downcast_mut::<Parameter>() {
                    parameter.read_only = true;
                } else if let Some(AppNode::Parameter(parameter)) =
                    node.as_any_mut().downcast_mut::<AppNode>()
                {
                    parameter.read_only = true;
                }
                Ok(())
            }),
            needs_tree_snapshot: false,
        });
        ctx.patch_node_meta(
            node_id,
            NodeMetaPatch {
                can_be_disabled: Some(false),
                user_permissions: Some(NodeUserPermissions::none()),
                ..NodeMetaPatch::default()
            },
        );
    }
}
