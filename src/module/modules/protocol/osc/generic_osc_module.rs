use std::collections::HashSet;

#[path = "generic_osc_module/osc_message.rs"]
mod osc_message;
#[path = "generic_osc_module/osc_runtime.rs"]
mod osc_runtime;

use golden_core::{
    edit::Edit, events::Event, logerror, node::{Folder, Node, NodeId}, parameter::{ParamValue, Parameter, ParameterChangeCheck}, process_ctx::{ProcessCtx, ProcessTreeSnapshot}
};

use self::osc_message::{OscDecodedMessage, OscValuePayload};
use self::osc_runtime::{
    OscOutboundMessage, OscTransportConfig, OscTransportHandle, OscWorkerEvent,
};

const GENERIC_OSC_MODULE_NODE_TYPE: &str = "generic_osc_module";
const OSC_OUTPUT_NODE_TYPE: &str = "osc_output";
const VALUE_LABEL_PREFIX: &str = "value ";

#[golden_core::node("generic_osc_module", label = "Generic OSC Module")]
pub struct GenericOscModule {
    base: crate::app::OscModuleBase,
    transport: Option<OscTransportHandle>,
    last_transport_config: Option<OscTransportConfig>,
    transport_dirty: bool,
    default_output_checked: bool,
    ignored_param_changes: HashSet<NodeId>,
    pending_outbound_nodes: HashSet<NodeId>,
    pending_incoming_messages: Vec<OscDecodedMessage>,
}

