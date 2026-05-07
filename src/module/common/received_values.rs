use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId},
    parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

const VALUE_LABEL_PREFIX: &str = "value ";

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ReceivedValuePayload {
    Single(ParamValue),
    Multi(Vec<ParamValue>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReceivedValueApplyResult {
    Applied {
        needs_snapshot_refresh: bool,
    },
    Retry,
    Ignored,
}

impl ReceivedValueApplyResult {
    pub(crate) fn applied(needs_snapshot_refresh: bool) -> Self {
        Self::Applied {
            needs_snapshot_refresh,
        }
    }
}

pub(crate) fn apply_received_value_payload(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    values_id: NodeId,
    path_segments: &[String],
    payload: &ReceivedValuePayload,
    options: ReceivedValueApplyOptions<'_>,
) -> ReceivedValueApplyResult {
    if path_segments.is_empty() {
        return ReceivedValueApplyResult::Ignored;
    }

    let (parent_id, leaf_name) =
        match resolve_or_create_parent(ctx, snapshot, values_id, path_segments, options.auto_add) {
            ParentResolution::Ready { parent_id, leaf_name } => (parent_id, leaf_name),
            ParentResolution::Retry => return ReceivedValueApplyResult::Retry,
            ParentResolution::Ignored => return ReceivedValueApplyResult::Ignored,
        };

    match payload {
        ReceivedValuePayload::Single(value) => apply_single_value_message(
            ctx,
            snapshot,
            parent_id,
            leaf_name,
            value.clone(),
            options,
        ),
        ReceivedValuePayload::Multi(values) => {
            apply_multi_value_message(ctx, snapshot, parent_id, leaf_name, values, options)
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ReceivedValueApplyOptions<'a> {
    pub(crate) auto_add: bool,
    pub(crate) source_description: &'a str,
    pub(crate) event_behaviour: ParameterEventBehaviour,
}

enum ParentResolution {
    Ready { parent_id: NodeId, leaf_name: String },
    Retry,
    Ignored,
}

fn resolve_or_create_parent(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    values_id: NodeId,
    segments: &[String],
    auto_add: bool,
) -> ParentResolution {
    let mut current = values_id;
    let Some((leaf_name, parents)) = segments.split_last() else {
        return ParentResolution::Ignored;
    };

    for segment in parents {
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

                ctx.add_child_boxed(current, Box::new(create_auto_values_folder(segment.as_str())), None);
                return ParentResolution::Retry;
            }
        }
    }

    ParentResolution::Ready {
        parent_id: current,
        leaf_name: leaf_name.clone(),
    }
}

fn apply_single_value_message(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    parent_id: NodeId,
    leaf_name: String,
    value: ParamValue,
    options: ReceivedValueApplyOptions<'_>,
) -> ReceivedValueApplyResult {
    match snapshot.find_child(parent_id, leaf_name.as_str()) {
        Some(existing_id) => {
            let Some(existing_snapshot) = snapshot.node(existing_id) else {
                return ReceivedValueApplyResult::Ignored;
            };

            if let Some(existing_value) = existing_snapshot.param_value.as_ref() {
                if param_types_match(existing_value, &value) {
                    ctx.set_param_with_behaviour(existing_id, value, options.event_behaviour);
                    return ReceivedValueApplyResult::applied(false);
                } else if options.auto_add {
                    ctx.replace_node_boxed(
                        existing_id,
                        Box::new(create_parameter_node(
                            leaf_name.as_str(),
                            value,
                            Some(format!("Auto-created from {}", options.source_description)),
                        )),
                    );
                    return ReceivedValueApplyResult::applied(true);
                }
            } else if options.auto_add {
                ctx.replace_node_boxed(
                    existing_id,
                    Box::new(create_parameter_node(
                        leaf_name.as_str(),
                        value,
                        Some(format!("Auto-created from {}", options.source_description)),
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
                    leaf_name.as_str(),
                    value,
                    Some(format!("Auto-created from {}", options.source_description)),
                )),
                None,
            );
            ReceivedValueApplyResult::applied(true)
        }
    }
}

fn apply_multi_value_message(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    parent_id: NodeId,
    leaf_name: String,
    values: &[ParamValue],
    options: ReceivedValueApplyOptions<'_>,
) -> ReceivedValueApplyResult {
    let folder_id = match snapshot.find_child(parent_id, leaf_name.as_str()) {
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

                ctx.replace_node_boxed(existing_id, Box::new(create_auto_values_folder(leaf_name.as_str())));
                return ReceivedValueApplyResult::Retry;
            }
        }
        None => {
            if !options.auto_add {
                return ReceivedValueApplyResult::Ignored;
            }

            ctx.add_child_boxed(parent_id, Box::new(create_auto_values_folder(leaf_name.as_str())), None);
            return ReceivedValueApplyResult::Retry;
        }
    };

    ReceivedValueApplyResult::applied(sync_multi_value_folder(ctx, snapshot, folder_id, values, options))
}

fn sync_multi_value_folder(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    folder_id: NodeId,
    values: &[ParamValue],
    options: ReceivedValueApplyOptions<'_>,
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
                        ctx.set_param_with_behaviour(existing_id, value.clone(), options.event_behaviour);
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

fn create_parameter_node(label: &str, value: ParamValue, description: Option<String>) -> Parameter {
    let mut parameter = Parameter::new(label, value, ParameterChangeCheck::ValueChange);
    parameter.node_data_mut().meta.description = description;
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    parameter
}

fn create_auto_values_folder(label: &str) -> Folder {
    let mut folder = Folder::new(label);
    crate::app::module::enable_module_authoring(folder.node_data_mut());
    folder
}

fn indexed_value_label(index: usize) -> String {
    format!("{VALUE_LABEL_PREFIX}{}", index + 1)
}

fn indexed_value_label_index(label: &str) -> Option<usize> {
    let suffix = label.strip_prefix(VALUE_LABEL_PREFIX)?;
    suffix.parse::<usize>().ok()?.checked_sub(1)
}

fn param_types_match(lhs: &ParamValue, rhs: &ParamValue) -> bool {
    std::mem::discriminant(lhs) == std::mem::discriminant(rhs)
}
