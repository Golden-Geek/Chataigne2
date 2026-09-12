use super::*;

impl AlchemistFormulaDefinition {
    pub(super) fn reconcile_properties(&self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        if snapshot
            .find_child_by_decl_id(self.id(), PROPERTIES_DECL_ID)
            .is_none()
        {
            ctx.add_child_tree(self.id(), properties_tree(), None);
        }
    }

    pub(super) fn remove_dangling_connections(&self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let anode_uuids = snapshot
            .child_ids(self.id())
            .into_iter()
            .filter_map(|child| {
                let node = snapshot.node(child)?;
                (node.node_type == ANODE_NODE_TYPE).then_some(node.uuid)
            })
            .collect::<HashSet<_>>();

        for child in snapshot.child_ids(self.id()) {
            let Some(node) = snapshot.node(child) else {
                continue;
            };
            if node.node_type != CONNECTION_NODE_TYPE {
                continue;
            }
            let source =
                child_reference_uuid(&snapshot, child, "source_node");
            let target =
                child_reference_uuid(&snapshot, child, "target_node");
            if (source.is_none_or(|uuid| !anode_uuids.contains(&uuid))
                || target.is_none_or(|uuid| !anode_uuids.contains(&uuid)))
                && !node_removal_pending(ctx, child)
            {
                ctx.edits.push(Edit::RemoveNode { node: child });
            }
        }
    }

    pub(super) fn sync_anode_sockets(&self, ctx: &mut ProcessCtx, skip_anode: Option<NodeId>) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let nodes = registry();
        for child in snapshot.child_ids(self.id()) {
            if Some(child) == skip_anode {
                continue;
            }
            let Some(anode) = snapshot.node(child) else {
                continue;
            };
            if anode.node_type != ANODE_NODE_TYPE {
                continue;
            }
            let Some(type_id) = anode_type_from_tags(&anode.tags) else {
                continue;
            };
            if nodes.get(&ANodeTypeId::new(&type_id)).is_none() {
                continue;
            }
            let Some(config_folder) =
                snapshot.find_child_by_decl_id(child, "config")
            else {
                continue;
            };
            sync_auto_input_count(ctx, &snapshot, child, config_folder, &type_id);
        }
        let Ok(formula) = formula_from_snapshot(&snapshot, self.id()) else {
            return;
        };
        let value_types = value_types();
        let solved = solve_document_types(
            &formula.graph,
            &TypeSolveCtx {
                value_types,
                nodes,
                properties: Some(&formula.properties),
            },
        );
        let signature_ctx = SignatureCtx {
            value_types,
            properties: Some(&formula.properties),
        };

