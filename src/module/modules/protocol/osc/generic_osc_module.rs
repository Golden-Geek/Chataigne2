use golden_core::{
    edit::Edit,
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    node::{Folder, Node, NodeCreationContext, NodeId, NodeMetaPatch, NodeScriptDescriptor},
    parameter::{ParamValue, Parameter, ParameterChangeCheck},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::{OscDecodedMessage, OscIncomingApplyResult, OscValuePayload};

const VALUE_LABEL_PREFIX: &str = "value ";

#[golden_core::node("osc_module", label = "OSC")]
pub struct GenericOscModule {
    base: crate::app::OscModuleBase,
}

impl GenericOscModule {
    pub fn create() -> Self {
        Self::new(crate::app::OscModuleBase::create())
    }

    #[cfg(test)]
    pub(crate) fn disable_transport_for_test(&mut self) {
        self.base.stop_transport();
        self.base.set_transport_dirty(false);
    }

    #[cfg(test)]
    pub(crate) fn enqueue_incoming_message_for_test(&mut self, message: OscDecodedMessage) {
        self.base.enqueue_incoming_message(message);
    }

    #[cfg(test)]
    pub(crate) fn auto_add_enabled_for_test(&self) -> bool {
        self.base.auto_add_enabled()
    }

    #[cfg(test)]
    pub(crate) fn has_pending_incoming_messages_for_test(&self) -> bool {
        self.base.has_pending_incoming_messages()
    }

    fn apply_incoming_message(
        base: &mut crate::app::OscModuleBase,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        message: &OscDecodedMessage,
    ) -> OscIncomingApplyResult {
        let Some(values_id) = base.values_id() else {
            return OscIncomingApplyResult::Ignored;
        };

        let segments = address_segments(message.address.as_str());
        if segments.is_empty() {
            return OscIncomingApplyResult::Ignored;
        }

        let auto_add = base.auto_add_enabled();
        let (parent_id, leaf_name) = match resolve_or_create_parent(ctx, snapshot, values_id, &segments, auto_add) {
            ParentResolution::Ready { parent_id, leaf_name } => (parent_id, leaf_name),
            ParentResolution::Retry => return OscIncomingApplyResult::Retry,
            ParentResolution::Ignored => return OscIncomingApplyResult::Ignored,
        };

        match &message.payload {
            OscValuePayload::Single(value) => apply_single_value_message(
                base,
                ctx,
                snapshot,
                parent_id,
                leaf_name,
                value.clone(),
                auto_add,
                message.address.as_str(),
            ),
            OscValuePayload::Multi(values) => {
                apply_multi_value_message(base, ctx, snapshot, parent_id, leaf_name, values, auto_add)
            }
            OscValuePayload::Arguments(_) => OscIncomingApplyResult::Ignored,
        }
    }
}

#[golden_core::item(
    "module",
    node = "osc_module",
    via = base,
    from_struct,
    menu_path = ["Network"]
)]
impl Node for GenericOscModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, context: NodeCreationContext) {
        self.base.on_node_ready(ctx, context);
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.base.update(ctx);

