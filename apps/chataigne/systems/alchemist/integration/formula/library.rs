use super::*;

#[node("alchemist_formula_library", label = "Formulas")]
pub struct FormulaLibrary {}

struct SharedFormulaWatcher {
    directory: PathBuf,
    dirty: Arc<AtomicBool>,
    _watcher: RecommendedWatcher,
}

static SHARED_FORMULA_WATCHER: OnceLock<Mutex<Option<SharedFormulaWatcher>>> =
    OnceLock::new();

fn shared_formula_watcher_state() -> &'static Mutex<Option<SharedFormulaWatcher>> {
    SHARED_FORMULA_WATCHER.get_or_init(|| Mutex::new(None))
}

fn ensure_shared_formula_watcher(directory: PathBuf) -> Result<(), String> {
    fs::create_dir_all(&directory)
        .map_err(|error| format!("failed to create shared formulas folder: {error}"))?;

    let mut state = shared_formula_watcher_state()
        .lock()
        .map_err(|_| "shared formula watcher lock is poisoned".to_owned())?;
    if state
        .as_ref()
        .is_some_and(|watcher| watcher.directory == directory)
    {
        return Ok(());
    }

    let dirty = Arc::new(AtomicBool::new(true));
    let dirty_flag = Arc::clone(&dirty);
    let mut watcher = RecommendedWatcher::new(
        move |result: Result<notify::Event, notify::Error>| {
            if result.is_ok() {
                dirty_flag.store(true, Ordering::Relaxed);
            }
        },
        Config::default(),
    )
    .map_err(|error| format!("failed to create shared formula watcher: {error}"))?;
    watcher
        .watch(&directory, RecursiveMode::NonRecursive)
        .map_err(|error| format!("failed to watch shared formulas folder: {error}"))?;

    *state = Some(SharedFormulaWatcher {
        directory,
        dirty,
        _watcher: watcher,
    });
    Ok(())
}

fn shared_formula_watcher_has_pending() -> bool {
    shared_formula_watcher_state()
        .lock()
        .ok()
        .and_then(|state| {
            state
                .as_ref()
                .map(|watcher| watcher.dirty.load(Ordering::Relaxed))
        })
        .unwrap_or(false)
}

fn take_shared_formula_watcher_pending() -> bool {
    shared_formula_watcher_state()
        .lock()
        .ok()
        .and_then(|state| {
            state
                .as_ref()
                .map(|watcher| watcher.dirty.swap(false, Ordering::Relaxed))
        })
        .unwrap_or(false)
}

#[cfg(test)]
pub(crate) fn reset_shared_formula_watcher_for_test() {
    let mut state = shared_formula_watcher_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *state = None;
}

#[node("alchemist_formula_library", from_struct)]
impl Node for FormulaLibrary {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(formula_container_rules())
    }

    fn user_container_accepts_item(
        &self,
        item_type: &str,
        item_kind: &str,
    ) -> bool {
        formula_container_accepts(item_type, item_kind)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        formula_container_creatable_items()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        create_formula_container_item(node_type)
    }

    fn create_user_item_tree(&self, node_type: &str) -> Option<NodeTree> {
        create_formula_container_item_tree(node_type)
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        let mut permissions = NodeUserPermissions::all();
        permissions.can_remove_and_duplicate = false;
        self.node_data_mut().meta.user_permissions = permissions;
    }

    fn on_node_ready(
        &mut self,
        ctx: &mut ProcessCtx,
        _context: NodeCreationContext,
    ) {
        self.reconcile_shared_formula_dir_parameter(ctx);
        self.ensure_shared_formula_watcher(ctx);
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        if !take_shared_formula_watcher_pending() {
            return;
        }
        self.ensure_shared_formula_watcher(ctx);
        self.sync_shared_formula_files(ctx);
    }

    fn needs_update(&self) -> bool {
        shared_formula_watcher_has_pending()
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(FORMULA_LIBRARY_FILE_WATCH_RATE_HZ)
            .with_compiled_kernel("chataigne.runtime.formula-library")
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        shared_formula_watcher_has_pending()
    }
}

const FORMULA_LIBRARY_SHARED_DIR_DECL_ID: &str = "shared_formula_dir";

fn shared_formula_dir_parameter(path: String) -> Parameter {
    parameter(
        "Shared Formulas Folder",
        FORMULA_LIBRARY_SHARED_DIR_DECL_ID,
        ParamValue::Str(path),
        true,
    )
}

