use super::duplicate::PreparedProjectSubtree;
use super::*;
use crate::parameter::ParamValue;

#[cfg(test)]
mod tests;

/// One complete persisted root and explicit parameter overrides applied before readiness.
pub struct ProjectImportSpec {
    /// Complete dense authored subtree, including every declared child identity.
    pub project: ProjectFile,
    /// Direct parameter placement/configuration overrides.
    pub initial_params: Vec<(DeclId, ParamValue)>,
}

/// Exact identity of one imported record after successful lifecycle completion.
#[derive(Clone, Debug)]
pub struct ProjectImportedIdentity {
    /// Identity in the input document.
    pub source_uuid: NodeUuid,
    /// New persistent identity.
    pub uuid: NodeUuid,
    /// New live engine identity.
    pub node: NodeId,
}

/// Authoritative result of a complete forest import.
pub struct ProjectImportResult {
    /// Created roots in input order.
    pub roots: Vec<NodeId>,
    /// Exact record mappings, ordered by source UUID.
    pub identities: Vec<ProjectImportedIdentity>,
}

impl<T: Node> Engine<T> {
    /// Captures a complete dense authored subtree without capturing the rest of the engine.
    pub fn capture_project_subtree_with<F>(
        &self,
        root: NodeId,
        mut encode: F,
    ) -> Result<ProjectFile, ProjectPersistenceError>
    where
        F: FnMut(&T) -> Result<serde_json::Value, String>,
    {
        Ok(ProjectFile {
            version: PROJECT_FILE_VERSION.into(),
            ui_state: None,
            root: self.encode_node_record_with(root, &mut encode)?,
        })
    }

    /// Imports a complete forest with one UUID map, detached decode and lifecycle batch.
    ///
    /// Cross-root references follow the shared map. External UUIDs retain their identity;
    /// cached runtime reference IDs/names are cleared by the normal decoder. Root labels
    /// reserve against live siblings and every earlier imported root. The caller owns
    /// domain/catalog admission; any codec, initialization or identity failure rolls back.
    pub fn import_project_subtrees_with<F, A>(
        &mut self,
        imports: Vec<ProjectImportSpec>,
        parent: NodeId,
        prev_sibling: Option<NodeId>,
        mut decode: F,
        mut admit: A,
    ) -> Result<ProjectImportResult, ProjectPersistenceError>
    where
        F: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
        A: FnMut(&T, &T) -> Result<(), String>,
    {
        const OPERATION: &str = "ImportProjectSubtrees";
        self.validate_persisted_subtree_destination(parent, prev_sibling, OPERATION)?;
        let mut source_uuids = HashSet::new();
        for import in &imports {
            golden_persistence::validate_project_document_version(&import.project)?;
            collect_record_uuids(&import.project.root, &mut source_uuids)?;
        }
        if source_uuids.iter().any(|uuid| uuid.0.is_nil()) {
            return Err(ProjectPersistenceError::Codec {
                node_type: "import".into(),
                message: "imported records require non-nil UUIDs".into(),
            });
        }
        let mut imports = imports;
        let mut uuid_map = HashMap::with_capacity(source_uuids.len());
        for import in &mut imports {
            remap_record_uuids(&mut import.project.root, &mut uuid_map);
        }
        let mut labels = HashMap::new();
        let mut prepared = Vec::with_capacity(imports.len());
        for import in imports {
            let mut record = import.project.root;
            let preferred = record.meta.label.as_deref().unwrap_or(&record.node_type);
            let label = self.reserve_child_label(parent, preferred, &mut labels);
            let short_name = generate_short_name(&label);
            record.meta.label = Some(label);
            record.meta.short_name = Some(short_name.clone());
            record.meta.decl_id = Some(DeclId(short_name));
            let parent_node = self
                .nodes
                .get(parent)
                .ok_or(ProjectPersistenceError::MissingNode(parent))?;
            let tree = Self::decode_project_record_tree_with(parent_node, &record, &uuid_map, &mut decode)?;
            fn admit_tree<T, A>(
                parent: &T,
                tree: &DecodedProjectTree<T>,
                admit: &mut A,
            ) -> Result<(), ProjectPersistenceError>
            where
                T: Node,
                A: FnMut(&T, &T) -> Result<(), String>,
            {
                admit(parent, &tree.node).map_err(|message| ProjectPersistenceError::Codec {
                    node_type: tree.node.get_type().into(),
                    message,
                })?;
                for child in &tree.children {
                    admit_tree(&tree.node, child, admit)?;
                }
                Ok(())
            }
            admit_tree(parent_node, &tree, &mut admit)?;
            let tree = PreparedProjectSubtree::for_import(parent, prev_sibling, tree);
            let tree = self.prepare_project_subtree_initial_params(tree, import.initial_params, OPERATION)?;
            prepared.push((tree, NodeCreationContext::Duplicate));
        }
        self.apply_atomic_edit_operation(|engine| {
            let roots = engine.commit_imported_project_forest(prepared, OPERATION)?;
            let imported_uuids = uuid_map.values().copied().collect::<HashSet<_>>();
            for root in &roots {
                for node in engine.collect_loaded_subtree_node_ids(*root)? {
                    let uuid = engine.nodes.get(node).unwrap().node_data().meta.uuid;
                    if !imported_uuids.contains(&uuid) {
                        return Err(ProjectPersistenceError::Codec {
                            node_type: "import".into(),
                            message: format!(
                                "complete import omitted lifecycle child '{}' of type '{}'",
                                engine.nodes.get(node).unwrap().node_data().meta.decl_id.0,
                                engine.nodes.get(node).unwrap().get_type(),
                            ),
                        });
                    }
                }
            }
            let mut identities = uuid_map
                .into_iter()
                .map(|(source_uuid, uuid)| {
                    let node = engine
                        .uuid_index
                        .get(&uuid)
                        .copied()
                        .ok_or_else(|| ProjectPersistenceError::Codec {
                            node_type: "import".into(),
                            message: format!("import lifecycle retired identity {}", source_uuid.0),
                        })?;
                    Ok(ProjectImportedIdentity {
                        source_uuid,
                        uuid,
                        node,
                    })
                })
                .collect::<Result<Vec<_>, ProjectPersistenceError>>()?;
            identities.sort_by_key(|identity| identity.source_uuid.0.as_u128());
            Ok(ProjectImportResult { roots, identities })
        })
    }
}