impl GenericOscModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::OscModuleBase::create(),
            None,
            None,
            true,
            false,
            HashSet::new(),
            HashSet::new(),
            Vec::new(),
        )
    }

    fn refresh_transport(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.transport_dirty = false;

        match self.read_transport_config(snapshot) {
            Ok(config) => {
                if self.transport.is_some() && self.last_transport_config.as_ref() == Some(&config) {
                    return;
                }

                self.stop_transport();

                match OscTransportHandle::spawn(config.clone()) {
                    Ok(handle) => {
                        self.transport = Some(handle);
                        self.last_transport_config = Some(config);
                        self.set_connected(ctx, snapshot, true);
                    }
                    Err(error) => {
                        logerror!("Failed to start OSC transport: {}", error);
                        self.transport = None;
                        self.last_transport_config = None;
                        self.set_connected(ctx, snapshot, false);
                    }
                }
            }
            Err(error) => {
                logerror!("Invalid OSC transport configuration: {}", error);
                self.stop_transport();
                self.last_transport_config = None;
                self.set_connected(ctx, snapshot, false);
            }
        }
    }

    fn read_transport_config(&self, snapshot: &ProcessTreeSnapshot) -> Result<OscTransportConfig, String> {
        let input_interface = self.required_enum_param(snapshot, "parameters/osc/input_interface")?;
        let bind_port = self.required_port(snapshot, "parameters/osc/bind_port")?;
        let receive_enabled = self.required_bool_param(snapshot, "parameters/osc/receive_enabled")?;

        Ok(OscTransportConfig {
            bind_interface_host: crate::app::module::common::network_interfaces::bind_host_for_interface_variant(
                input_interface.as_str(),
            ),
            bind_port,
            receive_enabled,
        })
    }

    fn drain_transport_events(&mut self, _ctx: &mut ProcessCtx, _snapshot: &ProcessTreeSnapshot) {
        let mut worker_events = Vec::new();
        let Some(transport) = &self.transport else {
            return;
        };

        while let Ok(event) = transport.try_recv() {
            worker_events.push(event);
        }

        for event in worker_events {
            match event {
                OscWorkerEvent::Message(message) => self.pending_incoming_messages.push(message),
                OscWorkerEvent::Error(error) => {
                    logerror!("OSC transport error: ", error);
                }
            }
        }
    }

    fn process_pending_incoming(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) -> bool {
        if self.pending_incoming_messages.is_empty() {
            return false;
        }

        let mut remaining = Vec::new();
        let mut messages = std::mem::take(&mut self.pending_incoming_messages).into_iter();

        while let Some(message) = messages.next() {
            match self.apply_incoming_message(ctx, snapshot, &message) {
                IncomingApplyResult::Applied | IncomingApplyResult::Ignored => {}
                IncomingApplyResult::Retry => {
                    remaining.push(message);
                    remaining.extend(messages);
                    self.pending_incoming_messages = remaining;
                    return true;
                }
            }
        }

        self.pending_incoming_messages = remaining;
        false
    }

    fn apply_incoming_message(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        message: &OscDecodedMessage,
    ) -> IncomingApplyResult {
        let Some(values_id) = snapshot.resolve_path_from(self.id(), "values") else {
            return IncomingApplyResult::Ignored;
        };

        let segments = address_segments(message.address.as_str());
        if segments.is_empty() {
            return IncomingApplyResult::Ignored;
        }

        let auto_add = self.auto_add_enabled(snapshot).unwrap_or(true);
        let (parent_id, leaf_name) = match self.resolve_or_create_parent(ctx, snapshot, values_id, &segments, auto_add) {
            ParentResolution::Ready { parent_id, leaf_name } => (parent_id, leaf_name),
            ParentResolution::Retry => return IncomingApplyResult::Retry,
            ParentResolution::Ignored => return IncomingApplyResult::Ignored,
        };

        match &message.payload {
            OscValuePayload::Single(value) => self.apply_single_value_message(
                ctx,
                snapshot,
                parent_id,
                leaf_name,
                value.clone(),
                auto_add,
                message.address.as_str(),
            ),
            OscValuePayload::Multi(values) => {
                self.apply_multi_value_message(ctx, snapshot, parent_id, leaf_name, values, auto_add)
            }
        }
    }

    fn resolve_or_create_parent(
        &mut self,
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

                    ctx.replace_node_boxed(child_id, Box::new(Folder::new(segment.clone())));
                    return ParentResolution::Retry;
                }
                None => {
                    if !auto_add {
                        return ParentResolution::Ignored;
                    }

                    ctx.add_child_boxed(current, Box::new(Folder::new(segment.clone())), None);
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
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        parent_id: NodeId,
        leaf_name: String,
        value: ParamValue,
        auto_add: bool,
        address: &str,
    ) -> IncomingApplyResult {
        match snapshot.find_child(parent_id, leaf_name.as_str()) {
            Some(existing_id) => {
                let Some(existing_snapshot) = snapshot.node(existing_id) else {
                    return IncomingApplyResult::Ignored;
                };

                if let Some(existing_value) = existing_snapshot.param_value.as_ref() {
                    if param_types_match(existing_value, &value) {
                        self.set_internal_param(ctx, existing_id, value);
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

                IncomingApplyResult::Applied
            }
            None => {
                if !auto_add {
                    return IncomingApplyResult::Ignored;
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
                IncomingApplyResult::Applied
            }
        }
    }

    fn apply_multi_value_message(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        parent_id: NodeId,
        leaf_name: String,
        values: &[ParamValue],
        auto_add: bool,
    ) -> IncomingApplyResult {
        let folder_id = match snapshot.find_child(parent_id, leaf_name.as_str()) {
            Some(existing_id) => {
                let Some(existing_snapshot) = snapshot.node(existing_id) else {
                    return IncomingApplyResult::Ignored;
                };
                if existing_snapshot.node_type == "folder" {
                    existing_id
                } else {
                    if !auto_add {
                        return IncomingApplyResult::Ignored;
                    }

                    ctx.replace_node_boxed(existing_id, Box::new(Folder::new(leaf_name)));
                    return IncomingApplyResult::Retry;
                }
            }
            None => {
                if !auto_add {
                    return IncomingApplyResult::Ignored;
                }

                ctx.add_child_boxed(parent_id, Box::new(Folder::new(leaf_name)), None);
                return IncomingApplyResult::Retry;
            }
        };

        self.sync_multi_value_folder(ctx, snapshot, folder_id, values, auto_add);
        IncomingApplyResult::Applied
    }

    fn sync_multi_value_folder(
        &mut self,
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
                            self.set_internal_param(ctx, existing_id, value.clone());
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

    fn flush_outbound_values(&mut self, _ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        if self.pending_outbound_nodes.is_empty() {
            return;
        }

        let outputs = self.collect_enabled_outputs(snapshot);
        if outputs.is_empty() {
            self.pending_outbound_nodes.clear();
            return;
        }

        let changed_nodes = std::mem::take(&mut self.pending_outbound_nodes);
        let target_nodes: HashSet<NodeId> = changed_nodes
            .into_iter()
            .filter_map(|node_id| resolve_outgoing_target(snapshot, node_id))
            .collect();

        for target_id in target_nodes {
            let Some(address) = outgoing_address_for_values_node(snapshot, self.id(), target_id) else {
                continue;
            };
            let Some(payload) = outgoing_payload_for_target(snapshot, target_id) else {
                continue;
            };

            for output in &outputs {
                let message = OscOutboundMessage {
                    address: address.clone(),
                    payload: payload.clone(),
                    remote_host: output.remote_host.clone(),
                    remote_port: output.remote_port,
                };

                let send_error = self.transport.as_ref().and_then(|transport| transport.send(message).err());
                if let Some(error_msg) = send_error {
                    logerror!("Failed to send OSC message to {}:{} - {}", output.remote_host, output.remote_port, error_msg);
                }
            }
        }
    }

    fn auto_add_enabled(&self, snapshot: &ProcessTreeSnapshot) -> Option<bool> {
        self.bool_param_at(snapshot, self.id(), "parameters/auto_add").ok()
    }

    fn collect_enabled_outputs(&self, snapshot: &ProcessTreeSnapshot) -> Vec<OscOutputTarget> {
        let Some(outputs_id) = snapshot.resolve_path_from(self.id(), "parameters/outputs") else {
            return Vec::new();
        };

        let mut output_ids = Vec::new();
        collect_outputs_recursive(snapshot, outputs_id, &mut output_ids);

        output_ids
            .into_iter()
            .filter_map(|output_id| self.output_target(snapshot, output_id))
            .collect()
    }

    fn output_target(&self, snapshot: &ProcessTreeSnapshot, output_id: NodeId) -> Option<OscOutputTarget> {
        if !self.bool_param_at(snapshot, output_id, "output/enabled").ok()? {
            return None;
        }

        let remote_host = self.string_param_at(snapshot, output_id, "output/remote_host")?;
        if remote_host.trim().is_empty() {
            return None;
        }

        let remote_port = self
            .int_param_at(snapshot, output_id, "output/remote_port")
            .and_then(|value| u16::try_from(value).ok())?;

        Some(OscOutputTarget {
            remote_host,
            remote_port,
        })
    }

    fn ensure_default_output_if_needed(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) -> bool {
        if self.default_output_checked {
            return false;
        }

        self.default_output_checked = true;
        let Some(outputs_id) = snapshot.resolve_path_from(self.id(), "parameters/outputs") else {
            return false;
        };

        let mut output_ids = Vec::new();
        collect_outputs_recursive(snapshot, outputs_id, &mut output_ids);
        if !output_ids.is_empty() {
            return false;
        }

        ctx.add_user_item_boxed(outputs_id, Box::new(crate::app::OscOutput::new()), None);
        true
    }

    fn set_connected(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot, connected: bool) {
        if let Some(connected_id) = snapshot.resolve_path_from(self.id(), "infos/connected") {
            self.set_internal_param(ctx, connected_id, ParamValue::Bool(connected));
        }
    }

    fn set_internal_param(&mut self, ctx: &mut ProcessCtx, param_id: NodeId, value: ParamValue) {
        self.ignored_param_changes.insert(param_id);
        ctx.set_param(param_id, value);
    }

    fn stop_transport(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            transport.stop();
        }
    }

    fn required_enum_param(&self, snapshot: &ProcessTreeSnapshot, path: &str) -> Result<String, String> {
        snapshot
            .resolve_path_from(self.id(), path)
            .and_then(|node_id| snapshot.node(node_id))
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_enum)
            .ok_or_else(|| format!("missing OSC enum parameter '{path}'"))
    }

    fn required_bool_param(&self, snapshot: &ProcessTreeSnapshot, path: &str) -> Result<bool, String> {
        self.bool_param_at(snapshot, self.id(), path)
            .map_err(|_| format!("missing OSC parameter '{path}'"))
    }

    fn required_port(&self, snapshot: &ProcessTreeSnapshot, path: &str) -> Result<u16, String> {
        let value = self
            .int_param_at(snapshot, self.id(), path)
            .ok_or_else(|| format!("missing OSC parameter '{path}'"))?;

        u16::try_from(value).map_err(|_| format!("OSC port '{path}' must be between 0 and 65535"))
    }

    fn string_param_at(&self, snapshot: &ProcessTreeSnapshot, start: NodeId, path: &str) -> Option<String> {
        snapshot.resolve_path_from(start, path).and_then(|node_id| {
            snapshot
                .node(node_id)
                .and_then(|node| node.param_value.as_ref())
                .and_then(ParamValue::as_str)
        })
    }

    fn int_param_at(&self, snapshot: &ProcessTreeSnapshot, start: NodeId, path: &str) -> Option<i32> {
        snapshot.resolve_path_from(start, path).and_then(|node_id| {
            snapshot
                .node(node_id)
                .and_then(|node| node.param_value.as_ref())
                .and_then(ParamValue::as_int)
        })
    }

    fn bool_param_at(&self, snapshot: &ProcessTreeSnapshot, start: NodeId, path: &str) -> Result<bool, String> {
        snapshot
            .resolve_path_from(start, path)
            .and_then(|node_id| snapshot.node(node_id))
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_bool)
            .ok_or_else(|| format!("missing boolean parameter '{path}'"))
    }
}