impl FormulaLibrary {
    /// Exposes the resolved shared-formulas folder as a read-only parameter
    /// so the frontend can tell whether an external-file-linked formula's
    /// path is inside it (i.e. is "Shared") without hardcoding the app-data
    /// path convention on the client.
    fn reconcile_shared_formula_dir_parameter(&self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let path = shared_formula_dir_from_snapshot(snapshot.as_ref())
            .map(|dir| dir.to_string_lossy().into_owned())
            .unwrap_or_default();
        if snapshot
            .find_child_by_decl_id(self.id(), FORMULA_LIBRARY_SHARED_DIR_DECL_ID)
            .is_none()
            && !child_add_pending(ctx, self.id(), FORMULA_LIBRARY_SHARED_DIR_DECL_ID)
        {
            ctx.add_child(self.id(), shared_formula_dir_parameter(path), None);
        }
    }

    fn ensure_shared_formula_watcher(&self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let Some(path) = shared_formula_dir_from_snapshot(snapshot.as_ref()) else {
            return;
        };
        match ensure_shared_formula_watcher(path) {
            Ok(()) => ctx.clear_node_warning(self.id(), Some(FORMULA_EXTERNAL_FILE_WARNING_ID)),
            Err(error) => ctx.set_node_warning_with(
                self.id(),
                Some(FORMULA_EXTERNAL_FILE_WARNING_ID),
                "Shared formulas folder could not be watched",
                Some(&error),
            ),
        }
    }

    fn sync_shared_formula_files(&self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let Some(shared_dir) = shared_formula_dir_from_snapshot(snapshot.as_ref()) else {
            return;
        };
        let mut renamed_nodes = HashSet::new();
        let mut renamed_paths = HashSet::new();
        match FormulaCatalog::renamed_shared_formula_paths(
            snapshot.as_ref(),
            self.id(),
            &shared_dir,
        ) {
            Ok(renames) => {
                for rename in renames {
                    let Some(file_param) =
                        snapshot.find_child_by_decl_id(rename.node, FORMULA_EXTERNAL_FILE_DECL_ID)
                    else {
                        continue;
                    };
                    renamed_nodes.insert(rename.node);
                    renamed_paths.insert(rename.path.clone());
                    ctx.edits.push(Edit::SetParam {
                        node: file_param,
                        value: ParamValue::File(rename.path.to_string_lossy().into_owned()),
                        behaviour: ParameterEventBehaviour::Coalesce,
                    });
                    if snapshot
                        .node(rename.node)
                        .is_some_and(|node| node.label != rename.label)
                    {
                        ctx.patch_node_meta(
                            rename.node,
                            NodeMetaPatch {
                                label: Some(rename.label),
                                ..NodeMetaPatch::default()
                            },
                        );
                    }
                }
            }
            Err(error) => {
                ctx.set_node_warning_with(
                    self.id(),
                    Some(FORMULA_EXTERNAL_FILE_WARNING_ID),
                    "Shared formulas folder could not be read",
                    Some(&error.to_string()),
                );
                return;
            }
        }
        match FormulaCatalog::stale_shared_formula_nodes_excluding(
            snapshot.as_ref(),
            self.id(),
            &shared_dir,
            &renamed_nodes,
        ) {
            Ok(nodes) => {
                for node in nodes {
                    ctx.edits.push(Edit::RemoveNode { node });
                }
            }
            Err(error) => {
                ctx.set_node_warning_with(
                    self.id(),
                    Some(FORMULA_EXTERNAL_FILE_WARNING_ID),
                    "Shared formulas folder could not be read",
                    Some(&error.to_string()),
                );
                return;
            }
        }
        match FormulaCatalog::missing_shared_formula_trees_excluding_paths(
            snapshot.as_ref(),
            self.id(),
            &shared_dir,
            &renamed_paths,
        ) {
            Ok(trees) => {
                for tree in trees {
                    ctx.edits.push(Edit::AddNodeTree {
                        tree,
                        parent: self.id(),
                        prev_sibling: None,
                    });
                }
            }
            Err(error) => ctx.set_node_warning_with(
                self.id(),
                Some(FORMULA_EXTERNAL_FILE_WARNING_ID),
                "Shared formulas folder could not be read",
                Some(&error.to_string()),
            ),
        }
    }
}
