use super::*;

impl<T: Node> Engine<T> {
    /// Validate the whole ingress set before transferring any request into the live queue.
    pub(super) fn absorb_edit_requests(&mut self, requests: Vec<EditRequest>) -> Result<(), EngineEditError> {
        let mut validated = Vec::with_capacity(requests.len());
        let mut presentations = HashMap::new();
        for (index, request) in requests.into_iter().enumerate() {
            let EditRequest {
                edit,
                completions,
                param_source,
            } = request;
            let observed = !completions.is_empty();
            if let Some(edit) = self.validate_ingress_edit(index, edit, &mut presentations, observed)? {
                validated.push(EditRequest {
                    edit,
                    completions,
                    param_source,
                });
            } else {
                debug_assert!(!observed, "observed no-ops must settle at application");
            }
        }
        for request in validated {
            self.edits.push_request(request);
        }
        Ok(())
    }

    fn validate_ingress_edit(
        &self,
        index: usize,
        edit: Edit,
        presentations: &mut HashMap<NodeId, PresentationHint>,
        observed: bool,
    ) -> Result<Option<Edit>, EngineEditError> {
        let edit = match edit {
            Edit::AddNode {
                node,
                parent,
                prev_sibling,
            } => Edit::AddNode {
                node: Box::new(self.coerce_node_for_engine(index, "AddNode", node)?),
                parent,
                prev_sibling,
            },
            Edit::AddNodeTree {
                tree,
                parent,
                prev_sibling,
            } => Edit::AddNodeTree {
                tree: self.coerce_node_tree_for_engine(index, "AddNodeTree", tree)?,
                parent,
                prev_sibling,
            },
            Edit::AddUserItemTree {
                tree,
                parent,
                prev_sibling,
            } => Edit::AddUserItemTree {
                tree: self.coerce_node_tree_for_engine(index, "AddUserItemTree", tree)?,
                parent,
                prev_sibling,
            },
            Edit::AddUserItemTrees { items } => Edit::AddUserItemTrees {
                items: items
                    .into_iter()
                    .map(|item| {
                        Ok(UserItemTreeInsertion {
                            parent: item.parent,
                            tree: self.coerce_node_tree_for_engine(index, "AddUserItemTrees", item.tree)?,
                        })
                    })
                    .collect::<Result<_, EngineEditError>>()?,
            },
            Edit::AddUserItem {
                node,
                parent,
                prev_sibling,
            } => Edit::AddUserItem {
                node: Box::new(self.coerce_node_for_engine(index, "AddUserItem", node)?),
                parent,
                prev_sibling,
            },
            Edit::ReplaceNode { node, new_node } => Edit::ReplaceNode {
                node,
                new_node: Box::new(self.coerce_node_for_engine(index, "ReplaceNode", new_node)?),
            },
            Edit::SetNodeWarning { node, warning } => {
                if let Some(mut next) = self.ingress_presentation(node, presentations) {
                    let previous = next.clone();
                    next.set_warning(warning.clone());
                    if previous == next && !observed {
                        return Ok(None);
                    }
                    presentations.insert(node, next);
                }
                Edit::SetNodeWarning { node, warning }
            }
            Edit::ClearNodeWarning { node, warning_id } => {
                if let Some(mut next) = self.ingress_presentation(node, presentations) {
                    let changed = match warning_id.as_deref() {
                        Some(id) => next.clear_warning(Some(id)),
                        None => next.clear_warnings(),
                    };
                    if !changed && !observed {
                        return Ok(None);
                    }
                    presentations.insert(node, next);
                }
                Edit::ClearNodeWarning { node, warning_id }
            }
            Edit::SetNodeChildWarningDepth { node, max_depth } => {
                if let Some(mut next) = self.ingress_presentation(node, presentations) {
                    if !next.set_child_warning_depth(max_depth) && !observed {
                        return Ok(None);
                    }
                    presentations.insert(node, next);
                }
                Edit::SetNodeChildWarningDepth { node, max_depth }
            }
            edit => edit,
        };
        Ok(Some(edit))
    }

    fn ingress_presentation(
        &self,
        node: NodeId,
        presentations: &HashMap<NodeId, PresentationHint>,
    ) -> Option<PresentationHint> {
        presentations.get(&node).cloned().or_else(|| {
            self.nodes
                .get(node)
                .map(|entry| entry.node_data().meta.presentation.clone())
        })
    }

    fn coerce_node_tree_for_engine(
        &self,
        edit_index: usize,
        operation: &'static str,
        tree: NodeTree,
    ) -> Result<NodeTree, EngineEditError> {
        let user_role = tree.user_role;
        let node = self.coerce_node_for_engine(edit_index, operation, tree.node)?;
        let mut coerced = NodeTree::new(node);
        coerced.user_role = user_role;
        for child in tree.children {
            coerced.push_child(self.coerce_node_tree_for_engine(edit_index, operation, child)?);
        }
        Ok(coerced)
    }
}
