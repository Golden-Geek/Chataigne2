use super::*;

pub(super) enum ParentResolution<'a> {
    Ready { parent_id: NodeId, leaf_name: &'a str },
    Retry,
    Ignored,
}

pub(super) fn resolve_or_create_parent<'a>(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    values_id: NodeId,
    segments: &'a [String],
    auto_add: bool,
) -> ParentResolution<'a> {
    let mut current = values_id;
    let Some((leaf_name, parents)) = segments.split_last() else {
        return ParentResolution::Ignored;
    };

    for (index, segment) in parents.iter().enumerate() {
        match snapshot.find_child(current, segment.as_str()) {
            Some(child_id) => {
                let Some(child_snapshot) = snapshot.node(child_id) else {
                    return ParentResolution::Ignored;
                };
                if child_snapshot.node_type == "folder" {
                    current = child_id;
                    continue;
                }
                if !auto_add {
                    return ParentResolution::Ignored;
                }

                ctx.replace_node_boxed(child_id, Box::new(create_auto_values_folder(segment.as_str())));
                return ParentResolution::Retry;
            }
            None => {
                if !auto_add {
                    return ParentResolution::Ignored;
                }

                ctx.add_child_tree(current, folder_path_tree(&parents[index..]), None);
                return ParentResolution::Retry;
            }
        }
    }

    ParentResolution::Ready {
        parent_id: current,
        leaf_name: leaf_name.as_str(),
    }
}

pub(super) fn apply_single_value_message(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    parent_id: NodeId,
    leaf_name: &str,
    value: &ParamValue,
    options: ReceivedValueApplyOptions,
) -> ReceivedValueApplyResult {
    match snapshot.find_child(parent_id, leaf_name) {
        Some(existing_id) => {
            let Some(existing_snapshot) = snapshot.node(existing_id) else {
                return ReceivedValueApplyResult::Ignored;
            };

            if let Some(existing_value) = existing_snapshot.param_value.as_ref() {
                if param_types_match(existing_value, value) {
                    if param_update_needed(existing_value, value) {
                        ctx.set_param_with_behaviour(existing_id, value.clone(), options.event_behaviour);
                    }
                    return ReceivedValueApplyResult::applied(false);
                } else if options.auto_add {
                    ctx.replace_node_boxed(
                        existing_id,
                        Box::new(create_parameter_node(
                            leaf_name,
                            value.clone(),
                            None, //(format!("Auto-created from {}", options.source_description)),
                        )),
                    );
                    return ReceivedValueApplyResult::applied(true);
                }
            } else if options.auto_add {
                ctx.replace_node_boxed(
                    existing_id,
                    Box::new(create_parameter_node(
                        leaf_name,
                        value.clone(),
                        None, //(format!("Auto-created from {}", options.source_description)),
                    )),
                );
                return ReceivedValueApplyResult::applied(true);
            }

            ReceivedValueApplyResult::applied(false)
        }
        None => {
            if !options.auto_add {
                return ReceivedValueApplyResult::Ignored;
            }

            ctx.add_child_boxed(
                parent_id,
                Box::new(create_parameter_node(
                    leaf_name,
                    value.clone(),
                    None, //(format!("Auto-created from {}", options.source_description)),
                )),
                None,
            );
            ReceivedValueApplyResult::applied(true)
        }
    }
}

pub(super) fn apply_multi_value_message(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    parent_id: NodeId,
    leaf_name: &str,
    values: &[ParamValue],
    options: ReceivedValueApplyOptions,
) -> ReceivedValueApplyResult {
    let folder_id = match snapshot.find_child(parent_id, leaf_name) {
        Some(existing_id) => {
            let Some(existing_snapshot) = snapshot.node(existing_id) else {
                return ReceivedValueApplyResult::Ignored;
            };
            if existing_snapshot.node_type == "folder" {
                existing_id
            } else {
                if !options.auto_add {
                    return ReceivedValueApplyResult::Ignored;
                }

                ctx.replace_node_boxed(existing_id, Box::new(create_auto_values_folder(leaf_name)));
                return ReceivedValueApplyResult::Retry;
            }
        }
        None => {
            if !options.auto_add {
                return ReceivedValueApplyResult::Ignored;
            }

            ctx.add_child_tree(parent_id, multi_value_folder_tree(leaf_name, values), None);
            return ReceivedValueApplyResult::applied(true);
        }
    };

    ReceivedValueApplyResult::applied(sync_multi_value_folder(ctx, snapshot, folder_id, values, options))
}

pub(super) fn sync_multi_value_folder(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    folder_id: NodeId,
    values: &[ParamValue],
    options: ReceivedValueApplyOptions,
) -> bool {
    let mut structure_changed = false;

    for (index, value) in values.iter().enumerate() {
        let label = indexed_value_label(index);
        match snapshot.find_child(folder_id, label.as_str()) {
            Some(existing_id) => {
                let Some(existing_snapshot) = snapshot.node(existing_id) else {
                    continue;
                };

                if let Some(existing_value) = existing_snapshot.param_value.as_ref() {
                    if param_types_match(existing_value, value) {
                        if param_update_needed(existing_value, value) {
                            ctx.set_param_with_behaviour(existing_id, value.clone(), options.event_behaviour);
                        }
                    } else if options.auto_add {
                        ctx.replace_node_boxed(
                            existing_id,
                            Box::new(create_parameter_node(label.as_str(), value.clone(), None)),
                        );
                        structure_changed = true;
                    }
                } else if options.auto_add {
                    ctx.replace_node_boxed(
                        existing_id,
                        Box::new(create_parameter_node(label.as_str(), value.clone(), None)),
                    );
                    structure_changed = true;
                }
            }
            None => {
                if !options.auto_add {
                    continue;
                }

                ctx.add_child_boxed(
                    folder_id,
                    Box::new(create_parameter_node(label.as_str(), value.clone(), None)),
                    None,
                );
                structure_changed = true;
            }
        }
    }

    for child_id in snapshot.child_ids(folder_id) {
        let Some(child_snapshot) = snapshot.node(child_id) else {
            continue;
        };
        if let Some(index) = indexed_value_label_index(child_snapshot.label.as_str()) {
            if index >= values.len() {
                ctx.edits.push(Edit::RemoveNode { node: child_id });
                structure_changed = true;
            }
        }
    }

    structure_changed
}