        for child in snapshot.child_ids(self.id()) {
            if Some(child) == skip_anode {
                continue;
            }
            let Some(anode) = snapshot.node(child) else {
                continue;
            };
            if anode.node_type != ANODE_NODE_TYPE {
                continue;
            }
            let anode_id = ANodeId::from_uuid(anode.uuid.0);
            let Some(node) = formula.graph.node(AlchemistGraphDomain::node_id(anode_id)) else {
                continue;
            };
            let instance = node.data.to_instance(anode_id);
            let Some(declaration) = nodes.get(&instance.type_id) else {
                continue;
            };
            let config_folder = snapshot.find_child_by_decl_id(child, "config");
            let Some(inputs_folder) =
                snapshot.find_child_by_decl_id(child, "inputs")
            else {
                continue;
            };
            let Some(outputs_folder) =
                snapshot.find_child_by_decl_id(child, "outputs")
            else {
                continue;
            };
            let signature =
                declaration.signature(&signature_ctx, &instance, &instance.type_bindings);
            let fallback_bindings = local_signature_bindings(&signature, &instance);
            let resolved_node = solved.graph.nodes.get(&anode_id);
            let resolved_bindings = resolved_node
                .map(|node| &node.bindings)
                .unwrap_or(&fallback_bindings);

            if let Some(config_folder) = config_folder {
                sync_auto_type_variable_config_params(
                    ctx,
                    &snapshot,
                    config_folder,
                    declaration.as_ref(),
                    &signature,
                    value_types,
                    resolved_bindings,
                );
            }

            for input in signature.inputs {
                let value_type = resolved_node
                    .and_then(|node| node.signature.inputs.get(&input.id))
                    .and_then(|socket| socket.value_type.clone())
                    .unwrap_or_else(|| {
                        constraint_value_type(&input.constraint, &fallback_bindings)
                    });
                let default = instance
                    .input_defaults
                    .get(&input.id)
                    .cloned()
                    .or(input.default_value)
                    .and_then(|value| value.convert_to(&value_type).ok())
                    .or_else(|| default_runtime_value(&value_type).ok())
                    .unwrap_or(RuntimeValue::Float(0.0));
                let decl_id = socket_decl_id("inputs", input.id.as_str());
                let Some(existing) =
                    snapshot.find_child_by_decl_id(inputs_folder, &decl_id)
                else {
                    continue;
                };
                let needs_rebuild = !input_socket_matches(
                    &snapshot,
                    existing,
                    input.id.as_str(),
                    &value_type,
                    &default,
                );
                if !needs_rebuild {
                    continue;
                }
                let tree = input_socket_tree(
                    input.id.as_str(),
                    &input.label,
                    &value_type,
                    &default,
                );
                replace_child_tree_once(ctx, inputs_folder, Some(existing), tree);
            }

            for output in signature.outputs {
                let value_type = resolved_node
                    .and_then(|node| node.signature.outputs.get(&output.id))
                    .and_then(|socket| socket.value_type.clone())
                    .unwrap_or_else(|| {
                        constraint_value_type(&output.constraint, &fallback_bindings)
                    });
                let decl_id = socket_decl_id("outputs", output.id.as_str());
                let Some(existing) =
                    snapshot.find_child_by_decl_id(outputs_folder, &decl_id)
                else {
                    continue;
                };
                let needs_rebuild = !output_socket_matches(
                    &snapshot,
                    existing,
                    output.id.as_str(),
                    &value_type,
                );
                if !needs_rebuild {
                    continue;
                }
                replace_child_tree_once(
                    ctx,
                    outputs_folder,
                    Some(existing),
                    output_socket_tree(
                        output.id.as_str(),
                        &output.label,
                        &value_type,
                    ),
                );
            }
        }
    }

    pub(super) fn sync_property_getters(&self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let Some(properties) =
            snapshot.find_child_by_decl_id(self.id(), PROPERTIES_DECL_ID)
        else {
            return;
        };
        for child in snapshot.child_ids(self.id()) {
            let Some(anode) = snapshot.node(child) else {
                continue;
            };
            if anode.node_type != ANODE_NODE_TYPE {
                continue;
            };
            let Some(type_id) = anode_type_from_tags(&anode.tags) else {
                continue;
            };
            let Some(config_decl_id) = anode_property_ref_config_decl_id(type_id.as_str()) else {
                continue;
            };
            let Some(config) =
                snapshot.find_child_by_decl_id(child, "config")
            else {
                continue;
            };
            let Some(property_uuid) =
                child_reference_uuid(&snapshot, config, config_decl_id)
            else {
                continue;
            };
            let Some((label, color)) = property_meta_by_uuid(
                &snapshot,
                properties,
                property_uuid,
            )
            else {
                continue;
            };
            let label_changed = anode.label != label;
            let color_changed = anode.presentation.default_color != color;
            if label_changed || color_changed {
                let mut presentation = anode.presentation.clone();
                presentation.default_color = color;
                ctx.patch_node_meta(
                    child,
                    NodeMetaPatch {
                        label: label_changed.then_some(label),
                        presentation: color_changed.then_some(presentation),
                        ..NodeMetaPatch::default()
                    },
                );
            }
        }
    }

    pub(super) fn validate(&mut self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot() else {
            return;
        };
        let anode_node_ids = formula_anode_node_ids(snapshot, self.id());
        let mut node_diagnostics = HashMap::<NodeId, Vec<String>>::new();
        let result = formula_from_snapshot(snapshot, self.id()).map(|formula| {
            let value_types = value_types();
            let nodes = registry();
            compile_graph(
                &formula.graph,
                &CompileCtx {
                    value_types,
                    nodes,
                    properties: Some(&formula.properties),
                },
            )
        });
        let (valid, diagnostics) = match result {
            Ok(compilation) => {
                for diagnostic in &compilation.diagnostics {
                    if !matches!(
                        diagnostic.severity,
                        DiagnosticSeverity::Warning | DiagnosticSeverity::Error
                    ) {
                        continue;
                    }
                    if let Some(node_id) = diagnostic_origin_node_id(
                        &diagnostic.origin,
                        &anode_node_ids,
                    ) {
                        node_diagnostics
                            .entry(node_id)
                            .or_default()
                            .push(diagnostic.message.clone());
                    }
                }
                let diagnostics = compilation
                    .diagnostics
                    .iter()
                    .map(|diagnostic| {
                        serde_json::json!({
                            "code": diagnostic.code,
                            "message": diagnostic.message,
                            "severity": match diagnostic.severity {
                                DiagnosticSeverity::Info => "info",
                                DiagnosticSeverity::Warning => "warning",
                                DiagnosticSeverity::Error => "error",
                            },
                            "origin": format!("{:?}", diagnostic.origin),
                        })
                    })
                    .collect::<Vec<_>>();
                (!compilation.has_errors(), diagnostics)
            }
            Err(message) => (
                false,
                vec![serde_json::json!({
                    "code": "invalid_formula_tree",
                    "message": message,
                    "severity": "error",
                    "origin": "Formula",
                })],
            ),
        };
        let diagnostics_json = serde_json::to_string(&diagnostics)
            .expect("Formula diagnostics must serialize");
        let diagnostic_node_ids =
            node_diagnostics.keys().copied().collect::<HashSet<_>>();
        let anode_warnings_to_clear = anode_node_ids
            .values()
            .copied()
            .filter(|node_id| {
                !diagnostic_node_ids.contains(node_id)
                    && node_has_warning(
                        snapshot,
                        *node_id,
                        ANODE_FORMULA_DIAGNOSTIC_WARNING_ID,
                    )
            })
            .collect::<Vec<_>>();
        let anode_warnings_to_set = node_diagnostics
            .into_iter()
            .filter_map(|(node_id, messages)| {
                let detail = messages.join("\n");
                let title = if messages.len() > 1 {
                    "Formula issues"
                } else {
                    "Formula issue"
                };
                (!node_warning_matches(
                    snapshot,
                    node_id,
                    ANODE_FORMULA_DIAGNOSTIC_WARNING_ID,
                    title,
                    Some(&detail),
                ))
                .then_some((node_id, title, detail))
            })
            .collect::<Vec<_>>();
        let formula_warning_to_clear =
            valid && node_has_warning(snapshot, self.id(), FORMULA_WARNING_ID);
        let formula_warning_to_set = if valid {
            None
        } else {
            let detail = formula_warning_detail(&diagnostics);
            (!node_warning_matches(
                snapshot,
                self.id(),
                FORMULA_WARNING_ID,
                "Formula is invalid",
                Some(&detail),
            ))
            .then_some(detail)
        };
        self.is_valid.set(ctx, valid);
        self.diagnostics_json.set(ctx, diagnostics_json);
        for node_id in anode_warnings_to_clear {
            ctx.clear_node_warning(
                node_id,
                Some(ANODE_FORMULA_DIAGNOSTIC_WARNING_ID),
            );
        }
        for (node_id, title, detail) in anode_warnings_to_set {
            ctx.set_node_warning_with(
                node_id,
                Some(ANODE_FORMULA_DIAGNOSTIC_WARNING_ID),
                title,
                Some(&detail),
            );
        }
        if valid {
            if formula_warning_to_clear {
                ctx.clear_node_warning(self.id(), Some(FORMULA_WARNING_ID));
            }
        } else if let Some(detail) = formula_warning_to_set {
            ctx.set_node_warning_with(
                self.id(),
                Some(FORMULA_WARNING_ID),
                "Formula is invalid",
                Some(&detail),
            );
        }
    }
}