#[golden_core::item("module", node = "generic_osc_module", via = base, from_struct)]
impl Node for GenericOscModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.transport_dirty = true;
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        if self.ensure_default_output_if_needed(ctx, snapshot) {
            return;
        }

        if self.transport_dirty {
            self.refresh_transport(ctx, snapshot);
        }

        self.drain_transport_events(ctx, snapshot);
        if self.process_pending_incoming(ctx, snapshot) {
            return;
        }

        self.flush_outbound_values(ctx, snapshot);
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_transport();
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        u32::MAX
    }

    fn on_param_change(&mut self, _ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        if self.ignored_param_changes.remove(&param) {
            return;
        }

        let Some(snapshot_arc) = _ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        if is_descendant_of(snapshot, param, self.id(), Some("parameters")) {
            if is_descendant_of(snapshot, param, self.id(), Some("parameters/osc")) {
                self.transport_dirty = true;
            }

            return;
        }

        if is_descendant_of(snapshot, param, self.id(), Some("values")) {
            self.pending_outbound_nodes.insert(param);
        }
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == GENERIC_OSC_MODULE_NODE_TYPE).then(Self::create)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OscOutputTarget {
    remote_host: String,
    remote_port: u16,
}

enum ParentResolution {
    Ready { parent_id: NodeId, leaf_name: String },
    Retry,
    Ignored,
}