        if !self.base.has_pending_incoming_messages() {
            return;
        }

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        self.base
            .process_pending_incoming(ctx, snapshot, Self::apply_incoming_message);
    }

    fn destroy(&mut self, ctx: &mut ProcessCtx) {
        self.base.destroy(ctx);
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        self.base.update_requires_tree_snapshot()
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        self.base.execution_rule()
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        self.base.child_event_interest_depth(event)
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        self.base.script_descriptor_for_node(self.node_data(), self.get_type())
    }

    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        self.base.engine_call_script_method(ctx, method, args)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        self.base.on_param_change(ctx, param, old_value);
    }

    fn on_meta_changed(&mut self, ctx: &mut ProcessCtx, node: NodeId, patch: NodeMetaPatch) {
        self.base.on_meta_changed(ctx, node, patch);
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        self.base.on_effective_enabled_changed(ctx, enabled);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.base.on_custom_event(ctx, event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
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
    base: &mut crate::app::OscModuleBase,
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    parent_id: NodeId,
    leaf_name: String,
    value: ParamValue,
    auto_add: bool,
    address: &str,
) -> OscIncomingApplyResult {
    match snapshot.find_child(parent_id, leaf_name.as_str()) {
        Some(existing_id) => {
            let Some(existing_snapshot) = snapshot.node(existing_id) else {
                return OscIncomingApplyResult::Ignored;
            };

            if let Some(existing_value) = existing_snapshot.param_value.as_ref() {
                if param_types_match(existing_value, &value) {
                    base.set_internal_param(ctx, existing_id, value);
                } else if auto_add {
                    ctx.replace_node_boxed(
                        existing_id,
                        Box::new(create_parameter_node(
                            leaf_name.as_str(),
                            value,
                            Some(format!("Auto-created from OSC address '{address}'")),
                        )),
                    );
                }
            } else if auto_add {
                ctx.replace_node_boxed(
                    existing_id,
                    Box::new(create_parameter_node(
                        leaf_name.as_str(),
                        value,
                        Some(format!("Auto-created from OSC address '{address}'")),
                    )),
                );
            }

            OscIncomingApplyResult::Applied
        }
        None => {
            if !auto_add {
                return OscIncomingApplyResult::Ignored;
            }

            ctx.add_child_boxed(
                parent_id,
                Box::new(create_parameter_node(
                    leaf_name.as_str(),
                    value,
                    Some(format!("Auto-created from OSC address '{address}'")),
                )),
                None,
            );
            OscIncomingApplyResult::Applied
        }
    }
}

fn apply_multi_value_message(
    base: &mut crate::app::OscModuleBase,
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    parent_id: NodeId,
    leaf_name: String,
    values: &[ParamValue],
    auto_add: bool,
) -> OscIncomingApplyResult {
    let folder_id = match snapshot.find_child(parent_id, leaf_name.as_str()) {
        Some(existing_id) => {
            let Some(existing_snapshot) = snapshot.node(existing_id) else {
                return OscIncomingApplyResult::Ignored;
            };
            if existing_snapshot.node_type == "folder" {
                existing_id
            } else {
                if !auto_add {
                    return OscIncomingApplyResult::Ignored;
                }

                ctx.replace_node_boxed(existing_id, Box::new(create_auto_values_folder(leaf_name.as_str())));
                return OscIncomingApplyResult::Retry;
            }
        }
        None => {
            if !auto_add {
                return OscIncomingApplyResult::Ignored;
            }

            ctx.add_child_boxed(parent_id, Box::new(create_auto_values_folder(leaf_name.as_str())), None);
            return OscIncomingApplyResult::Retry;
        }
    };

    sync_multi_value_folder(base, ctx, snapshot, folder_id, values, auto_add);
    OscIncomingApplyResult::Applied
}

fn sync_multi_value_folder(
    base: &mut crate::app::OscModuleBase,
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    folder_id: NodeId,
    values: &[ParamValue],
    auto_add: bool,
) {
    for (index, value) in values.iter().enumerate() {
        let label = indexed_value_label(index);
        match snapshot.find_child(folder_id, label.as_str()) {
            Some(existing_id) => {
                let Some(existing_snapshot) = snapshot.node(existing_id) else {
                    continue;
                };

                if let Some(existing_value) = existing_snapshot.param_value.as_ref() {
                    if param_types_match(existing_value, value) {
                        base.set_internal_param(ctx, existing_id, value.clone());
                    } else if auto_add {
                        ctx.replace_node_boxed(
                            existing_id,
                            Box::new(create_parameter_node(label.as_str(), value.clone(), None)),
                        );
                    }
                } else if auto_add {
                    ctx.replace_node_boxed(
                        existing_id,
                        Box::new(create_parameter_node(label.as_str(), value.clone(), None)),
                    );
                }
            }
            None => {
                if !auto_add {
                    continue;
                }

                ctx.add_child_boxed(
                    folder_id,
                    Box::new(create_parameter_node(label.as_str(), value.clone(), None)),
                    None,
                );
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
            }
        }
    }
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

fn address_segments(address: &str) -> Vec<String> {
    address
        .split('/')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
        .collect()
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

#[cfg(test)]
mod generic_osc_module_tests;