enum IncomingApplyResult {
    Applied,
    Retry,
    Ignored,
}

fn create_parameter_node(label: &str, value: ParamValue, description: Option<String>) -> Parameter {
    let mut parameter = Parameter::new(label, value, ParameterChangeCheck::ValueChange);
    parameter.node_data_mut().meta.description = description;
    parameter
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

fn collect_outputs_recursive(snapshot: &ProcessTreeSnapshot, parent: NodeId, output: &mut Vec<NodeId>) {
    for child_id in snapshot.child_ids(parent) {
        let Some(child_snapshot) = snapshot.node(child_id) else {
            continue;
        };
        if child_snapshot.node_type == OSC_OUTPUT_NODE_TYPE {
            output.push(child_id);
        } else if child_snapshot.node_type == "folder" {
            collect_outputs_recursive(snapshot, child_id, output);
        }
    }
}

fn resolve_outgoing_target(snapshot: &ProcessTreeSnapshot, node_id: NodeId) -> Option<NodeId> {
    let node = snapshot.node(node_id)?;
    if node.param_value.is_none() {
        return None;
    }

    let parent_id = node.parent?;
    let parent = snapshot.node(parent_id)?;
    if parent.node_type == "folder"
        && indexed_value_label_index(node.label.as_str()).is_some()
        && has_indexed_value_children(snapshot, parent_id)
    {
        return Some(parent_id);
    }

    Some(node_id)
}

fn outgoing_address_for_values_node(snapshot: &ProcessTreeSnapshot, module_id: NodeId, target_id: NodeId) -> Option<String> {
    let values_id = snapshot.resolve_path_from(module_id, "values")?;
    let mut segments = Vec::new();
    let mut current = Some(target_id);

    while let Some(node_id) = current {
        if node_id == values_id {
            break;
        }

        let node = snapshot.node(node_id)?;
        let segment = node.label.trim().trim_matches('/');
        if !segment.is_empty() {
            segments.push(segment.to_string());
        }
        current = node.parent;
    }

    if current != Some(values_id) || segments.is_empty() {
        return None;
    }

    segments.reverse();
    Some(format!("/{}", segments.join("/")))
}

fn outgoing_payload_for_target(snapshot: &ProcessTreeSnapshot, target_id: NodeId) -> Option<OscValuePayload> {
    let target = snapshot.node(target_id)?;
    if let Some(value) = target.param_value.clone() {
        return Some(OscValuePayload::Single(value));
    }

    if target.node_type != "folder" {
        return None;
    }

    let mut indexed_values = snapshot
        .child_ids(target_id)
        .into_iter()
        .filter_map(|child_id| {
            let child = snapshot.node(child_id)?;
            let index = indexed_value_label_index(child.label.as_str())?;
            let value = child.param_value.clone()?;
            Some((index, value))
        })
        .collect::<Vec<_>>();
    indexed_values.sort_by_key(|(index, _)| *index);

    if indexed_values.is_empty() {
        return None;
    }

    Some(OscValuePayload::Multi(
        indexed_values.into_iter().map(|(_, value)| value).collect(),
    ))
}

fn has_indexed_value_children(snapshot: &ProcessTreeSnapshot, folder_id: NodeId) -> bool {
    snapshot.child_ids(folder_id).into_iter().any(|child_id| {
        snapshot
            .node(child_id)
            .is_some_and(|child| indexed_value_label_index(child.label.as_str()).is_some())
    })
}

fn is_descendant_of(snapshot: &ProcessTreeSnapshot, start: NodeId, ancestor: NodeId, path: Option<&str>) -> bool {
    let Some(expected_root) = path.and_then(|relative| snapshot.resolve_path_from(ancestor, relative)) else {
        return path.is_none();
    };

    let mut current = Some(start);
    while let Some(node_id) = current {
        if node_id == expected_root {
            return true;
        }
        current = snapshot.node(node_id).and_then(|node| node.parent);
    }

    false
}
